/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Main image logic.
#![allow(clippy::doc_markdown)]

use alloc::string::ToString;
use alloc::vec::Vec;
use alloc::{format, vec};

use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{error, trace, warn};
use zune_core::options::DecoderOptions;

use crate::bitstream::BitStreamHuffman;
#[cfg(feature = "arith")]
use crate::bitstream_arith::{ArithACTables, ArithDCTables, BitStreamArithmetic};
use crate::color_convert::choose_ycbcr_to_rgb_convert_func;
use crate::components::{Components, SampleRatios};
use crate::errors::{DecodeErrors, UnsupportedSchemes};
#[cfg(feature = "arith")]
use crate::headers::parse_dac;
use crate::headers::{
    parse_app1, parse_app13, parse_app14, parse_app2, parse_dqt, parse_huffman, parse_sos,
    parse_start_of_frame
};
use crate::huffman::HuffmanTable;
use crate::idct::{choose_idct_func, choose_idct_1x1_func, choose_idct_4x4_func};
use crate::marker::Marker;
use crate::misc::SOFMarkers;
use crate::upsampler::{
    choose_horizontal_samp_function, choose_hv_samp_function, choose_v_samp_function,
    generic_sampler, upsample_no_op
};

/// Maximum components
pub(crate) const MAX_COMPONENTS: usize = 4;

/// Maximum image dimensions supported.
pub(crate) const MAX_DIMENSIONS: usize = 1 << 27;

/// Color conversion function that can convert YCbCr colorspace to RGB(A/X) for
/// 16 values
///
/// The following are guarantees to the following functions
///
/// 1. The `&[i16]` slices passed contain 16 items
///
/// 2. The slices passed are in the following order
///    `y,cb,cr`
///
/// 3. `&mut [u8]` is zero initialized
///
/// 4. `&mut usize` points to the position in the array where new values should
///    be used
///
/// The pointer should
/// 1. Carry out color conversion
/// 2. Update `&mut usize` with the new position
pub type ColorConvert16Ptr = fn(&[i16; 16], &[i16; 16], &[i16; 16], &mut [u8], &mut usize);

/// IDCT  function prototype
///
/// This encapsulates a dequantize and IDCT function which will carry out the
/// following functions
///
/// Multiply each 64 element block of `&mut [i16]` with `&Aligned32<[i32;64]>`
/// Carry out IDCT (type 3 dct) on ach block of 64 i16's
pub type IDCTPtr = fn(&mut [i32; 64], &mut [i16], usize);

/// Tracks the current decoding phase for incremental decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecodingState {
    /// Headers not yet fully decoded. `resume_position == 0` means start from SOI.
    DecodeHeaders { resume_position: usize },
    /// Headers decoded; scan data starts at `scan_start_position`.
    ///
    /// `append_snapshot` records the lengths of the append-only metadata
    /// buffers (ICC, extended XMP, gain map) at the moment we entered the
    /// scan. On every scan retry we roll those buffers back to this snapshot
    /// before re-seeking, so inline markers re-encountered during replay
    /// (see `mcu.rs::check_stream_marker_after_mcu_width`) do not duplicate
    /// metadata entries.
    DecodeScan {
        scan_start_position: usize,
        append_snapshot:     HeaderAppendStateSnapshot
    },
}

// Snapshot of append-only buffers populated across multi-segment markers
// (ICC chunks, extended XMP, gain map). All other parsers either overwrite
// fixed slots (qt/huffman/components) or fail before mutating, so only
// these three need to be rolled back when a marker parser errors out
// or when scan-phase replay re-seeks past markers it already consumed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HeaderAppendStateSnapshot {
    icc:  usize,
    xmp:  usize,
    gain: usize
}

impl HeaderAppendStateSnapshot {
    fn capture<T: ZByteReaderTrait>(decoder: &JpegDecoder<T>) -> Self {
        Self {
            icc:  decoder.icc_data.len(),
            xmp:  decoder.extended_xmp_segments.len(),
            gain: decoder.info.gain_map_info.len()
        }
    }

    fn rollback<T: ZByteReaderTrait>(self, decoder: &mut JpegDecoder<T>) {
        decoder.icc_data.truncate(self.icc);
        decoder.extended_xmp_segments.truncate(self.xmp);
        decoder.info.gain_map_info.truncate(self.gain);
    }
}

/// Result of handling a single marker inside the header loop.
enum MarkerStep {
    /// Continue reading further markers.
    Continue,
    /// Reached SOS; headers are done and scan starts at the current position.
    EnteredScan
}

/// An encapsulation of an ICC chunk
pub(crate) struct ICCChunk {
    pub(crate) seq_no:      u8,
    pub(crate) num_markers: u8,
    pub(crate) data:        Vec<u8>
}

// A separate struct to allow &borrowing tables while &mut borrowing components
pub(crate) struct EntropyTables {
    /// DC Huffman Tables with a maximum of 4 tables for each  component
    pub(crate) dc_huffman:    [Option<HuffmanTable>; MAX_COMPONENTS],
    /// AC Huffman Tables with a maximum of 4 tables for each component
    pub(crate) ac_huffman:    [Option<HuffmanTable>; MAX_COMPONENTS],
    /// Arithmetic coding initial conditioning parameters and statistics (has a default value)
    #[cfg(feature = "arith")]
    pub(crate) dc_arithmetic: [ArithDCTables; MAX_COMPONENTS],
    /// Arithmetic coding initial conditioning parameters and statistics  (has a default value)
    #[cfg(feature = "arith")]
    pub(crate) ac_arithmetic: [ArithACTables; MAX_COMPONENTS]
}

/// A JPEG Decoder Instance.
#[allow(clippy::upper_case_acronyms, clippy::struct_excessive_bools)]
pub struct JpegDecoder<T> {
    /// Struct to hold image information from SOI
    pub(crate) info:             ImageInfo,
    ///  Quantization tables, will be set to none and the tables will
    /// be moved to `components` field
    pub(crate) qt_tables:        [Option<[i32; 64]>; MAX_COMPONENTS],
    // Entropy coding tables
    pub(crate) entropy_tables:   EntropyTables,
    /// Image components, holds information like DC prediction and quantization
    /// tables of a component
    pub(crate) components:       Vec<Components>,
    /// maximum horizontal component of all channels in the image
    pub(crate) h_max:            usize,
    // maximum vertical component of all channels in the image
    pub(crate) v_max:            usize,
    /// mcu's  width (interleaved scans)
    pub(crate) mcu_width:        usize,
    /// MCU height(interleaved scans
    pub(crate) mcu_height:       usize,
    /// Number of MCU's in the x plane
    pub(crate) mcu_x:            usize,
    /// Number of MCU's in the y plane
    pub(crate) mcu_y:            usize,
    /// Is the image interleaved?
    pub(crate) is_interleaved:   bool,
    /// Image input colorspace, should be YCbCr for a sane image, might be
    /// grayscale too
    pub(crate) input_colorspace: ColorSpace,
    // Is the image using arithmetic coding?
    pub(crate) is_arithmetic:    bool,
    // Progressive image details
    /// Is the image progressive?
    pub(crate) is_progressive:   bool,

    /// Start of spectral scan
    pub(crate) spec_start:       u8,
    /// End of spectral scan
    pub(crate) spec_end:         u8,
    /// Successive approximation bit position high
    pub(crate) succ_high:        u8,
    /// Successive approximation bit position low
    pub(crate) succ_low:         u8,
    /// Number of components.
    pub(crate) num_scans:        u8,
    /// For a scan, check if any component has vertical/horizontal sampling.
    pub(crate) scan_subsampled:  bool,
    // Function pointers, for pointy stuff.
    /// Dequantize and idct function
    // This is determined at runtime which function to run, statically it's
    // initialized to a platform independent one and during initialization
    // of this struct, we check if we can switch to a faster one which
    // depend on certain CPU extensions.
    pub(crate) idct_func: IDCTPtr,
    /// Specialized IDCT when we can guarantee only few coefficients are non-zero.
    ///
    /// **The callee must uphold a contract**. See [`choose_idct_4x4_func`].
    pub(crate) idct_4x4_func: IDCTPtr,
    pub(crate) idct_1x1_func: IDCTPtr,
    // Color convert function which acts on 16 YCbCr values
    pub(crate) color_convert_16: ColorConvert16Ptr,
    pub(crate) z_order:          [usize; MAX_COMPONENTS],
    /// restart markers
    pub(crate) restart_interval: usize,
    pub(crate) todo:             usize,
    // decoder options
    pub(crate) options:          DecoderOptions,
    // byte-stream
    pub(crate) stream:           ZReader<T>,
    // Indicate whether headers have been decoded
    pub(crate) headers_decoded:  bool,
    pub(crate) seen_sof:         bool,

    // exif data, lifted from app2
    pub(crate) icc_data: Vec<ICCChunk>,
    pub(crate) is_mjpeg: bool,
    pub(crate) coeff:    usize, // Solves some weird bug :)
    /// Extended XMP segments
    pub(crate) extended_xmp_segments: Vec<ExtendedXmpSegment>,
    /// Current decoding phase for incremental decoding.
    state: DecodingState,
}

impl<T> JpegDecoder<T>
where
    T: ZByteReaderTrait
{
    // Mark the current stream position as a safe resume point at a marker
    // boundary; on a future retry decode_headers_internal will seek here
    // instead of restarting from SOI.
    fn stream_position(&mut self) -> Result<usize, DecodeErrors> {
        let position = self.stream.position()?;
        usize::try_from(position).map_err(|_| {
            DecodeErrors::FormatStatic("Stream position does not fit in usize")
        })
    }

    fn checkpoint_headers(&mut self) -> Result<(), DecodeErrors> {
        let resume_position = self.stream_position()?;
        self.state = DecodingState::DecodeHeaders { resume_position };
        Ok(())
    }

    fn enter_scan_state(&mut self) -> Result<(), DecodeErrors> {
        let scan_start_position = self.stream_position()?;
        let append_snapshot = HeaderAppendStateSnapshot::capture(self);
        self.state = DecodingState::DecodeScan {
            scan_start_position,
            append_snapshot
        };
        Ok(())
    }

    // Match output colorspace; we only care for ycbcr to rgb/rgba here, in
    // case one is using another colorspace may god help you.
    fn set_color_convert_from_options(&mut self) {
        let out_colorspace = self.options.jpeg_get_out_colorspace();
        if matches!(
            out_colorspace,
            ColorSpace::BGR | ColorSpace::BGRA | ColorSpace::RGB | ColorSpace::RGBA
        ) {
            self.color_convert_16 = choose_ycbcr_to_rgb_convert_func(
                self.options.jpeg_get_out_colorspace(),
                &self.options
            )
            .unwrap();
        }
    }

    #[allow(clippy::redundant_field_names)]
    fn default(options: DecoderOptions, buffer: T) -> Self {
        let color_convert = choose_ycbcr_to_rgb_convert_func(ColorSpace::RGB, &options).unwrap();
        JpegDecoder {
            info:                  ImageInfo::default(),
            qt_tables:             [None, None, None, None],
            entropy_tables:        EntropyTables {
                dc_huffman: [None, None, None, None],
                ac_huffman: [None, None, None, None],
                #[cfg(feature = "arith")]
                dc_arithmetic: [
                    ArithDCTables::default(),
                    ArithDCTables::default(),
                    ArithDCTables::default(),
                    ArithDCTables::default()
                ],
                #[cfg(feature = "arith")]
                ac_arithmetic: [
                    ArithACTables::default(),
                    ArithACTables::default(),
                    ArithACTables::default(),
                    ArithACTables::default()
                ]
            },
            components:        vec![],
            // Interleaved information
            h_max:             1,
            v_max:             1,
            mcu_height:        0,
            mcu_width:         0,
            mcu_x:             0,
            mcu_y:             0,
            is_interleaved:    false,
            is_arithmetic:     false,
            is_progressive:    false,
            spec_start:        0,
            spec_end:          0,
            succ_high:         0,
            succ_low:          0,
            num_scans:         0,
            scan_subsampled:   false, 
            idct_func:         choose_idct_func(&options),
            idct_4x4_func:     choose_idct_4x4_func(&options),
            idct_1x1_func:     choose_idct_1x1_func(&options),
            color_convert_16:  color_convert,
            input_colorspace:  ColorSpace::YCbCr,
            z_order:           [0; MAX_COMPONENTS],
            restart_interval:  0,
            todo:              0x7fff_ffff,
            options:           options,
            stream:            ZReader::new(buffer),
            headers_decoded:   false,
            seen_sof:          false,
            icc_data:          vec![],
            is_mjpeg:          false,
            coeff:             1,
            extended_xmp_segments: vec![],
            state:             DecodingState::DecodeHeaders { resume_position: 0 },
        }
    }
    /// Decode a buffer already in memory
    ///
    /// The buffer should be a valid jpeg file, perhaps created by the command
    /// `std:::fs::read()` or a JPEG file downloaded from the internet.
    ///
    /// # Errors
    /// See DecodeErrors for an explanation
    pub fn decode(&mut self) -> Result<Vec<u8>, DecodeErrors> {
        self.decode_headers()?;
        let size = self.output_buffer_size().unwrap();
        let mut out = vec![0; size];
        self.decode_into(&mut out)?;
        Ok(out)
    }

    /// Create a new Decoder instance
    ///
    /// # Arguments
    ///  - `stream`: The raw bytes of a jpeg file.
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new(stream: T) -> JpegDecoder<T> {
        JpegDecoder::default(DecoderOptions::default(), stream)
    }

    /// Returns the image information
    ///
    /// This **must** be called after a subsequent call to [`decode`] or [`decode_headers`]
    /// it will return `None`
    ///
    /// # Returns
    /// - `Some(info)`: Image information,width, height, number of components
    /// - None: Indicates image headers haven't been decoded
    ///
    /// [`decode`]: JpegDecoder::decode
    /// [`decode_headers`]: JpegDecoder::decode_headers
    #[must_use]
    pub fn info(&self) -> Option<ImageInfo> {
        // we check for fails to that call by comparing what we have to the default, if
        // it's default we assume that the caller failed to uphold the
        // guarantees. We can be sure that an image cannot be the default since
        // its a hard panic in-case width or height are set to zero.
        if !self.headers_decoded {
            return None;
        }

        return Some(self.info.clone());
    }

    /// Return the number of bytes required to hold a decoded image frame
    /// decoded using the given input transformations
    ///
    /// # Returns
    ///  - `Some(usize)`: Minimum size for a buffer needed to decode the image
    ///  - `None`: Indicates the image was not decoded, or image dimensions would overflow a usize
    ///
    #[must_use]
    pub fn output_buffer_size(&self) -> Option<usize> {
        return if self.headers_decoded {
            Some(
                usize::from(self.width())
                    .checked_mul(usize::from(self.height()))?
                    .checked_mul(self.options.jpeg_get_out_colorspace().num_components())?
            )
        } else {
            None
        };
    }

    /// Get an immutable reference to the decoder options
    /// for the decoder instance
    ///
    /// This can be used to modify options before actual decoding
    /// but after initial creation
    ///
    /// # Example
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_jpeg::JpegDecoder;
    ///
    /// let mut decoder = JpegDecoder::new(ZCursor::new(&[]));
    /// // get current options
    /// let mut options = decoder.options();
    /// // modify it
    ///  let new_options = options.set_max_width(10);
    /// // set it back
    /// decoder.set_options(new_options);
    ///
    /// ```
    #[must_use]
    pub const fn options(&self) -> &DecoderOptions {
        &self.options
    }
    /// Return the input colorspace of the image
    ///
    /// This indicates the colorspace that is present in
    /// the image, but this may be different to the colorspace that
    /// the output will be transformed to
    ///
    /// # Returns
    /// -`Some(Colorspace)`: Input colorspace
    /// - None : Indicates the headers weren't decoded
    #[must_use]
    pub fn input_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded { Some(self.input_colorspace) } else { None };
    }
    /// Set decoder options
    ///
    /// This can be used to set new options even after initialization
    /// but before decoding.
    ///
    /// This does not bear any significance after decoding an image
    ///
    /// # Arguments
    /// - `options`: New decoder options
    ///
    /// # Example
    /// Set maximum jpeg progressive passes to be 4
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder =JpegDecoder::new(ZCursor::new(&[]));
    /// // this works also because DecoderOptions implements `Copy`
    /// let options = decoder.options().jpeg_set_max_scans(4);
    /// // set the new options
    /// decoder.set_options(options);
    /// // now decode
    /// decoder.decode().unwrap();
    /// ```
    pub fn set_options(&mut self, options: DecoderOptions) {
        self.options = options;
    }
    #[allow(clippy::cast_possible_truncation)]
    fn reassemble_extended_xmp(&mut self) {
        if self.extended_xmp_segments.is_empty() {
            return;
        }

        // Sort by offset
        self.extended_xmp_segments.sort_by_key(|a| a.offset);

        let guid = &self.extended_xmp_segments[0].guid;
        let total_size = self.extended_xmp_segments[0].total_size;

        // Check for consistency
        for segment in &self.extended_xmp_segments {
            if &segment.guid != guid || segment.total_size != total_size {
                error!("Inconsistent Extended XMP segments");
                self.extended_xmp_segments.clear();
                return;
            }
        }

        let mut rolling_offset = 0;
        let mut complete = true;

        for segment in &self.extended_xmp_segments {
            if segment.offset != rolling_offset {
                // Gap or overlap
                complete = false;
                break;
            }
            rolling_offset += segment.data.len() as u32;
        }

        if complete && rolling_offset == total_size {
            let mut result = Vec::with_capacity(total_size as usize);
            for segment in &self.extended_xmp_segments {
                result.extend_from_slice(&segment.data);
            }
            self.info.extended_xmp = Some(result);
            self.info.extended_xmp_guid = Some(guid.clone());
            self.extended_xmp_segments.clear();
        } else if rolling_offset > total_size {
            error!("Extended XMP overflow");
            self.extended_xmp_segments.clear();
        }
        // Else: Incomplete, wait for more.
    }
    /// Decode Decoder headers
    ///
    /// This routine takes care of parsing supported headers from a Decoder
    /// image
    ///
    /// # Supported Headers
    ///  - APP(0)
    ///  - SOF(O)
    ///  - DQT -> Quantization tables
    ///  - DHT -> Huffman tables
    ///  - SOS -> Start of Scan
    /// # Unsupported Headers
    ///  - SOF(n) -> Decoder images which are not baseline/progressive
    ///  - DAC -> Images using Arithmetic tables
    ///  - JPG(n)
    fn decode_headers_internal(&mut self) -> Result<(), DecodeErrors> {
        match self.state {
            DecodingState::DecodeScan { .. } => {
                return Ok(());
            }
            DecodingState::DecodeHeaders { resume_position } => {
                if resume_position == 0 {
                    self.reset_header_state();

                    // First two bytes should be jpeg soi marker
                    let magic_bytes = self.stream.get_u16_be_err()?;

                    if magic_bytes != 0xffd8 {
                        return Err(DecodeErrors::IllegalMagicBytes(magic_bytes));
                    }

                    // Color convert depends only on options, so pick it once
                    // on a fresh decode rather than on every resume.
                    self.set_color_convert_from_options();
                    self.checkpoint_headers()?;
                } else {
                    self.stream.set_position(resume_position)?;
                }
            }
        }

        let mut last_byte = 0;
        let mut bytes_before_marker = 0;

        loop {
            // read a byte
            let mut m = self.stream.read_u8_err()?;

            // AND OF COURSE some images will have fill bytes in their marker
            // bitstreams because why not.
            //
            // I am disappointed as a man.
            if (m == 0xFF || m == 0) && last_byte == 0xFF {
                // This handles the edge case where
                // images have markers with fill bytes(0xFF)
                // or byte stuffing (0)
                // I.e 0xFF 0xFF 0xDA
                // and
                // 0xFF 0 0xDA
                // It should ignore those fill bytes and take 0xDA
                // I don't know why such images exist
                // but they do.
                // so this is for you (with love)
                while m == 0xFF || m == 0x0 {
                    last_byte = m;
                    m = self.stream.read_u8_err()?;
                }
            }
            // Last byte should be 0xFF to confirm existence of a marker since markers look
            // like OxFF(some marker data)
            if last_byte == 0xFF {
                let marker = Marker::from_u8(m);
                if let Some(n) = marker {
                    if bytes_before_marker > 3 {
                        if self.options.strict_mode()
                        /*No reason to use this*/
                        {
                            return Err(DecodeErrors::FormatStatic(
                                "[strict-mode]: Extra bytes between headers"
                            ));
                        }

                        error!(
                            "Extra bytes {} before marker 0xFF{:X}",
                            bytes_before_marker - 3,
                            m
                        );
                    }

                    bytes_before_marker = 0;

                    if let MarkerStep::EnteredScan = self.handle_known_marker(n)? {
                        return Ok(());
                    }
                } else {
                    bytes_before_marker = 0;
                    warn!("Marker 0xFF{m:X} not known");
                    self.skip_unknown_marker()?;
                }
            }
            last_byte = m;
            bytes_before_marker += 1;
        }
    }

    // Parse a recognised marker and update the resume checkpoint. Rollback of
    // append-only metadata on parser error lives inside `parse_marker_inner`
    // itself so every caller (including the inline-marker path in `mcu.rs`)
    // is protected uniformly.
    fn handle_known_marker(&mut self, n: Marker) -> Result<MarkerStep, DecodeErrors> {
        self.parse_marker_inner(n)?;

        if !self.extended_xmp_segments.is_empty() {
            self.reassemble_extended_xmp();
        }

        // break after reading the start of scan.
        // what follows is the image data
        if n == Marker::SOS {
            self.headers_decoded = true;
            trace!("Input colorspace {:?}", self.input_colorspace);

            // Check if image is RGB
            // The check is weird, we need to check if ID
            // represents R, G and B in ascii,
            //
            // I am not sure if this is even specified in any standard,
            // but jpegli https://github.com/google/jpegli does encode
            // its images that way, so this will check for that. and handle it appropriately
            // It is spefified here so that on a successful header decode,we can at least
            // try to attribute image colorspace  correctly.
            //
            // It was first the issue in https://github.com/etemesi254/zune-image/issues/291
            // that brought it to light
            //
            let mut is_rgb = self.components.len() == 3;
            let chars = ['R', 'G', 'B'];
            for (comp, single_char) in self.components.iter().zip(chars.iter()) {
                is_rgb &= comp.id == (*single_char) as u8;
            }
            // Image is RGB, change colorspace
            if is_rgb {
                self.input_colorspace = ColorSpace::RGB;
            }

            self.enter_scan_state()?;
            return Ok(MarkerStep::EnteredScan);
        }

        self.checkpoint_headers()?;
        Ok(MarkerStep::Continue)
    }

    // Read a length-prefixed marker payload and skip past it. Shared by the
    // unknown-marker path in `decode_headers_internal` and the catch-all arm
    // in `parse_marker_inner` so the length validation lives in one place.
    fn skip_marker_payload(&mut self) -> Result<(), DecodeErrors> {
        let length = self.stream.get_u16_be_err()?;

        if length < 2 {
            return Err(DecodeErrors::Format(format!(
                "Found a marker with invalid length : {length}"
            )));
        }

        warn!("Skipping {} bytes", length - 2);
        self.stream.skip((length - 2) as usize)?;
        Ok(())
    }

    // Skip a marker we don't recognise, then checkpoint past it so we don't
    // need to re-skip on retry.
    fn skip_unknown_marker(&mut self) -> Result<(), DecodeErrors> {
        self.skip_marker_payload()?;
        self.checkpoint_headers()?;
        Ok(())
    }
    pub(crate) fn parse_marker_inner(&mut self, m: Marker) -> Result<(), DecodeErrors> {
        // Snapshot append-only metadata so any error path leaves the decoder
        // in the same shape as before the marker started. Required for both
        // header-phase resume and the non-strict inline-marker dispatch from
        // `mcu.rs::check_stream_marker_after_mcu_width`.
        let snapshot = HeaderAppendStateSnapshot::capture(self);
        let result = self.parse_marker_dispatch(m);
        if result.is_err() {
            snapshot.rollback(self);
        }
        result
    }

    #[allow(clippy::too_many_lines)]
    fn parse_marker_dispatch(&mut self, m: Marker) -> Result<(), DecodeErrors> {
        match m {
            Marker::SOF(0..=2) => {
                // choose marker
                let marker =
                    match m {
                        Marker::SOF(0 | 1) =>
                            SOFMarkers::BaselineDct,
                        Marker::SOF(2) => {
                            self.is_progressive = true;
                            SOFMarkers::ProgressiveDctHuffman
                        }
                        _ => unreachable!(),
                    };

                trace!("Image encoding scheme =`{marker:?}`");
                // get components
                parse_start_of_frame(marker, self)?;
            }
            #[cfg(feature = "arith")]
            Marker::SOF(9..=10) => {
                // choose marker
                self.is_arithmetic = true;
                let marker = match m {
                    Marker::SOF(9) => SOFMarkers::ExtendedSequentialDctArithmetic,
                    Marker::SOF(10) => {
                        self.is_progressive = true;
                        self.is_arithmetic = true;
                        SOFMarkers::ProgressiveDctArithmetic
                    }
                    _ => unreachable!()
                };

                trace!("Image encoding scheme =`{marker:?}`");
                // get components
                parse_start_of_frame(marker, self)?;
            }
            // Start of Frame Segments not supported
            Marker::SOF(v) => {
                let feature = UnsupportedSchemes::from_int(v);

                if let Some(feature) = feature {
                    return Err(DecodeErrors::Unsupported(feature));
                }

                return Err(DecodeErrors::Format(format!(
                    "Unsupported image format (SOF_{v})"
                )));
            }
            //APP(0) segment
            Marker::APP(0) => {
                let mut length = self.stream.get_u16_be_err()?;

                if length < 2 {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                // skip for now
                if length > 5 {
                    let mut buffer = [0u8; 5];
                    self.stream.read_exact_bytes(&mut buffer)?;
                    if &buffer == b"AVI1\0" {
                        self.is_mjpeg = true;
                    }
                    length -= 5;
                }

                self.stream.skip(length.saturating_sub(2) as usize)?;

                //parse_app(buf, m, &mut self.info)?;
            }
            Marker::APP(1) => {
                parse_app1(self)?;
            }

            Marker::APP(2) => {
                parse_app2(self)?;
            }
            // Quantization tables
            Marker::DQT => {
                parse_dqt(self)?;
            }
            // Huffman tables
            Marker::DHT => {
                parse_huffman(self)?;
            }
            // Start of Scan Data
            Marker::SOS => {
                parse_sos(self)?;
            }
            Marker::EOI => return Err(DecodeErrors::FormatStatic("Premature End of image")),

            #[cfg(feature = "arith")]
            Marker::DAC => {
                parse_dac(self)?;
            }

            Marker::DNL => {
                return Err(DecodeErrors::Format(format!(
                    "Parsing of the following header `{m:?}` is not supported,\
                                cannot continue"
                )));
            }
            Marker::DRI => {
                if self.stream.get_u16_be_err()? != 4 {
                    return Err(DecodeErrors::Format(
                        "Bad DRI length, Corrupt JPEG".to_string()
                    ));
                }

                self.restart_interval = usize::from(self.stream.get_u16_be_err()?);
                trace!("DRI marker present ({})", self.restart_interval);

                self.todo = self.restart_interval;
            }
            Marker::APP(14) => {
                parse_app14(self)?;
            }
            Marker::APP(13) => {
                parse_app13(self)?;
            }
            _ => {
                warn!(
                    "Capabilities for processing marker \"{m:?}\" not implemented"
                );
                self.skip_marker_payload()?;
            }
        }
        Ok(())
    }

    /// Get the embedded ICC profile if it exists
    /// and is correct
    ///
    /// One needs not to decode the whole image to extract this,
    /// calling [`decode_headers`] for an image with an ICC profile
    /// allows you to decode this
    ///
    /// # Returns
    /// - `Some(Vec<u8>)`: The raw ICC profile of the image
    /// - `None`: May indicate an error  in the ICC profile , non-existence of
    ///   an ICC profile, or that the headers weren't decoded.
    ///
    /// [`decode_headers`]:Self::decode_headers
    #[must_use]
    pub fn icc_profile(&self) -> Option<Vec<u8>> {
        let mut marker_present: [Option<&ICCChunk>; 256] = [None; 256];

        if !self.headers_decoded {
            return None;
        }
        let num_markers = self.icc_data.len();

        if num_markers == 0 || num_markers >= 255 {
            return None;
        }
        // check validity
        for chunk in &self.icc_data {
            if usize::from(chunk.num_markers) != num_markers {
                // all the lengths must match
                return None;
            }
            if chunk.seq_no == 0 {
                warn!("Zero sequence number in ICC, corrupt ICC chunk");
                return None;
            }
            if marker_present[usize::from(chunk.seq_no)].is_some() {
                // duplicate seq_no
                warn!("Duplicate sequence number in ICC, corrupt chunk");
                return None;
            }

            marker_present[usize::from(chunk.seq_no)] = Some(chunk);
        }
        let mut data = Vec::with_capacity(1000);
        // assemble the data now
        for chunk in marker_present.get(1..=num_markers).unwrap() {
            if let Some(ch) = chunk {
                data.extend_from_slice(&ch.data);
            } else {
                warn!("Missing icc sequence number, corrupt ICC chunk ");
                return None;
            }
        }

        Some(data)
    }
    /// Return the exif data for the file
    ///
    /// This returns the raw exif data starting at the
    /// TIFF header
    ///
    /// # Returns
    /// -`Some(data)`: The raw exif data, if present in the image
    /// - None: May indicate the following
    ///
    ///    1. The image doesn't have exif data
    ///    2. The image headers haven't been decoded
    #[must_use]
    pub fn exif(&self) -> Option<&Vec<u8>> {
        return self.info.exif_data.as_ref();
    }
    /// Return the XMP data for the file
    ///
    /// This returns raw XMP data starting at the XML header
    /// One needs an XML/XMP decoder to extract valuable metadata
    ///
    ///
    /// # Returns
    ///  - `Some(data)`: Raw xmp data
    ///  - `None`: May indicate the following
    ///     1. The image does not have xmp data
    ///     2. The image headers have not been decoded
    ///
    /// # Example
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder = JpegDecoder::new(ZCursor::new(&[]));
    /// // decode headers to extract xmp metadata if present
    /// decoder.decode_headers().unwrap();
    /// if let Some(data) = decoder.xmp(){
    ///     let stringified = String::from_utf8_lossy(data);
    ///     println!("XMP")
    /// } else{
    ///     println!("No XMP Found")
    /// }
    ///
    /// ```
    pub fn xmp(&self) -> Option<&Vec<u8>> {
        return self.info.xmp_data.as_ref();
    }
    /// Return the IPTC data for the file
    ///
    /// This returns the raw IPTC data.
    ///
    /// # Returns
    /// -`Some(data)`: The raw IPTC data, if present in the image
    /// - None: May indicate the following
    ///
    ///    1. The image doesn't have IPTC data
    ///    2. The image headers haven't been decoded
    #[must_use]
    pub fn iptc(&self) -> Option<&Vec<u8>> {
        return self.info.iptc_data.as_ref();
    }
    /// Get the output colorspace the image pixels will be decoded into
    ///
    ///
    /// # Note.
    /// This field can only be regarded after decoding headers,
    /// as markers such as Adobe APP14 may dictate different colorspaces
    /// than requested.
    ///
    /// Calling `decode_headers` is sufficient to know what colorspace the
    /// output is, if this is called after `decode` it indicates the colorspace
    /// the output is currently in
    ///
    /// Additionally, not all input->output colorspace mappings are supported
    /// but all input colorspaces can map to RGB colorspace, so that's a safe bet
    /// if one is handling image formats
    ///
    ///# Returns
    /// - `Some(Colorspace)`: If headers have been decoded, the colorspace the
    ///   output array will be in
    ///- `None
    #[must_use]
    pub fn output_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded {
            Some(self.options.jpeg_get_out_colorspace())
        } else {
            None
        };
    }

    /// Decode into a pre-allocated buffer
    ///
    /// It is an error if the buffer size is smaller than
    /// [`output_buffer_size()`](Self::output_buffer_size)
    ///
    /// If the buffer is bigger than expected, we ignore the end padding bytes
    ///
    /// # Example
    ///
    /// - Read  headers and then alloc a buffer big enough to hold the image
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder = JpegDecoder::new(ZCursor::new(&[]));
    /// // before we get output, we must decode the headers to get width
    /// // height, and input colorspace
    /// decoder.decode_headers().unwrap();
    ///
    /// let mut out = vec![0;decoder.output_buffer_size().unwrap()];
    /// // write into out
    /// decoder.decode_into(&mut out).unwrap();
    /// ```
    ///
    ///
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), DecodeErrors> {
        match self.state {
            DecodingState::DecodeHeaders { .. } => {
                self.decode_headers_internal()?;
            }
            DecodingState::DecodeScan { scan_start_position, append_snapshot } => {
                // Roll back any metadata that an inline marker pushed during
                // a previous scan attempt, then seek back to the start of the
                // scan so MCU decoding can be retried from the same anchor.
                append_snapshot.rollback(self);
                self.stream.set_position(scan_start_position)?;
            }
        }

        let expected_size = self.output_buffer_size().unwrap();

        if out.len() < expected_size {
            // too small of a size
            return Err(DecodeErrors::TooSmallOutput(expected_size, out.len()));
        }

        // ensure we don't touch anyone else's scratch space
        let out_len = core::cmp::min(out.len(), expected_size);
        let out = &mut out[0..out_len];

        if self.is_arithmetic {
            #[cfg(feature = "arith")]
            {
                if self.is_progressive {
                    self.decode_mcu_ycbcr_progressive::<BitStreamArithmetic>(out)
                } else {
                    self.decode_mcu_ycbcr_baseline::<BitStreamArithmetic>(out)
                }
            }
            #[cfg(not(feature = "arith"))]
            unreachable!();
        } else if self.is_progressive {
            self.decode_mcu_ycbcr_progressive::<BitStreamHuffman>(out)
        } else {
            self.decode_mcu_ycbcr_baseline::<BitStreamHuffman>(out)
        }
    }

    /// Read only headers from a jpeg image buffer
    ///
    /// This allows you to extract important information like
    /// image width and height without decoding the full image
    ///
    /// # Examples
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_jpeg::{JpegDecoder};
    ///
    /// let img_data = std::fs::read("a_valid.jpeg").unwrap();
    /// let mut decoder = JpegDecoder::new(ZCursor::new(&img_data));
    /// decoder.decode_headers().unwrap();
    ///
    /// println!("Total decoder dimensions are : {:?} pixels",decoder.dimensions());
    /// println!("Number of components in the image are {}", decoder.info().unwrap().components);
    /// ```
    /// # Errors
    /// See DecodeErrors enum for list of possible errors during decoding.
    ///
    /// If the reader runs out of data the error will satisfy
    /// [`is_recoverable_eof()`](crate::errors::DecodeErrors::is_recoverable_eof);
    /// the caller may retry after providing more data.
    pub fn decode_headers(&mut self) -> Result<(), DecodeErrors> {
        self.decode_headers_internal()?;
        Ok(())
    }

    /// Reset all state set during header parsing so that
    /// `decode_headers_internal` can be called again from scratch.
    // NB: fields here must stay in sync with `fn default()`.
    fn reset_header_state(&mut self) {
        self.info = ImageInfo::default();
        self.qt_tables = [None, None, None, None];
        self.entropy_tables.dc_huffman = [None, None, None, None];
        self.entropy_tables.ac_huffman = [None, None, None, None];
        self.components.clear();
        self.h_max = 1;
        self.v_max = 1;
        self.mcu_height = 0;
        self.mcu_width = 0;
        self.mcu_x = 0;
        self.mcu_y = 0;
        self.is_interleaved = false;
        self.is_progressive = false;
        self.spec_start = 0;
        self.spec_end = 0;
        self.succ_high = 0;
        self.succ_low = 0;
        self.num_scans = 0;
        self.scan_subsampled = false;
        self.input_colorspace = ColorSpace::YCbCr;
        self.z_order = [0; MAX_COMPONENTS];
        self.restart_interval = 0;
        self.todo = 0x7fff_ffff;
        self.headers_decoded = false;
        self.seen_sof = false;
        self.icc_data.clear();
        self.is_mjpeg = false;
        self.coeff = 1;
        self.extended_xmp_segments.clear();
        self.state = DecodingState::DecodeHeaders { resume_position: 0 };
        // Best-effort seek to start; may fail for non-seekable streams.
        let _ = self.stream.set_position(0);
    }

    /// Create a new decoder with the specified options to be used for decoding
    /// an image
    ///
    /// # Arguments
    /// - `buf`: The input buffer from where we will pull in compressed jpeg bytes from
    /// - `options`: Options specific to this decoder instance
    #[must_use]
    pub fn new_with_options(buf: T, options: DecoderOptions) -> JpegDecoder<T> {
        JpegDecoder::default(options, buf)
    }

    /// Set up-sampling routines in case an image is down sampled
    pub(crate) fn set_upsampling(&mut self) {
        // no sampling, return early
        // check if horizontal max ==1
        if self.h_max == self.v_max && self.h_max == 1 {
            return ;
        }

        for comp in &mut self.components {
            let hs = self.h_max / comp.horizontal_sample;
            let vs = self.v_max / comp.vertical_sample;

            let samp_factor = match (hs, vs) {
                (1, 1) => {
                    comp.sample_ratio = SampleRatios::None;
                    upsample_no_op
                }
                (2, 1) => {
                    comp.sample_ratio = SampleRatios::H;
                    choose_horizontal_samp_function(&self.options)
                }
                (1, 2) => {
                    comp.sample_ratio = SampleRatios::V;
                    choose_v_samp_function(&self.options)
                }
                (2, 2) => {
                    comp.sample_ratio = SampleRatios::HV;
                    choose_hv_samp_function(&self.options)
                }
                (hs, vs) => {
                    comp.sample_ratio = SampleRatios::Generic(hs, vs);
                    generic_sampler()
                }
            };
            comp.setup_upsample_scanline();
            comp.up_sampler = samp_factor;
        }

    }
    #[must_use]
    /// Get the width of the image as a u16
    ///
    /// The width lies between 1 and 65535
    pub(crate) fn width(&self) -> u16 {
        self.info.width
    }

    /// Get the height of the image as a u16
    ///
    /// The height lies between 1 and 65535
    #[must_use]
    pub(crate) fn height(&self) -> u16 {
        self.info.height
    }

    /// Get image dimensions as a tuple of width and height
    /// or `None` if the image hasn't been decoded.
    ///
    /// # Returns
    /// - `Some(width,height)`: Image dimensions
    /// -  None : The image headers haven't been decoded
    #[must_use]
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        return if self.headers_decoded {
            Some((self.info.width as usize, self.info.height as usize))
        } else {
            None
        };
    }
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct GainMapInfo {
    pub data: Vec<u8>
}

#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub(crate) struct ExtendedXmpSegment {
    pub(crate) offset: u32,
    pub(crate) total_size: u32,
    pub(crate) guid: Vec<u8>,
    pub(crate) data: Vec<u8>,
}

/// A struct representing Image Information
#[derive(Default, Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct ImageInfo {
    /// Width of the image
    pub width: u16,
    /// Height of image
    pub height: u16,
    /// PixelDensity
    pub pixel_density: u8,
    /// Start of frame markers
    pub sof: SOFMarkers,
    /// Horizontal sample
    pub x_density: u16,
    /// Vertical sample
    pub y_density: u16,
    /// Number of components
    pub components: u8,
    /// Gain Map information, useful for
    /// UHDR images
    pub gain_map_info: Vec<GainMapInfo>,
    /// Multi picture information, useful for
    /// UHDR images
    pub multi_picture_information: Option<Vec<u8>>,
    /// Exif Data
    pub exif_data: Option<Vec<u8>>,
    /// XMP Data
    pub xmp_data: Option<Vec<u8>>,
    /// IPTC Data
    pub iptc_data: Option<Vec<u8>>,
    /// Extended XMP Data
    pub extended_xmp: Option<Vec<u8>>,
    /// Extended XMP Guid
    pub extended_xmp_guid: Option<Vec<u8>>,
    /// Image sub-sampling ratio
    pub sample_ratio: SampleRatios,
    /// The offset at which Multi picture information was found
    pub multi_picture_information_offset: Option<u64>,
}

impl ImageInfo {
    /// Set width of the image
    ///
    /// Found in the start of frame
    pub(crate) fn set_width(&mut self, width: u16) {
        self.width = width;
    }

    /// Set height of the image
    ///
    /// Found in the start of frame
    pub(crate) fn set_height(&mut self, height: u16) {
        self.height = height;
    }

    /// Set the image density
    ///
    /// Found in the start of frame
    pub(crate) fn set_density(&mut self, density: u8) {
        self.pixel_density = density;
    }

    /// Set image Start of frame marker
    ///
    /// found in the Start of frame header
    pub(crate) fn set_sof_marker(&mut self, marker: SOFMarkers) {
        self.sof = marker;
    }

    /// Set image x-density(dots per pixel)
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_x(&mut self, sample: u16) {
        self.x_density = sample;
    }

    /// Set image y-density
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_y(&mut self, sample: u16) {
        self.y_density = sample;
    }
}
