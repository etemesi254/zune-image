//! Main image logic.
#![allow(clippy::doc_markdown)]

use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::color_convert::choose_ycbcr_to_rgb_convert_func;
use crate::components::{ComponentID, Components, SampleRatios};
use crate::errors::{DecodeErrors, UnsupportedSchemes};
use crate::headers::{parse_app14, parse_dqt, parse_huffman, parse_sos, parse_start_of_frame};
use crate::huffman::HuffmanTable;
use crate::idct::choose_idct_func;
use crate::marker::Marker;
use crate::misc::SOFMarkers;
use crate::upsampler::{
    choose_horizontal_samp_function, choose_hv_samp_function, choose_v_samp_function
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
///     `y,cb,cr`
///
/// 3. `&mut [u8]` is zero initialized
///
/// 4. `&mut usize` points to the position in the array where new values should
/// be used
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

/// A JPEG Decoder Instance.
#[allow(clippy::upper_case_acronyms)]
pub struct JpegDecoder<'a>
{
    /// Struct to hold image information from SOI
    pub(crate) info:              ImageInfo,
    ///  Quantization tables, will be set to none and the tables will
    /// be moved to `components` field
    pub(crate) qt_tables:         [Option<[i32; 64]>; MAX_COMPONENTS],
    /// DC Huffman Tables with a maximum of 4 tables for each  component
    pub(crate) dc_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    /// AC Huffman Tables with a maximum of 4 tables for each component
    pub(crate) ac_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    /// Image components, holds information like DC prediction and quantization
    /// tables of a component
    pub(crate) components:        Vec<Components>,
    /// maximum horizontal component of all channels in the image
    pub(crate) h_max:             usize,
    // maximum vertical component of all channels in the image
    pub(crate) v_max:             usize,
    /// mcu's  width (interleaved scans)
    pub(crate) mcu_width:         usize,
    /// MCU height(interleaved scans
    pub(crate) mcu_height:        usize,
    /// Number of MCU's in the x plane
    pub(crate) mcu_x:             usize,
    /// Number of MCU's in the y plane
    pub(crate) mcu_y:             usize,
    /// Is the image interleaved?
    pub(crate) is_interleaved:    bool,
    pub(crate) sub_sample_ratio:  SampleRatios,
    /// Image input colorspace, should be YCbCr for a sane image, might be
    /// grayscale too
    pub(crate) input_colorspace:  ColorSpace,
    // Progressive image details
    /// Is the image progressive?
    pub(crate) is_progressive:    bool,

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
    // Function pointers, for pointy stuff.
    /// Dequantize and idct function
    // This is determined at runtime which function to run, statically it's
    // initialized to a platform independent one and during initialization
    // of this struct, we check if we can switch to a faster one which
    // depend on certain CPU extensions.
    pub(crate) idct_func: IDCTPtr,
    // Color convert function which acts on 16 YCbCr values
    pub(crate) color_convert_16: ColorConvert16Ptr,
    pub(crate) z_order:          [usize; MAX_COMPONENTS],
    /// restart markers
    pub(crate) restart_interval: usize,
    pub(crate) todo:             usize,
    // decoder options
    pub(crate) options:          DecoderOptions,
    // byte-stream
    pub(crate) stream:           ZByteReader<'a>,
    // Indicate whether headers have been decoded
    pub(crate) headers_decoded:  bool
}

impl<'a> JpegDecoder<'a>
{
    fn default(options: DecoderOptions, buffer: &'a [u8]) -> Self
    {
        let color_convert =
            choose_ycbcr_to_rgb_convert_func(ColorSpace::RGB, options.use_unsafe).unwrap();
        JpegDecoder {
            info: ImageInfo::default(),
            qt_tables: [None, None, None, None],
            dc_huffman_tables: [None, None, None, None],
            ac_huffman_tables: [None, None, None, None],
            components: vec![],
            // Interleaved information
            h_max: 1,
            v_max: 1,
            mcu_height: 0,
            mcu_width: 0,
            mcu_x: 0,
            mcu_y: 0,
            is_interleaved: false,
            sub_sample_ratio: SampleRatios::None,

            // Progressive information
            is_progressive: false,
            spec_start: 0,
            spec_end: 0,
            succ_high: 0,
            succ_low: 0,
            num_scans: 0,

            // Function pointers
            idct_func: choose_idct_func(options.use_unsafe),
            color_convert_16: color_convert,

            // Colorspace
            input_colorspace: ColorSpace::YCbCr,

            z_order: [0; MAX_COMPONENTS],
            restart_interval: 0,
            todo: 0x7fff_ffff,
            // options
            options,
            stream: ZByteReader::new(buffer),
            headers_decoded: false
        }
    }
    /// Decode a buffer already in memory
    ///
    /// The buffer should be a valid jpeg file, perhaps created by the command
    /// `std:::fs::read()` or a JPEG file downloaded from the internet.
    ///
    /// # Errors
    /// See DecodeErrors for an explanation
    pub fn decode(&mut self) -> Result<Vec<u8>, DecodeErrors>
    {
        self.decode_internal()
    }

    /// Create a new Decoder instance
    #[must_use]
    #[allow(clippy::new_without_default)]
    pub fn new(stream: &'a [u8]) -> JpegDecoder
    {
        JpegDecoder::default(DecoderOptions::default(), stream)
    }

    /// Returns the image information
    ///
    /// This **must** be called after a subsequent call to `decode_file` or
    /// `decode_buffer` otherwise it will return None
    ///
    #[must_use]
    pub fn info(&self) -> Option<ImageInfo>
    {
        // we check for fails to that call by comparing what we have to the default, if
        // it's default we assume that the caller failed to uphold the
        // guarantees. We can be sure that an image cannot be the default since
        // its a hard panic in-case width or height are set to zero.
        if !self.headers_decoded
        {
            return None;
        }

        return Some(self.info.clone());
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
    fn decode_headers_internal(&mut self) -> Result<(), DecodeErrors>
    {
        if self.headers_decoded
        {
            warn!("Headers decoded!");
            return Ok(());
        }
        // First two bytes should be jpeg soi marker
        let magic_bytes = self.stream.get_u16_be_err()?;

        let mut last_byte = 0;
        let mut bytes_before_marker = 0;

        if magic_bytes != 0xffd8
        {
            return Err(DecodeErrors::IllegalMagicBytes(magic_bytes));
        }

        loop
        {
            // read a byte
            let m = self.stream.get_u8_err()?;
            // Last byte should be 0xFF to confirm existence of a marker since markers look
            // like OxFF(some marker data)
            if last_byte == 0xFF
            {
                let marker = Marker::from_u8(m);

                if let Some(n) = marker
                {
                    if bytes_before_marker > 3
                    {
                        if self.options.strict_mode
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

                    self.parse_marker_inner(n)?;

                    if n == Marker::SOS
                    {
                        self.headers_decoded = true;
                        return Ok(());
                    }
                }
                else
                {
                    bytes_before_marker = 0;

                    warn!("Marker 0xFF{:X} not known", m);

                    let length = self.stream.get_u16_be_err()?;

                    if length < 2
                    {
                        return Err(DecodeErrors::Format(format!(
                            "Found a marker with invalid length : {length}"
                        )));
                    }

                    warn!("Skipping {} bytes", length - 2);
                    self.stream.skip((length - 2) as usize);
                }
            }
            last_byte = m;
            bytes_before_marker += 1;
        }
    }
    pub(crate) fn parse_marker_inner(&mut self, m: Marker) -> Result<(), DecodeErrors>
    {
        match m
        {
            Marker::SOF(0 | 1 | 2) =>
            {
                let marker = {
                    // choose marker
                    if m == Marker::SOF(0) || m == Marker::SOF(1)
                    {
                        SOFMarkers::BaselineDct
                    }
                    else
                    {
                        self.is_progressive = true;
                        SOFMarkers::ProgressiveDctHuffman
                    }
                };

                info!("Image encoding scheme =`{:?}`", marker);
                // get components
                parse_start_of_frame(marker, self)?;
            }
            // Start of Frame Segments not supported
            Marker::SOF(v) =>
            {
                let feature = UnsupportedSchemes::from_int(v);

                if let Some(feature) = feature
                {
                    return Err(DecodeErrors::Unsupported(feature));
                }

                return Err(DecodeErrors::Format("Unsupported image format".to_string()));
            }
            //APP(0) segment
            Marker::APP(0 | 1) =>
            {
                let length = self.stream.get_u16_be_err()?;

                if length < 2
                {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                // skip for now
                self.stream.skip((length - 2) as usize);

                //parse_app(buf, m, &mut self.info)?;
            }
            // Quantization tables
            Marker::DQT =>
            {
                parse_dqt(self)?;
            }
            // Huffman tables
            Marker::DHT =>
            {
                parse_huffman(self)?;
            }
            // Start of Scan Data
            Marker::SOS =>
            {
                parse_sos(self)?;

                // break after reading the start of scan.
                // what follows is the image data
                return Ok(());
            }
            Marker::EOI => return Err(DecodeErrors::FormatStatic("Premature End of image")),

            Marker::DAC | Marker::DNL =>
            {
                return Err(DecodeErrors::Format(format!(
                    "Parsing of the following header `{m:?}` is not supported,\
                                cannot continue"
                )));
            }
            Marker::DRI =>
            {
                info!("DRI marker present");

                if self.stream.get_u16_be_err()? != 4
                {
                    return Err(DecodeErrors::Format(
                        "Bad DRI length, Corrupt JPEG".to_string()
                    ));
                }

                self.restart_interval = usize::from(self.stream.get_u16_be_err()?);
                self.todo = self.restart_interval;
            }
            Marker::APP(14) =>
            {
                parse_app14(self)?;
            }
            _ =>
            {
                warn!(
                    "Capabilities for processing marker \"{:?}\" not implemented",
                    m
                );

                let length = self.stream.get_u16_be_err()?;

                if length < 2
                {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                warn!("Skipping {} bytes", length - 2);
                self.stream.skip((length - 2) as usize);
            }
        }
        Ok(())
    }

    /// Get the output colorspace the image pixels will be decoded into
    #[must_use]
    pub fn get_output_colorspace(&self) -> ColorSpace
    {
        return self.options.out_colorspace;
    }

    fn decode_internal(&mut self) -> Result<Vec<u8>, DecodeErrors>
    {
        self.decode_headers_internal()?;

        if self.is_progressive
        {
            self.decode_mcu_ycbcr_progressive()
        }
        else
        {
            self.decode_mcu_ycbcr_baseline()
        }
    }
    /// Read only headers from a jpeg image buffer
    ///
    /// This allows you to extract important information like
    /// image width and height without decoding the full image
    ///
    /// # Examples
    /// ```no_run
    /// use zune_jpeg::{JpegDecoder};
    ///
    /// let img_data = std::fs::read("a_valid.jpeg").unwrap();
    /// let mut decoder = JpegDecoder::new(&img_data);
    /// decoder.decode_headers().unwrap();
    ///
    /// println!("Total decoder dimensions are : {} pixels",usize::from(decoder.width()) * usize::from(decoder.height()));
    /// println!("Number of components in the image are {}", decoder.info().unwrap().components);
    /// ```
    /// # Errors
    /// See DecodeErrors enum for list of possible errors during decoding
    pub fn decode_headers(&mut self) -> Result<(), DecodeErrors>
    {
        self.decode_headers_internal()?;
        Ok(())
    }
    /// Create a new decoder with the specified options to be used for decoding
    /// an image
    #[must_use]
    pub fn new_with_options(options: DecoderOptions, buf: &'a [u8]) -> JpegDecoder
    {
        JpegDecoder::default(options, buf)
    }

    /// Set up-sampling routines in case an image is down sampled
    pub(crate) fn set_upsampling(&mut self) -> Result<(), DecodeErrors>
    {
        // no sampling, return early
        // check if horizontal max ==1
        if self.h_max == self.v_max && self.h_max == 1
        {
            return Ok(());
        }

        // match for other ratios
        match (self.h_max, self.v_max)
        {
            (2, 1) =>
            {
                self.sub_sample_ratio = SampleRatios::H;
                // horizontal sub-sampling
                info!("Horizontal sub-sampling (2,1)");

                let up_sampler = choose_horizontal_samp_function(self.options.use_unsafe);

                self.components[1..].iter_mut().for_each(|x| {
                    x.up_sampler = up_sampler;
                    x.setup_upsample_scanline(self.h_max, self.v_max);
                });
            }
            (1, 2) =>
            {
                self.sub_sample_ratio = SampleRatios::V;
                // Vertical sub-sampling
                info!("Vertical sub-sampling (1,2)");

                self.components[1..].iter_mut().for_each(|x| {
                    x.up_sampler = choose_v_samp_function(self.options.use_unsafe);
                    x.setup_upsample_scanline(self.h_max, self.v_max);
                });
            }
            (2, 2) =>
            {
                self.sub_sample_ratio = SampleRatios::HV;
                // vertical and horizontal sub sampling
                info!("Vertical and horizontal sub-sampling(2,2)");

                self.components[1..].iter_mut().for_each(|x| {
                    x.up_sampler = choose_hv_samp_function(self.options.use_unsafe);
                    x.setup_upsample_scanline(self.h_max, self.v_max);
                });
            }
            (_, _) =>
            {
                // no op. Do nothing
                // Jokes , panic...
                return Err(DecodeErrors::Format(
                    "Unknown down-sampling method, cannot continue".to_string()
                ));
            }
        }

        return Ok(());
    }
    #[must_use]
    /// Get the width of the image as a u16
    ///
    /// The width lies between 0 and 65535
    pub fn width(&self) -> u16
    {
        self.info.width
    }

    /// Get the height of the image as a u16
    ///
    /// The height lies between 0 and 65535
    #[must_use]
    pub fn height(&self) -> u16
    {
        self.info.height
    }

    /// Check that all components have the correct width and height
    /// before continuing to decode
    ///
    /// This helps to identify some corrupt images that may have invalid widths and heights and error out
    /// before trying to decode.
    pub(crate) fn check_component_dimensions(&self) -> Result<(), DecodeErrors>
    {
        // find  y component
        let y_comp = self
            .components
            .iter()
            .find(|c| c.component_id == ComponentID::Y)
            .ok_or(DecodeErrors::FormatStatic(
                "Could not find Y component for the image"
            ))?;

        let y_width = y_comp.width_stride;
        let cb_cr_width = y_width / self.h_max;

        for comp in &self.components
        {
            if comp.component_id == ComponentID::Y
            {
                continue;
            }

            if comp.width_stride != cb_cr_width
            {
                return Err(DecodeErrors::Format(format!("Invalid image width and height stride for component {:?}, expected {}, but found {}", comp.component_id, cb_cr_width, comp.width_stride)));
            }

            if (comp.horizontal_sample != 1 || comp.vertical_sample != 1)
                && comp.component_id != ComponentID::Y
            {
                return Err(DecodeErrors::Format(format!(
                    "Invalid component sample for component {:?}, expected (1,1), found ({},{})",
                    comp.component_id, comp.vertical_sample, comp.horizontal_sample
                )));
            }
        }

        Ok(())
    }
}

/// A struct representing Image Information
#[derive(Default, Clone, Eq, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct ImageInfo
{
    /// Width of the image
    pub width:         u16,
    /// Height of image
    pub height:        u16,
    /// PixelDensity
    pub pixel_density: u8,
    /// Start of frame markers
    pub sof:           SOFMarkers,
    /// Horizontal sample
    pub x_density:     u16,
    /// Vertical sample
    pub y_density:     u16,
    /// Number of components
    pub components:    u8
}

impl ImageInfo
{
    /// Set width of the image
    ///
    /// Found in the start of frame

    pub(crate) fn set_width(&mut self, width: u16)
    {
        self.width = width;
    }

    /// Set height of the image
    ///
    /// Found in the start of frame

    pub(crate) fn set_height(&mut self, height: u16)
    {
        self.height = height;
    }

    /// Set the image density
    ///
    /// Found in the start of frame

    pub(crate) fn set_density(&mut self, density: u8)
    {
        self.pixel_density = density;
    }

    /// Set image Start of frame marker
    ///
    /// found in the Start of frame header

    pub(crate) fn set_sof_marker(&mut self, marker: SOFMarkers)
    {
        self.sof = marker;
    }

    /// Set image x-density(dots per pixel)
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_x(&mut self, sample: u16)
    {
        self.x_density = sample;
    }

    /// Set image y-density
    ///
    /// Found in the APP(0) marker
    #[allow(dead_code)]
    pub(crate) fn set_y(&mut self, sample: u16)
    {
        self.y_density = sample;
    }
}
