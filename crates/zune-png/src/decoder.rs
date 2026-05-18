/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! A fast, correct, and safe PNG and APNG decoder.
//!
//! This crate provides a flexible decoder for Portable Network Graphics (PNG) images,
//! including full support for Animated PNGs (APNG). It is designed to be highly performant
//! while correctly handling edge cases, various bit depths (8-bit and 16-bit), and complex
//! APNG compositing lifecycles.
//!
//! # Features
//! - **Standard PNG Support:** Decodes static PNGs across all standard color types and depths.
//! - **Animated PNG (APNG) Support:** Provides tools to extract, dispose, and blend animated frames correctly.
//! - **16-bit to 8-bit Stripping:** Optional native stripping of 16-bit images down to 8-bit.
//! - **Endianness Handling:** Automatically manages byte swapping for 16-bit depth images on Little Endian systems.
//! - **Zero-Allocation Decoding:** Ability to decode directly into pre-allocated buffers.
//!
//! # Basic Usage (Static Images)
//!
//! Decoding a standard, single-frame PNG is straightforward. You initialize the decoder,
//! read the headers to understand the image properties, and then decode the pixels.
//!
//! ```no_run
//! use zune_core::bytestream::ZCursor;
//! use zune_core::result::DecodingResult;
//! use zune_png::PngDecoder;
//!
//! // 1. Provide the raw bytes to the decoder
//! let file_data = std::fs::read("image.png").unwrap();
//! let mut decoder = PngDecoder::new(ZCursor::new(&file_data));
//!
//! // 2. Decode headers to populate image info
//! decoder.decode_headers().unwrap();
//!
//! // 3. Retrieve useful metadata
//! let (width, height) = decoder.dimensions().unwrap();
//! let colorspace = decoder.colorspace().unwrap();
//!
//! // 4. Decode the image data
//! match decoder.decode().unwrap() {
//!     DecodingResult::U8(pixels) => {
//!         // Handle standard 8-bit per channel images
//!         println!("Decoded {} bytes", pixels.len());
//!     }
//!     DecodingResult::U16(pixels) => {
//!         // Handle 16-bit per channel images
//!         println!("Decoded {} 16-bit samples", pixels.len());
//!     }
//!     _ => unreachable!(),
//! }
//! ```
//!
//! # Animated Images (APNG)
//!
//! APNG files store multiple frames that must be composited sequentially onto a main canvas.
//! The APNG specification defines a strict lifecycle for rendering frames:
//! 1. **Disposal**: The canvas is modified based on the *previous* frame's disposal rules (e.g., cleared or reverted).
//! 2. **Blending**: The *current* frame is drawn onto the canvas using its blend operation.
//!
//! To correctly decode an APNG, you must maintain the state of the main canvas and a backup canvas
//! across iterations. The `post_process_image_apng` utility function correctly abstracts the math and
//! state management needed for handling `DisposeOp` and `BlendOp` across both `u8` and `u16` depths.
//!
//! ## Example: Decoding an APNG
//!
//! ```no_run
//! use zune_core::bytestream::ZCursor;
//! use zune_core::result::DecodingResult;
//! use zune_png::{PngDecoder, FrameInfo, post_process_image_apng};
//!
//! let file_data = std::fs::read("animated.png").unwrap();
//! let mut decoder = PngDecoder::new(ZCursor::new(&file_data));
//!
//! decoder.decode_headers().unwrap();
//!
//! if decoder.is_animated() {
//!     let info = decoder.info().unwrap().clone();
//!     let colorspace = decoder.colorspace().unwrap();
//!     let components = colorspace.num_components();
//!
//!     // 1. Allocate buffers for the entire canvas size
//!     let buffer_size = info.width * info.height * components;
//!     let mut output_canvas = vec![0u8; buffer_size];
//!     let mut backup_canvas = vec![0u8; buffer_size];
//!
//!     // 2. Track the previous frame's rules for proper disposal
//!     let mut prev_frame_info: Option<FrameInfo> = None;
//!
//!     // 3. Loop through all frames
//!     while decoder.more_frames() {
//!         decoder.decode_headers().unwrap();
//!
//!         // Clone the current frame info so we can store it at the end of the loop
//!         let frame = decoder.frame_info().unwrap().clone();
//!
//!         match decoder.decode().unwrap() {
//!             DecodingResult::U8(frame_pixels) => {
//!                 // 4. Run the compositing lifecycle
//!                 post_process_image_apng(
//!                     &info,
//!                     colorspace,
//!                     &frame,
//!                     prev_frame_info.as_ref(), // Pass previous rules for disposal
//!                     &frame_pixels,
//!                     &mut backup_canvas,       // Used internally for DisposeOp::Previous
//!                     &mut output_canvas,       // The main rendering surface
//!                     None                      // Optional gamma override
//!                 ).unwrap();
//!
//!                 // `output_canvas` now contains the fully composited current frame!
//!                 // You can save it, display it, or push it to a video encoder here.
//!
//!                 // 5. Save current frame info to act as disposal instructions for the next loop
//!                 prev_frame_info = Some(frame);
//!             }
//!             DecodingResult::U16(_) => {
//!                 // post_process_image_apng is generic and supports U16 as well!
//!                 // (Implementation omitted for brevity)
//!             }
//!             _ => {}
//!         }
//!     }
//! }
//! ```
//! # Zero-Allocation Decoding
//!
//! For performance-critical applications, or when processing many images in a loop, you can avoid
//! allocations by decoding directly into a pre-allocated slice using `decode_into()`.
//!
//! ```no_run
//! use zune_core::bytestream::ZCursor;
//! use zune_png::PngDecoder;
//!
//! let file_data = std::fs::read("image.png").unwrap();
//! let mut decoder = PngDecoder::new(ZCursor::new(&file_data));
//!
//! decoder.decode_headers().unwrap();
//!
//! // Calculate the exact bytes needed for the output
//! let buffer_size = decoder.output_buffer_size().unwrap();
//! let mut output = vec![0u8; buffer_size]; // Allocate once!
//!
//! // Decode directly into the slice
//! decoder.decode_into(&mut output).unwrap();
//! ```
use alloc::vec;
use alloc::vec::Vec;

use zune_core::bit_depth::{BitDepth, ByteEndian};
use zune_core::bytestream::{ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;

use crate::apng::{ActlChunk, FrameInfo, SingleFrame};
use crate::constants::PNG_SIGNATURE;
use crate::decoder::DecodingState::{DecodingBody, DecodingHeaders};
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngDecodeErrors;
use crate::error::PngDecodeErrors::GenericStatic;

use crate::options::default_chunk_handler;
use crate::utils::{convert_u16_to_u8_slice, is_le};

/// A palette entry.
///
/// The alpha field is used if the image has a tRNS
/// chunk and pLTE chunk.
///
// NB: (cae), using typed structs leads to a slow down so just use an array of u8,
// slowdown may be due to LLVM not decoding it correctly and converting it to memcpy/mov's where
// it can
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub(crate) struct PLTEEntry(pub(crate) [u8; 4]);

impl Default for PLTEEntry {
    fn default() -> Self {
        // but a tRNS chunk may contain fewer values than there are palette entries.
        // In this case, the alpha value for all remaining palette entries is assumed to be 255
        PLTEEntry([0, 0, 0, 255])
    }
}

#[derive(Copy, Clone)]
pub(crate) struct PngChunk {
    pub length: usize,
    pub chunk_type: PngChunkType,
    pub chunk: [u8; 4],
}

/// Time information data
///
/// Extracted from tIME chunk
#[derive(Debug, Default, Copy, Clone)]
pub struct TimeInfo {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// iTXt details
///
/// UTF-8 encoded text
///
/// Extracted from iXTt chunk where present
#[derive(Clone)]
pub struct ItxtChunk {
    pub keyword: Vec<u8>,
    pub text: Vec<u8>,
}

/// tEXt chunk details
///
/// Latin-1 character set
///
/// Extracted from tEXt chunk where present
#[derive(Clone)]
pub struct TextChunk {
    pub keyword: Vec<u8>,
    pub text: Vec<u8>,
}

/// zTxt details
///
/// Extracted from zTXt chunk where present
#[derive(Clone)]
pub struct ZtxtChunk {
    pub keyword: Vec<u8>,
    /// Uncompressed text
    pub text: Vec<u8>,
}
#[derive(Clone, Default, Copy)]
pub struct ChrmInfo {
    pub white_point_x: u32,
    pub white_point_y: u32,
    pub red_x: u32,
    pub red_y: u32,
    pub green_x: u32,
    pub green_y: u32,
    pub blue_x: u32,
    pub blue_y: u32,
}
#[derive(Clone, Default, Copy)]

pub struct PhysInfo {
    pub ppu_x: u32,
    pub ppu_y: u32,
    pub unit_specifier: u8,
}
#[derive(Clone, Default, Copy, Debug)]
pub struct CicpInfo {
    pub color_primaries: u8,
    pub transfer_function: u8,
    pub matrix_coefficients: u8,
    pub video_full_range_flag: u8,
}

#[derive(Clone, Default, Copy)]
pub struct ClliInfo {
    pub max_cll: u32,
    pub max_fall: u32,
}

/// Represents PNG information that can be extracted
/// from a png file.
#[derive(Default, Clone)]
pub struct PngInfo {
    /// Image width
    pub width: usize,
    /// Image height
    pub height: usize,
    /// Image gamma
    pub gamma: Option<f32>,
    /// Image interlace method
    pub interlace_method: InterlaceMethod,
    /// Image time info
    pub time_info: Option<TimeInfo>,
    /// Image exif data
    pub exif: Option<Vec<u8>>,
    /// Icc profile
    pub icc_profile: Option<Vec<u8>>,
    /// UTF-8 encoded text chunk
    pub itxt_chunk: Vec<ItxtChunk>,
    /// ztxt chunk
    pub ztxt_chunk: Vec<ZtxtChunk>,
    /// tEXt chunk
    pub text_chunk: Vec<TextChunk>,
    /// cLLI chunk (Content Light Level Information)
    pub clli_info: Option<ClliInfo>,
    ///  pHYs chunk (Physical Pixel Dimensions)
    pub phys_info: Option<PhysInfo>,
    /// cICP chunk (Coding-Independent Code Points)
    pub cicp_info: Option<CicpInfo>,
    pub chrm_info: Option<ChrmInfo>,
    pub sbit_info: Option<[u8; 4]>,
    // no need to expose these ones
    pub(crate) depth: u8,
    // use bit_depth
    pub(crate) color: PngColor,
    // use get_colorspace
    pub(crate) component: u8,
    // use get_colorspace().num_components()
    pub(crate) filter_method: FilterMethod, // for internal use,no need to expose
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DecodingState {
    DecodingHeaders,
    DecodingBody,
    Done,
}

/// A PNG decoder instance.
///
/// This is the main decoder for png image decoding.
///
/// Instantiate the decoder with either the [new](PngDecoder::new)
/// or [new_with_options](PngDecoder::new_with_options) and
/// using either  of the [`decode_raw`](PngDecoder::decode_into) or
/// [`decode`](PngDecoder::decode) will return pixels present in that image
///
/// # Note
/// The decoder currently expands images less than 8 bits per pixels to 8 bits per pixel
/// if this is not desired, then I'd suggest another png decoder
///
/// To get extra details such as exif data and ICC profile if present, use [`get_info`](PngDecoder::info)
/// and access the relevant fields exposed
pub struct PngDecoder<T> {
    pub(crate) stream: ZReader<T>,
    pub(crate) options: DecoderOptions,
    pub(crate) png_info: PngInfo,
    pub(crate) palette: Vec<PLTEEntry>,
    pub(crate) frames: Vec<SingleFrame>,
    pub(crate) actl_info: Option<ActlChunk>,
    pub(crate) non_parsed_header: Option<PngChunk>,
    pub(crate) decoding_state: DecodingState,
    pub(crate) seen_headers: bool,
    pub(crate) trns_bytes: [u16; 4],
    pub(crate) seen_hdr: bool,
    // number of fCTL frames seen, tracks how many animations have been decoded,
    pub(crate) num_fctl_seen: usize,
    pub(crate) seen_ptle: bool,
    pub(crate) seen_trns: bool,
    pub(crate) seen_iend: bool,
    pub(crate) current_frame: usize,
    pub(crate) current_idat_bytes_left: usize,
}

impl<T: ZByteReaderTrait> PngDecoder<T> {
    /// Create a new PNG decoder
    ///
    /// # Arguments
    ///
    /// * `data`: The raw bytes of a png encoded file
    ///
    /// returns: PngDecoder
    ///
    /// The decoder settings are set to be default which is
    ///  strict mode + intrinsics
    pub fn new(data: T) -> PngDecoder<T> {
        let default_opt = DecoderOptions::default();

        PngDecoder::new_with_options(data, default_opt)
    }
    /// Create a new decoder with the specified options
    ///
    /// # Arguments
    ///
    /// * `data`: Raw encoded png file contents
    /// * `options`:  The custom options for this decoder
    ///
    /// returns: PngDecoder
    ///
    #[allow(unused_mut, clippy::redundant_field_names)]
    pub fn new_with_options(data: T, options: DecoderOptions) -> PngDecoder<T> {
        PngDecoder {
            seen_hdr: false,
            stream: ZReader::new(data),
            options: options,
            palette: Vec::new(),
            png_info: PngInfo::default(),
            non_parsed_header: None,
            decoding_state: DecodingState::DecodingHeaders,
            actl_info: None,
            frames: vec![],
            seen_ptle: false,
            num_fctl_seen: 1,
            seen_trns: false,
            seen_iend: false,
            trns_bytes: [0; 4],
            current_frame: 0,
            current_idat_bytes_left: 0,
            seen_headers: false,
        }
    }

    /// Get the global image dimensions (width, height) in pixels.
    ///
    /// # Animated PNGs (APNG)
    /// If the image is animated, this returns the dimensions of the **global output canvas**,
    /// *not* the dimensions of the current frame being decoded. Individual frames can be smaller
    /// than the canvas. To get the dimensions of a specific frame, use [`frame_info()`](Self::frame_info).
    ///
    /// # Returns
    /// - `Some((width, height))`
    /// - `None`: If the image headers haven't been decoded yet (`IHDR` chunk not reached)
    ///   or there was an error decoding them.
    pub fn dimensions(&self) -> Option<(usize, usize)> {
        if !self.seen_hdr {
            return None;
        }

        Some((self.png_info.width, self.png_info.height))
    }
    /// Return the depth of the image
    ///
    /// Bit depths less than 8 will be returned as [`BitDepth::Eight`](zune_core::bit_depth::BitDepth::Eight)
    ///
    /// # Returns
    /// - `Some(depth)`:  The bit depth of the image.
    /// - `None`: The header wasn't decoded hence the depth wasn't discovered.
    pub const fn depth(&self) -> Option<BitDepth> {
        if !self.seen_hdr {
            return None;
        }
        match self.png_info.depth {
            1 | 2 | 4 | 8 => Some(BitDepth::Eight),
            16 => Some(BitDepth::Sixteen),
            _ => unreachable!(),
        }
    }
    /// Get the colorspace that the *decoded* output pixels will be in.
    ///
    /// This decoder handles internal format conversions for you. It guarantees the returned
    /// colorspace matches the actual decoded byte layout, which may differ from the raw file:
    ///
    /// - **Transparency (`tRNS`):** If a palette, grayscale, or RGB image contains a `tRNS` chunk,
    ///   the decoder automatically applies it. The returned colorspace will be upgraded to include
    ///   Alpha (e.g., `RGB` becomes `RGBA`).
    /// - **Forced Alpha:** If `DecoderOptions::png_set_add_alpha_channel(true)` is set,
    ///   this will consistently return an Alpha-variant colorspace.
    ///
    /// # Returns
    ///  - `Some(ColorSpace)`: The final colorspace of the decoded bytes.
    ///  - `None`: If the image headers haven't been decoded yet.
    pub const fn colorspace(&self) -> Option<ColorSpace> {
        if !self.seen_hdr {
            return None;
        }
        if self.options.png_get_add_alpha_channel() {
            return match self.png_info.color {
                PngColor::Luma | PngColor::LumaA => Some(ColorSpace::LumaA),
                PngColor::Palette | PngColor::RGB | PngColor::RGBA => Some(ColorSpace::RGBA),
                PngColor::Unknown => unreachable!(),
            };
        }
        if !self.seen_trns {
            match self.png_info.color {
                PngColor::Palette => Some(ColorSpace::RGB),
                PngColor::Luma => Some(ColorSpace::Luma),
                PngColor::LumaA => Some(ColorSpace::LumaA),
                PngColor::RGB => Some(ColorSpace::RGB),
                PngColor::RGBA => Some(ColorSpace::RGBA),
                PngColor::Unknown => unreachable!(),
            }
        } else {
            // for tRNS chunks, RGB=>RGBA
            // Luma=>LumaA, but if we are already in RGB and RGBA, just return
            // them
            match self.png_info.color {
                PngColor::Palette | PngColor::RGB => Some(ColorSpace::RGBA),
                PngColor::Luma => Some(ColorSpace::LumaA),
                PngColor::LumaA => Some(ColorSpace::LumaA),
                PngColor::RGBA => Some(ColorSpace::RGBA),
                _ => unreachable!(),
            }
        }
    }
    /// Returns `true` if the image is an Animated PNG (APNG) containing multiple frames.
    ///
    /// **Important:** This method relies on header information. You must call [`decode_headers`](Self::decode_headers)
    /// before calling this, otherwise it will always return `false`.
    ///
    /// # Note on APNG
    /// The APNG specification is an extension of the standard PNG format. Animated files
    /// contain a global animation control chunk (`acTL`) and multiple frame-specific chunks.
    /// If this returns `true`, you should use a `while decoder.more_frames()` loop to extract the sequence.
    pub fn is_animated(&self) -> bool {
        self.actl_info.is_some()
    }

    /// Returns `true` if there are still unread frames remaining in an animated image.
    ///
    /// This function evaluates the number of frame control (`fcTL`) chunks seen so far
    /// against the total frame count declared in the animation header.
    ///
    /// It is designed to be used as the condition in a `while` loop when decoding APNGs.
    /// For static (non-animated) images, or if all frames have already been decoded, this
    /// will safely return `false`.
    ///
    /// # Example
    /// ```no_run
    /// // Assuming decoder is initialized and headers are decoded
    /// while decoder.more_frames() {
    ///     decoder.decode_headers().unwrap();
    ///     let current_frame_info = decoder.frame_info().unwrap();
    ///     let pixels = decoder.decode().unwrap();
    /// }
    /// ```
    pub fn more_frames(&self) -> bool {
        if let Some(actl) = self.actl_info.as_ref() {
            // From https://wiki.mozilla.org/APNG_Specification

            // `num_frames` indicates the total number of frames in the animation. This must equal the number of `fcTL` chunks.
            // `num_fctl_seen` counts the number of fctl chunks seen
            return self.num_fctl_seen != actl.num_frames as usize;
        }
        false
    }

    pub(crate) fn read_chunk_header(&mut self) -> Result<PngChunk, PngDecodeErrors> {
        // Format is length - chunk type - [data] -  crc chunk, load crc chunk now
        let chunk_length = self.stream.get_u32_be_err()? as usize;
        let chunk_type_int = self.stream.get_u32_be_err()?.to_be_bytes();

        let chunk_type = match &chunk_type_int {
            b"IHDR" => PngChunkType::IHDR,
            b"tRNS" => PngChunkType::tRNS,
            b"PLTE" => PngChunkType::PLTE,
            b"IDAT" => PngChunkType::IDAT,
            b"IEND" => PngChunkType::IEND,
            b"pHYs" => PngChunkType::pHYs,
            b"tIME" => PngChunkType::tIME,
            b"gAMA" => PngChunkType::gAMA,
            b"acTL" => PngChunkType::acTL,
            b"fcTL" => PngChunkType::fcTL,
            b"iCCP" => PngChunkType::iCCP,
            b"iTXt" => PngChunkType::iTXt,
            b"eXIf" => PngChunkType::eXIf,
            b"zTXt" => PngChunkType::zTXt,
            b"tEXt" => PngChunkType::tEXt,
            b"fdAT" => PngChunkType::fdAT,
            b"cHRM" => PngChunkType::cHRM,
            b"cLLI" => PngChunkType::cLLI,
            b"cICP" => PngChunkType::cICP,
            b"sBIT" => PngChunkType::sBIT,
            _ => PngChunkType::unkn,
        };

        Ok(PngChunk {
            length: chunk_length,
            chunk: chunk_type_int,
            chunk_type,
        })
    }

    /// Decode headers from the ong stream and store information
    /// in the internal structure
    ///
    /// After calling this, header information can
    /// be accessed by public headers
    pub fn decode_headers(&mut self) -> Result<(), PngDecodeErrors> {
        self.decode_headers_inner()
    }
    pub(crate) fn decode_headers_inner(&mut self) -> Result<(), PngDecodeErrors> {
        if self.decoding_state != DecodingHeaders || self.seen_iend {
            return Ok(());
        }

        if !self.seen_hdr {
            // READ PNG signature
            let signature = self.stream.get_u64_be_err()?;

            if signature != PNG_SIGNATURE {
                return Err(PngDecodeErrors::BadSignature);
            }
        }
        loop {
            // on streaming.rs it stops if it finds a header that is not IDAT,
            // so it stores that header in non_parsed_header for subsequent use,
            // we should first check if that header exist and parse that and if not, we parse our
            // normal headers
            let header = {
                if let Some(header) = self.non_parsed_header.take() {
                    header
                } else {
                    self.read_chunk_header()?
                }
            };

            if header.chunk_type == PngChunkType::IDAT || header.chunk_type == PngChunkType::fdAT {
                // Stop parsing headers. We are ready to stream pixels.
                // Save the length so our streaming loop knows how much to read
                self.current_idat_bytes_left = header.length;
                self.decoding_state = DecodingBody;
                if header.chunk_type == PngChunkType::fdAT {
                    // starts with a 4 byte sequence, skip that
                    self.stream.skip(4)?;
                    self.current_idat_bytes_left = self.current_idat_bytes_left.saturating_sub(4);
                }
                break;
            }
            self.parse_header(header)?;

            if header.chunk_type == PngChunkType::IEND {
                break;
            }
        }
        // some checks
        if !self.seen_hdr {
            // we allow arbitrary headers, IHDR does not have to be the first,
            // but we do not allow IDAT being seen without a IHDR
            return Err(PngDecodeErrors::GenericStatic(
                "IHDR not encountered in fisrst scan",
            ));
        }
        if self.png_info.color == PngColor::Palette && !self.seen_ptle {
            return Err(PngDecodeErrors::GenericStatic(
                "Palette image without the corresponding PLTE chunk",
            ));
        }
        self.seen_headers = true;
        Ok(())
    }

    pub(crate) fn parse_header(&mut self, header: PngChunk) -> Result<(), PngDecodeErrors> {
        match header.chunk_type {
            PngChunkType::IDAT => {
                // IDAT is streamed, so not be dealt with here
                unreachable!("Should be dealt with in caller")
            }
            PngChunkType::IHDR => self.parse_ihdr(header)?,
            PngChunkType::PLTE => self.parse_plte(header)?,
            PngChunkType::tRNS => self.parse_trns(header)?,
            PngChunkType::gAMA => self.parse_gama(header)?,
            PngChunkType::acTL => self.parse_actl(header)?,
            PngChunkType::tIME => self.parse_time(header)?,
            PngChunkType::eXIf => self.parse_exif(header)?,
            PngChunkType::iCCP => self.parse_iccp(header)?,
            PngChunkType::iTXt => self.parse_itxt(header)?,
            PngChunkType::zTXt => self.parse_ztxt(header)?,
            PngChunkType::tEXt => self.parse_text(header)?,
            PngChunkType::fcTL => self.parse_fctl(header)?,
            PngChunkType::cHRM => self.parse_chrm(header)?,
            PngChunkType::sBIT => self.parse_sbit(header)?,
            PngChunkType::cICP => self.parse_cicp(header)?,
            PngChunkType::cLLI => self.parse_clli(header)?,
            PngChunkType::pHYs => self.parse_phys(header)?,
            PngChunkType::IEND => self.seen_iend = true,

            _ => default_chunk_handler(header.length, header.chunk, &mut self.stream)?,
        }

        if !self.seen_hdr {
            return Err(GenericStatic("IHDR block not encountered,corrupt jpeg"));
        }

        Ok(())
    }
    /// Return the configured image byte endian which the pixels
    /// will be in if the image is in 16 bit
    ///
    /// If the image depth is less than 16 bit, then the endianness has
    /// no effect
    pub const fn byte_endian(&self) -> ByteEndian {
        self.options.byte_endian()
    }

    /// Return the number of bytes required to hold a decoded image frame
    /// decoded using the given input transformations
    ///
    /// # Returns
    ///  - `Some(usize)`: Minimum size for a buffer needed to decode the image
    ///  - `None`: Indicates the image headers was not decoded.
    ///
    /// # Panics
    /// In case `width*height*colorspace` calculation may overflow a usize
    pub fn output_buffer_size(&self) -> Option<usize> {
        if !self.seen_hdr {
            return None;
        }

        let info = &self.png_info;
        let bytes = if info.depth == 16 && !self.options.png_get_strip_to_8bit() { 2 } else { 1 };

        let out_n = self.colorspace()?.num_components();
        let dims = self.dimensions().unwrap();

        dims.0
            .checked_mul(dims.1)?
            .checked_mul(out_n)?
            .checked_mul(bytes)
    }

    /// Get png information which was extracted from the headers
    ///
    ///
    /// # Returns
    /// - `Some(info)` : The information present in the header
    /// - `None` : Indicates headers were not decoded
    pub const fn info(&self) -> Option<&PngInfo> {
        if self.seen_hdr {
            Some(&self.png_info)
        } else {
            None
        }
    }
    /// Get a mutable reference to the decoder options
    /// for the decoder instance
    ///
    /// Can be used to modify options before actual decoding but after initial
    /// creation
    pub const fn options(&self) -> &DecoderOptions {
        &self.options
    }

    /// Overwrite decoder options with the new options
    ///
    /// Can be used to modify decoding after initialization but before
    /// decoding, it does not do anything after decoding an image
    pub fn set_options(&mut self, options: DecoderOptions) {
        self.options = options;
    }

    /// Decode PNG encoded images and write raw pixels into `out`
    ///
    /// # Arguments
    /// - `out`: The slice which we will write our values into.
    ///   If the slice length is smaller than [`output_buffer_size`](Self::output_buffer_size), it's an error
    ///
    /// # Converting 16 bit to 8 bit images
    /// When indicated by  [`DecoderOptions::png_set_strip_to_8bit`](zune_core::options::DecoderOptions::png_get_strip_to_8bit)
    /// the library will implicitly convert 16 bit to 8 bit by discarding the lower 8 bits
    ///
    /// # Endianness
    ///
    /// - In case the image is a 16 bit PNG, endianness of the samples may be retrieved
    ///   via [`byte_endian`](Self::byte_endian) method, which returns the configured byte
    ///   endian of the samples.
    /// - PNG uses Big Endian while most machines today are Little Endian (x86 and mainstream Arm),
    ///   hence if the configured endianness is little endian the library will implicitly convert
    ///   samples to little endian
    ///
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), PngDecodeErrors> {
        // decode headers
        self.decode_headers()?;

        self.decode_stream_into(out)
    }

    /// Decode data returning it into `Vec<u8>`.
    ///
    /// ## 16 Bit Images
    ///
    /// For 16 Bit images, the endianess of the image will be in Big Endian, most
    /// platforms are little endian so decoding to a `Vec<u8>` and then aliasing will cause problems, the best method
    /// in case you want to handle decoding in your native endian use [decode] which handles endianess for you
    ///
    /// ## Converting 16 bit to 8 bit images
    /// When indicated by  [`DecoderOptions::png_set_strip_to_8bit`](zune_core::options::DecoderOptions::png_get_strip_to_8bit)
    /// the library will implicitly convert 16 bit to 8 bit by discarding the lower 8 bits
    ///
    /// returns: `Result<Vec<u8, Global>, PngErrors>`
    ///
    ///
    /// [decode]: PngDecoder::decode
    pub fn decode_raw(&mut self) -> Result<Vec<u8>, PngDecodeErrors> {
        self.decode_stream_raw()
    }
    /// Return the metadata (`FrameInfo`) for the *next* frame waiting to be decoded.
    ///
    /// Because APNG frames can have varying dimensions, offsets, and compositing operations,
    /// you must retrieve this information to know how to properly handle the raw pixels
    /// returned by the next call to `decode()`, `decode_raw()`, or `decode_into()`.
    ///
    /// **Lifecycle:**
    /// 1. Call `decode_headers()` to parse the upcoming frame's control chunk.
    /// 2. Call `frame_info()` to read the metadata.
    /// 3. Call `decode()` to extract the pixels for that frame.
    ///
    /// Once a decode function is called, the internal state advances, and this function will
    /// then point to the subsequent frame (if any).
    ///
    /// # Returns
    /// - `Some(FrameInfo)`: The metadata for the pending frame.
    /// - `None`: If no frame control chunk has been decoded yet.
    ///
    /// # Example
    ///
    /// This example gets frame information of an animated image
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_png::PngDecoder;
    /// let mut decoder = PngDecoder::new(ZCursor::new(&[]));
    ///
    /// // decode the headers to get the information
    /// decoder.decode_headers().unwrap();
    ///
    ///if decoder.is_animated(){
    ///     while decoder.more_frames(){
    ///         // multiple calls is okay, the library will handle it correctly
    ///         decoder.decode_headers().unwrap();
    ///         // get information, MUST BE before calling (decode,decode_headers,decode_raw)
    ///         let info = decoder.frame_info().unwrap();
    ///         // decode the frame
    ///         let data = decoder.decode().unwrap();
    ///     }
    /// }
    /// ```
    pub fn frame_info(&self) -> Option<FrameInfo> {
        if let Some(frame) = self.frames.get(self.current_frame) {
            return Some(frame.fctl_info);
        }
        None
    }
    /// Decode PNG encoded images and return the vector of raw pixels but for 16-bit images
    /// represent them in a `Vec<u16>` if  [`DecoderOptions::png_set_strip_to_8bit`](zune_core::options::DecoderOptions::png_get_strip_to_8bit)
    /// returns them in a `Vec<u8>`
    ///
    ///
    /// This returns an enum type [`DecodingResult`](zune_core::result::DecodingResult) which
    /// one can de-sugar to extract actual values.
    ///
    /// # Converting 16 bit to 8 bit images
    /// When indicated by  [`DecoderOptions::png_set_strip_to_8bit`](zune_core::options::DecoderOptions::png_get_strip_to_8bit)
    /// the library will implicitly convert 16 bit to 8 bit by discarding the lower 8 bits
    ///
    /// If such is specified, this routine will always return [`DecodingResult::U8`](zune_core::result::DecodingResult::U8)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_core::result::DecodingResult;
    /// use zune_png::PngDecoder;
    /// let mut decoder = PngDecoder::new(ZCursor::new(&[]));
    ///
    /// match decoder.decode().unwrap(){
    ///     DecodingResult::U16(value)=>{
    ///         // deal with 16 bit images
    ///     }
    ///     DecodingResult::U8(value)=>{
    ///         // deal with <8 bit image
    ///     }
    ///     _=>{}
    /// }
    /// ```
    pub fn decode(&mut self) -> Result<DecodingResult, PngDecodeErrors> {
        // Here we want to either return a `u8` or a `u16` depending on the
        // headers, so we pull two tricks
        //  1 - We either allocate u8 or u16 depending on the output
        //      We actually allocate both, but one of the vectors ends up being
        //      zero, and in creating an empty vec nothing is allocated on the heap
        //  2 - We convert samples to native endian, so that transmuting is a no-op in case of
        //      16 bit images in the next step

        self.decode_headers()?;

        // in case we are to strip 16 bit to 8 bit, use decode_raw which does that for us
        if self.options.png_get_strip_to_8bit() && self.png_info.depth == 16 {
            let bytes = self.decode_raw()?;
            return Ok(DecodingResult::U8(bytes));
        }
        let info = &self.png_info;
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let out_n = self.colorspace().unwrap().num_components();
        let new_len = info.width * info.height * out_n;

        let mut out_u8: Vec<u8> = vec![0; new_len * usize::from(info.depth != 16)];
        let mut out_u16: Vec<u16> = vec![0; new_len * usize::from(info.depth == 16)];

        // use either out_u8 or out_u16 depending on the expected type for the output
        let out = if bytes == 1 {
            &mut out_u8
        } else {
            let b = convert_u16_to_u8_slice(&mut out_u16);

            assert_eq!(b.len(), new_len * 2); // length should be twice that of u8
            b
        };
        self.decode_stream_into(out)?;

        if is_le() {
            // png is BE by default
            // so all bytes are decoded as BE but case to LE, if the target platform is LE, swap
            out_u16.iter_mut().for_each(|x| *x = x.swap_bytes());
        }

        if self.png_info.depth <= 8 {
            return Ok(DecodingResult::U8(out_u8));
        }

        if self.png_info.depth == 16 {
            return Ok(DecodingResult::U16(out_u16));
        }

        Err(PngDecodeErrors::GenericStatic("Not implemented"))
    }
}
