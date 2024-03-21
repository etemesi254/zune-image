/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

// Stolen from  mozilla nsDecoder (https://searchfox.org/mozilla-central/source/image/decoders/nsBMPDecoder.cpp#10)
//
// BMP is a format that has been extended multiple times. To understand the
// decoder you need to understand this history. The summary of the history
// below was determined from the following documents.
//
// - http://www.fileformat.info/format/bmp/egff.htm
// - http://www.fileformat.info/format/os2bmp/egff.htm
// - http://fileformats.archiveteam.org/wiki/BMP
// - http://fileformats.archiveteam.org/wiki/OS/2_BMP
// - https://en.wikipedia.org/wiki/BMP_file_format
// - https://upload.wikimedia.org/wikipedia/commons/c/c4/BMPfileFormat.png
// - https://blog.mozilla.org/nnethercote/2015/11/06/i-rewrote-firefoxs-bmp-decoder/
//
//
// WINDOWS VERSIONS OF THE BMP FORMAT
// ----------------------------------
// WinBMPv1.
// - This version is no longer used and can be ignored.
//
// WinBMPv2.
// - First is a 14 byte file header that includes: the magic number ("BM"),
//   file size, and offset to the pixel data (|mDataOffset|).
// - Next is a 12 byte info header which includes: the info header size
//   (mBIHSize), width, height, number of color planes, and bits-per-pixel
//   (|mBpp|) which must be 1, 4, 8 or 24.
// - Next is the semi-optional color table, which has length 2^|mBpp| and has 3
//   bytes per value (BGR). The color table is required if |mBpp| is 1, 4, or 8.
// - Next is an optional gap.
// - Next is the pixel data, which is pointed to by |mDataOffset|.
//
// WinBMPv3. This is the most widely used version.
// - It changed the info header to 40 bytes by taking the WinBMPv2 info
//   header, enlargening its width and height fields, and adding more fields
//   including: a compression type (|mCompression|) and number of colors
//   (|mNumColors|).
// - The semi-optional color table is now 4 bytes per value (BGR0), and its
//   length is |mNumColors|, or 2^|mBpp| if |mNumColors| is zero.
// - |mCompression| can be RGB (i.e. no compression), RLE4 (if |mBpp|==4) or
//   RLE8 (if |mBpp|==8) values.
//
// WinBMPv3-NT. A variant of WinBMPv3.
// - It did not change the info header layout from WinBMPv3.
// - |mBpp| can now be 16 or 32, in which case |mCompression| can be RGB or the
//   new BITFIELDS value; in the latter case an additional 12 bytes of color
//   bitfields follow the info header.
//
// WinBMPv4.
// - It extended the info header to 108 bytes, including the 12 bytes of color
//   mask data from WinBMPv3-NT, plus alpha mask data, and also color-space and
//   gamma correction fields.
//
// WinBMPv5.
// - It extended the info header to 124 bytes, adding color profile data.
// - It also added an optional color profile table after the pixel data (and
//   another optional gap).
//
// WinBMPv3-ICO. This is a variant of WinBMPv3.
// - It's the BMP format used for BMP images within ICO files.
// - The only difference with WinBMPv3 is that if an image is 32bpp and has no
//   compression, then instead of treating the pixel data as 0RGB it is treated
//   as ARGB, but only if one or more of the A values are non-zero.
//
// Clipboard variants.
// - It's the BMP format used for BMP images captured from the clipboard.
// - It is missing the file header, containing the BM signature and the data
//   offset. Instead the data begins after the header.
// - If it uses BITFIELDS compression, then there is always an additional 12
//   bytes of data after the header that must be read. In WinBMPv4+, the masks
//   are supposed to be included in the header size, which are the values we use
//   for decoding purposes, but there is additional three masks following the
//   header which must be skipped to get to the pixel data.
//
// OS/2 VERSIONS OF THE BMP FORMAT
// -------------------------------
// OS2-BMPv1.
// - Almost identical to WinBMPv2; the differences are basically ignorable.
//
// OS2-BMPv2.
// - Similar to WinBMPv3.
// - The info header is 64 bytes but can be reduced to as little as 16; any
//   omitted fields are treated as zero. The first 40 bytes of these fields are
//   nearly identical to the WinBMPv3 info header; the remaining 24 bytes are
//   different.
// - Also adds compression types "Huffman 1D" and "RLE24", which we don't
//   support.
// - We treat OS2-BMPv2 files as if they are WinBMPv3 (i.e. ignore the extra 24
//   bytes in the info header), which in practice is good enough.

use alloc::vec::Vec;
use alloc::{format, vec};

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteIoError, ZByteReaderTrait, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::log::{trace, warn};
use zune_core::options::DecoderOptions;

use crate::common::{BmpCompression, BmpPixelFormat};
use crate::utils::expand_bits_to_byte;
use crate::BmpDecoderErrors;

/// Probe some bytes to see
/// if they consist of a BMP image
pub fn probe_bmp(bytes: &[u8]) -> bool {
    if let Some(magic_bytes) = bytes.get(0..2) {
        if magic_bytes == b"BM" {
            // skip file_size   -> 4
            // skip reserved    -> 4
            // skip data offset -> 4
            // read sz
            if let Some(sz) = bytes.get(14) {
                let sz = *sz;

                return sz == 12
                    || sz == 16 /*os-v2*/
                    || sz == 40
                    || sz == 52
                    || sz == 56
                    || sz == 64 /*os-v2*/
                    || sz == 108
                    || sz == 124;
            }
        }
    }
    false
}

/// A single palette entry for bmp
///
/// For some configurations, alpha is disabled,
#[derive(Clone, Copy, Default, Debug)]
struct PaletteEntry {
    red:   u8,
    green: u8,
    blue:  u8,
    alpha: u8
}

/// A BMP decoder.
pub struct BmpDecoder<T>
where
    T: ZByteReaderTrait
{
    bytes:           ZReader<T>,
    options:         DecoderOptions,
    width:           usize,
    height:          usize,
    flip_vertically: bool,
    rgb_bitfields:   [u32; 4],
    decoded_headers: bool,
    pix_fmt:         BmpPixelFormat,
    comp:            BmpCompression,
    ihszie:          u32,
    hsize:           u32,
    palette:         Vec<PaletteEntry>,
    depth:           u16,
    is_alpha:        bool,
    palette_numbers: usize
}

impl<T> BmpDecoder<T>
where
    T: ZByteReaderTrait
{
    /// Create a new bmp decoder that reads data from
    /// `data`
    ///
    /// # Arguments
    /// - `data`: The buffer from which we will read bytes from
    ///
    /// # Returns
    /// - A BMP decoder instance
    pub fn new(data: T) -> BmpDecoder<T> {
        BmpDecoder::new_with_options(data, DecoderOptions::default())
    }
    /// Create a new decoder instance with specified options
    ///
    /// # Arguments
    ///
    /// * `data`: The buffer from which we will read data from
    /// * `options`:  Specialized options for this decoder
    ///
    /// returns: A BMP Decoder instance
    ///
    pub fn new_with_options(data: T, options: DecoderOptions) -> BmpDecoder<T> {
        BmpDecoder {
            bytes: ZReader::new(data),
            options,
            decoded_headers: false,
            width: 0,
            height: 0,
            comp: BmpCompression::Unknown,
            rgb_bitfields: [0; 4],
            pix_fmt: BmpPixelFormat::None,
            flip_vertically: false,
            ihszie: 0,
            hsize: 0,
            depth: 0,
            palette: vec![],
            is_alpha: false,
            palette_numbers: 0
        }
    }

    /// Decode headers stored in the bmp file and store
    /// information in the decode context
    ///
    /// After calling this, most information fields will be filled
    /// except the actual decoding bytes
    ///
    /// # Returns
    /// - Ok(()) Indicates everything was okay during header parsing
    /// - Err: Error that occurred when decoding headers
    pub fn decode_headers(&mut self) -> Result<(), BmpDecoderErrors> {
        if self.decoded_headers {
            return Ok(());
        }

        if self.bytes.read_u8_err()? != b'B' || self.bytes.read_u8_err()? != b'M' {
            return Err(BmpDecoderErrors::InvalidMagicBytes);
        }
        // 4 bytes file size
        // skip 4 reserved bytes
        self.bytes.skip(8)?;

        let hsize = self.bytes.get_u32_le_err()?;
        let ihsize = self.bytes.get_u32_le_err()?;

        if ihsize.saturating_add(14) > hsize {
            return Err(BmpDecoderErrors::GenericStatic("Invalid header size"));
        }

        let (width, height);
        match ihsize {
            16 | 40 | 52 | 56 | 64 | 108 | 124 => {
                width = self.bytes.get_u32_le_err()?;
                height = self.bytes.get_u32_le_err()?;
            }
            12 => {
                // os-v2 images
                width = self.bytes.get_u16_le_err()? as u32;
                height = self.bytes.get_u16_le_err()? as u32;
            }
            _ => {
                return Err(BmpDecoderErrors::GenericStatic(
                    "Unknown information header size"
                ));
            }
        }

        self.flip_vertically = (height as i32) > 0;

        self.height = (height as i32).unsigned_abs() as usize;
        self.width = width as usize;

        if self.height > self.options.max_height() {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "height",
                self.options.max_height(),
                self.height
            ));
        }

        if self.width > self.options.max_width() {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "width",
                self.options.max_width(),
                self.width
            ));
        }
        if self.width == 0 {
            return Err(BmpDecoderErrors::GenericStatic(
                "Width is zero, invalid image"
            ));
        }
        if self.height == 0 {
            return Err(BmpDecoderErrors::GenericStatic(
                "Height is zero, invalid image"
            ));
        }

        trace!("Width: {}", self.width);
        trace!("Height: {}", self.height);

        // planes
        if self.bytes.get_u16_le_err()? != 1 {
            return Err(BmpDecoderErrors::GenericStatic("Invalid BMP header"));
        }

        let depth = self.bytes.get_u16_le_err()?;

        if depth == 0 {
            return Err(BmpDecoderErrors::GenericStatic(
                "Depth is zero, invalid image"
            ));
        }

        let compression = if hsize >= 40 {
            match BmpCompression::from_u32(self.bytes.get_u32_le_err()?) {
                Some(c) => c,
                None => {
                    return Err(BmpDecoderErrors::GenericStatic(
                        "Unsupported BMP compression scheme"
                    ));
                }
            }
        } else {
            BmpCompression::RGB
        };
        if compression == BmpCompression::BITFIELDS {
            self.bytes.skip(20)?;

            self.rgb_bitfields[0] = self.bytes.get_u32_le_err()?;
            self.rgb_bitfields[1] = self.bytes.get_u32_le_err()?;
            self.rgb_bitfields[2] = self.bytes.get_u32_le_err()?;

            if ihsize > 40 {
                self.rgb_bitfields[3] = self.bytes.get_u32_le_err()?;
            }
        }

        match depth {
            32 => self.pix_fmt = BmpPixelFormat::RGBA,
            24 => self.pix_fmt = BmpPixelFormat::RGB,
            16 => {
                if compression == BmpCompression::RGB {
                    self.pix_fmt = BmpPixelFormat::RGB;
                } else if compression == BmpCompression::BITFIELDS {
                    self.pix_fmt = BmpPixelFormat::RGBA;
                }
            }
            8 => {
                if hsize.wrapping_sub(ihsize).wrapping_sub(14) > 0 {
                    self.pix_fmt = BmpPixelFormat::PAL8;
                } else {
                    self.pix_fmt = BmpPixelFormat::GRAY8;
                }
            }
            1 | 2 | 4 => {
                if depth == 2 {
                    warn!("Depth of 2 not officially supported");
                }

                if hsize.wrapping_sub(ihsize).wrapping_sub(14) > 0 {
                    self.pix_fmt = BmpPixelFormat::PAL8;
                } else {
                    let message = format!("Unknown palette for {}-color bmp", 1 << depth);
                    return Err(BmpDecoderErrors::Generic(message));
                }
            }

            _ => {
                let message = format!("Depth {depth} unsupported");
                return Err(BmpDecoderErrors::Generic(message));
            }
        };
        if self.pix_fmt == BmpPixelFormat::None {
            return Err(BmpDecoderErrors::GenericStatic("Unsupported Pixel format"));
        }

        let p = self.hsize.wrapping_sub(self.ihszie).wrapping_sub(14);

        if self.pix_fmt == BmpPixelFormat::PAL8 {
            let mut colors = 1_u32 << depth;

            if ihsize >= 36 {
                self.bytes.set_position(46)?;
                let t = self.bytes.get_u32_le_err()? as i32;

                if t < 0 || t > (1 << depth) {
                    let msg = format!("Incorrect number of colors {} for depth {}", t, depth);
                    if self.options.strict_mode() {
                        return Err(BmpDecoderErrors::Generic(msg));
                    }
                    warn!("{}", msg);
                } else if t != 0 {
                    colors = t as u32;
                }
            } else {
                colors = 256.min(p / 3);
            }
            // palette location
            self.bytes.set_position((14 + ihsize) as usize)?;

            // OS/2 bitmap, 3 bytes per palette entry
            if ihsize == 12 {
                if p < colors * 3 {
                    return Err(BmpDecoderErrors::GenericStatic("Invalid Palette entries"));
                }
                self.palette.resize(256, PaletteEntry::default());

                self.palette.iter_mut().take(colors as usize).for_each(|x| {
                    let [b, g, r] = self
                        .bytes
                        .read_fixed_bytes_or_error::<3>()
                        .unwrap_or([0, 0, 0]);

                    x.red = r;
                    x.green = g;
                    x.blue = b;
                });
            } else {
                self.palette.resize(256, PaletteEntry::default());

                self.palette.iter_mut().take(colors as usize).for_each(|x| {
                    let [b, g, r, _] = self
                        .bytes
                        .read_fixed_bytes_or_error::<4>()
                        .unwrap_or([0, 0, 0, 0]);

                    x.red = r;
                    x.green = g;
                    x.blue = b;
                    // alpha is silently ignored, which is weird
                    // but i will match that behaviour
                    x.alpha = 255;
                });
            }
            self.palette_numbers = colors as usize;
        }

        trace!("Pixel format : {:?}", self.pix_fmt);
        trace!("Compression  : {:?}", compression);
        trace!("Bit depth: {:?}", depth);
        self.comp = compression;
        self.depth = depth;
        self.ihszie = ihsize;
        self.hsize = hsize;
        self.bytes.set_position(hsize as usize)?;

        self.decoded_headers = true;

        Ok(())
    }

    /// Return the expected size of the output buffer for which
    /// a contiguous slice of `&[u8]` can store it without needing reallocation
    ///
    /// Returns `None` if headers haven't been decoded or if calculation overflows
    pub fn output_buf_size(&self) -> Option<usize> {
        if !self.decoded_headers {
            return None;
        }
        self.width
            .checked_mul(self.height)?
            .checked_mul(self.pix_fmt.num_components())
    }

    /// Return the BMP bit depth
    ///
    /// This is always [BitDepth::Eight](zune_core::bit_depth::BitDepth::Eight)
    /// since it's the only one BMP supports
    ///
    /// Images with less than 8 bits per pixel are usually scaled to be eight bits
    pub fn depth(&self) -> BitDepth {
        BitDepth::Eight
    }
    /// Get dimensions of the image
    ///
    /// This is a tuple of width,height
    ///
    /// # Returns
    /// - `Some((width,height))`  - The image dimensions
    /// - `None`: Indicates that the image headers weren't decoded
    ///    or an error occurred during decoding the headers   
    pub fn dimensions(&self) -> Option<(usize, usize)> {
        if !self.decoded_headers {
            return None;
        }
        Some((self.width, self.height))
    }
    /// Get the image colorspace or none if the headers weren't decoded
    ///
    /// # Returns
    /// - `Some(colorspace)`: The colorspace of the image
    /// - `None`: Indicates headers weren't decoded or an error occured
    /// during decoding of headers
    pub fn colorspace(&self) -> Option<ColorSpace> {
        if !self.decoded_headers {
            return None;
        }

        Some(self.pix_fmt.into_colorspace())
    }
    /// Decode an image returning the decoded bytes as an
    /// allocated `Vec<u8>` or an error if decoding could not be completed
    ///
    ///
    /// Also see [`decode_into`](Self::decode_into) which decodes into
    /// a pre-allocated buffer
    pub fn decode(&mut self) -> Result<Vec<u8>, BmpDecoderErrors> {
        self.decode_headers()?;
        let mut output = vec![
            0_u8;
            self.output_buf_size()
                .ok_or(BmpDecoderErrors::OverFlowOccurred)?
        ];

        self.decode_into(&mut output)?;

        Ok(output)
    }

    /// Decode an encoded image into a buffer or return an error
    /// if something bad occurred
    ///
    /// Also see [`decode`](Self::decode) which allocates and decodes into buffer
    pub fn decode_into(&mut self, buf: &mut [u8]) -> Result<(), BmpDecoderErrors> {
        self.decode_headers()?;

        let output_size = self
            .output_buf_size()
            .ok_or(BmpDecoderErrors::OverFlowOccurred)?;

        let buf = &mut buf[0..output_size];

        if self.comp == BmpCompression::RLE4 || self.comp == BmpCompression::RLE8 {
            let scanline_data = self.decode_rle()?;

            if self.pix_fmt == BmpPixelFormat::PAL8 {
                self.expand_palette(&scanline_data, buf, false);
                // flip, orientation isn't good
                self.flip_vertically = true;
            }
        } else {
            match self.depth {
                8 | 16 | 24 | 32 => {
                    // NOTE: Most of these images have a weird
                    // rchunks iteration on output buffer, this is intended to skip
                    // the output buffer flip most decoders do.
                    //
                    //
                    // BMP format writes from bottom to top meaning the first byte in the stream
                    // points to the last strip, first byte, or [x,1]  in this iteration
                    //   ┌───────────────┐
                    //   │               │
                    //   │               │
                    //   │               │
                    //   │[x,1]          │
                    //   └───────────────┘
                    //
                    // Furthermore, bmp has padding bytes to make buffers round off to 4 bytes, so
                    // most input iterations take that into account.

                    if self.pix_fmt == BmpPixelFormat::PAL8 {
                        self.expand_palette_from_remaining_bytes(buf, true)?;
                        // do not flip for palette, orientation is good
                        self.flip_vertically ^= true;
                    } else {
                        // copy
                        if self.depth == 32 || self.depth == 16 {
                            // bpp of 32 doesn't have padding, width should be a multiple of 4 already

                            let pad_size = self.width * self.pix_fmt.num_components();

                            if self.rgb_bitfields == [0; 4] && self.depth == 16 {
                                // no bitfields. set default bitfields masks
                                self.rgb_bitfields = [31 << 10, 31 << 5, 31, 31];
                            }

                            if self.rgb_bitfields == [0; 4] && self.depth == 32 {
                                // if there are no bitfields, it's simply a copy, adding alpha channel
                                // as 255
                                for out in buf.rchunks_exact_mut(pad_size) {
                                    for a in out.chunks_exact_mut(4) {
                                        let pixels = self.bytes.read_fixed_bytes_or_zero::<4>();
                                        a.copy_from_slice(&pixels);
                                        a[3] = 255;
                                    }
                                }
                            } else {
                                // extract bitfields
                                let [mr, mg, mb, ma] = self.rgb_bitfields;

                                let rshift = (32 - mr.leading_zeros()).wrapping_sub(8) as i32;
                                let gshift = (32 - mg.leading_zeros()).wrapping_sub(8) as i32;
                                let bshift = (32 - mb.leading_zeros()).wrapping_sub(8) as i32;
                                let ashift = (32 - ma.leading_zeros()).wrapping_sub(8) as i32;

                                let rcount = mr.count_ones();
                                let gcount = mg.count_ones();
                                let bcount = mb.count_ones();
                                let acount = ma.count_ones();

                                if self.depth == 32 {
                                    // for the case where the bit-depth is 32, there are no padding bytes
                                    // hence we can iterate simply

                                    for out in buf.rchunks_exact_mut(pad_size) {
                                        for a in out.chunks_exact_mut(4) {
                                            let v = self.bytes.get_u32_le();

                                            a[0] = shift_signed(v & mr, rshift, rcount) as u8;
                                            a[1] = shift_signed(v & mg, gshift, gcount) as u8;
                                            a[2] = shift_signed(v & mb, bshift, bcount) as u8;
                                            if ma == 0 {
                                                // alpha would be zero
                                                a[3] = 255;
                                            } else {
                                                a[3] = shift_signed(v & ma, ashift, acount) as u8;
                                            }
                                        }
                                    }
                                } else if self.depth == 16 {
                                    //let bytes = self.bytes.remaining_bytes()?;
                                    // this path is confirmed by the rgb16.bmp image in test-images/bmp

                                    // optimizer hint to say that the inner loop will always
                                    // have more than three
                                    // PS: Not sure if it works
                                    assert!(self.pix_fmt.num_components() >= 3);

                                    // Number of iterations we expect to go
                                    let num_iters = buf.len() / pad_size;
                                    // Input bytes per every output byte width
                                    //
                                    // this does ceil division to ensure input is appropriately rounded
                                    // of to a multiple of 4 to handle pad bytes in BMP
                                    let input_bytes_per_width = (((num_iters * 2) + 3) / 4) * 4;

                                    // we chunk according to number of iterations, which is usually the
                                    // image dimensions (w*h*color components), this is given by the size of
                                    // buf - output buffer is appropriately sized for this-.
                                    // for the input, we are reading two bytes( for every three bytes (or four)
                                    // consumed hence if we know how many iterations we will be doing
                                    // we can use that to chunk appropriately

                                    for out in buf.rchunks_exact_mut(pad_size) {
                                        let mut count = 0;
                                        for a in out
                                            .chunks_exact_mut(self.pix_fmt.num_components())
                                            .take(input_bytes_per_width)
                                        {
                                            let v = u32::from(u16::from_le_bytes(
                                                self.bytes.read_fixed_bytes_or_zero::<2>()
                                            ));
                                            count += 2;

                                            a[0] = shift_signed(v & mr, rshift, rcount) as u8;
                                            a[1] = shift_signed(v & mg, gshift, gcount) as u8;
                                            a[2] = shift_signed(v & mb, bshift, bcount) as u8;

                                            if a.len() > 3 {
                                                // handle alpha channel
                                                if ma == 0 {
                                                    a[3] = 255;
                                                } else {
                                                    a[3] =
                                                        shift_signed(v & ma, ashift, acount) as u8;
                                                }
                                            }
                                        }
                                        //assert_eq!(count, input_bytes_per_width * 2);
                                        debug_assert!(
                                            input_bytes_per_width * 2 >= count,
                                            "Input bytes cannot be greater than count"
                                        );
                                        self.bytes.skip(
                                            (input_bytes_per_width * 2).saturating_sub(count)
                                        )?;
                                    }
                                }
                            }

                            self.flip_vertically ^= true;
                        } else {
                            // let bytes = self.bytes.remaining_bytes()?;
                            // bmp rounds up each line to be a multiple of 4, padding the end if necessary
                            // remove padding bytes
                            let out_width = self.width * self.pix_fmt.num_components();

                            // includes pad bytes (multiple of 4)
                            let in_width = ((self.width * usize::from(self.depth) + 31) / 8) & !3;

                            for out in buf.rchunks_exact_mut(out_width) {
                                self.bytes.read_exact_bytes(out)?;
                                // skip padding bytes
                                self.bytes.skip(in_width.saturating_sub(out_width))?;
                            }
                            self.flip_vertically ^= true;
                        }
                    }
                }
                1 | 2 | 4 => {
                    // For depths less than 8, we must have a palette present
                    //
                    // Then the operations become expanding bits
                    // to bytes and then read palette entries
                    if self.pix_fmt != BmpPixelFormat::PAL8 {
                        return Err(BmpDecoderErrors::GenericStatic(
                            "Bit Depths less than 8 must have a palette"
                        ));
                    }
                    let width_bytes = ((self.width + 7) >> 3) << 3;

                    // temporary location for an scaled down image width, bytes are read here
                    // before expanding them in a separate pass
                    let in_width_bytes = ((self.width * usize::from(self.depth)) + 7) / 8;
                    let mut in_width_buf = vec![0_u8; in_width_bytes];

                    let scanline_size = width_bytes * 3;
                    let mut scanline_bytes = vec![0_u8; scanline_size];

                    for out_bytes in
                        buf.rchunks_exact_mut((3 + usize::from(self.is_alpha)) * self.width)
                    {
                        // read a whole width scanline before expanding
                        self.bytes.read_exact_bytes(&mut in_width_buf)?;

                        expand_bits_to_byte(
                            self.depth as usize,
                            true,
                            &in_width_buf,
                            &mut scanline_bytes
                        );
                        self.expand_palette(&scanline_bytes, out_bytes, true);
                    }
                    self.flip_vertically ^= true;
                }
                d => unreachable!("Unhandled depth {}", d)
            }
        }
        // The code for flip uses xor, so that if the image was to be flipped
        // (hence it was initially set to  true) and the code encounters a route that does
        // the flipping internally(using rchunks where appropriate) the route will xor it with true
        // (i.e code like self.flip_vertically ^= true), so that then looks like
        //
        // |self.flip_vertically| route | result |
        // | false              | true  | true   |
        // | true               | true  | false  |
        //
        // Meaning the if below is only entered if the BMP indicated it wasn't supposed to be flipped
        // vertically to undo the implicit earlier flip vertically that may have occurred
        if self.flip_vertically {
            //
            // if we are here it means the image was not supposed to be flipped
            // but we eagerly flip because most bmp images are flipped, hence the code above
            // usually assumes that
            //
            // This code undoes the effect of the above flips.
            let length = self.width * self.pix_fmt.num_components();

            let mut scanline = vec![0; length];
            let mid = buf.len() / 2;
            let (in_img_top, in_img_bottom) = buf.split_at_mut(mid);

            for (in_dim, out_dim) in in_img_top
                .chunks_exact_mut(length)
                .zip(in_img_bottom.rchunks_exact_mut(length))
            {
                // write in dim to scanlines
                scanline.copy_from_slice(in_dim);
                // write out dim to in dim
                in_dim.copy_from_slice(out_dim);
                // copy previous scanline to in dim
                out_dim.copy_from_slice(&scanline);
            }
        }

        Ok(())
    }

    /// Expand paletted bmp images to full version
    ///
    ///
    /// # Arguments
    /// - in_bytes: Palette entry indices
    /// - buf: Where to write the bytes to
    /// - unpad: Whether to take padding bytes into account, this is important since
    ///  callers like RLE bytes do not take padding into account but for non-rle data
    /// we must take it into account
    ///
    fn expand_palette(&self, in_bytes: &[u8], buf: &mut [u8], unpad: bool) {
        let palette: &[PaletteEntry; 256] = &self.palette[0..256].try_into().unwrap();

        let pad = usize::from(unpad) * (((-(self.width as i32)) as u32) & 3) as usize;

        if self.is_alpha {
            // bmp rounds up each line to be a multiple of 4, padding the end if necessary
            // remove padding bytes

            for (out_stride, in_stride) in buf
                .rchunks_exact_mut(self.width * 4)
                .take(self.height)
                .zip(in_bytes.chunks_exact(self.width + pad))
            {
                for (pal_byte, chunks) in in_stride.iter().zip(out_stride.chunks_exact_mut(4)) {
                    let entry = palette[usize::from(*pal_byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                    chunks[3] = entry.alpha;
                }
            }
        } else {
            for (out_stride, in_stride) in buf
                .rchunks_exact_mut(self.width * 3)
                .take(self.height)
                .zip(in_bytes.chunks_exact(self.width + pad))
            {
                for (pal_byte, chunks) in in_stride.iter().zip(out_stride.chunks_exact_mut(3)) {
                    let entry = palette[usize::from(*pal_byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                }
            }
        }
    }
    // RUST borrowing rules
    fn expand_palette_from_remaining_bytes(
        &mut self, buf: &mut [u8], unpad: bool
    ) -> Result<(), ZByteIoError> {
        //let in_bytes = self.bytes.remaining_bytes()?;
        let palette: &[PaletteEntry; 256] = &self.palette[0..256].try_into().unwrap();

        let pad = usize::from(unpad) * (((-(self.width as i32)) as u32) & 3) as usize;

        if self.is_alpha {
            // bmp rounds up each line to be a multiple of 4, padding the end if necessary
            // remove padding bytes

            for out_stride in buf.rchunks_exact_mut(self.width * 4).take(self.height) {
                for chunks in out_stride.chunks_exact_mut(4) {
                    let byte = self.bytes.read_u8();
                    let entry = palette[usize::from(byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                    chunks[3] = entry.alpha;
                }
                self.bytes.skip(pad)?;
            }
        } else {
            for out_stride in buf.rchunks_exact_mut(self.width * 3).take(self.height) {
                for chunks in out_stride.chunks_exact_mut(3) {
                    let byte = self.bytes.read_u8();
                    let entry = palette[usize::from(byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                }

                self.bytes.skip(pad)?;
            }
        }
        Ok(())
    }
    fn decode_rle(&mut self) -> Result<Vec<u8>, BmpDecoderErrors> {
        // Docs are from imagine crate
        //
        // * If the first byte is **non-zero** it's the number of times that the second
        //   byte appears in the output. The second byte is an index into the palette,
        //   and you just put out that color and output it into the bitmap however many
        //   times.
        // * If the first byte is **zero**, it signals an "escape sequence" sort of
        //   situation. The second byte will give us the details:
        //   * 0: end of line
        //   * 1: end of bitmap
        //   * 2: "Delta", the *next* two bytes after this are unsigned offsets to the
        //     right and up of where the output should move to (remember that this mode
        //     always describes the BMP with a bottom-left origin).
        //   * 3+: "Absolute", The second byte gives a count of how many bytes follow
        //     that we'll output without repetition. The absolute output sequences
        //     always have a padding byte on the ending if the sequence count is odd, so
        //     we can keep pulling `[u8;2]` at a time from our data and it all works.
        //
        //
        // Code is from ffmpeg

        // Allocate space for our RLE storage

        // for depths less than 8(4 only), allocate full space for the expanded bits
        let depth = if self.depth < 8 { 8 } else { self.depth };
        let mut pixels = vec![0; ((self.width * self.height * usize::from(depth)) + 7) >> 3];

        //let rt = temp_scanline.len();
        let mut line = (self.height - 1) as i32;
        let mut output = &mut pixels[(line as usize) * self.width..];
        let mut pos = 0;
        //let t = buf.len();

        if !(self.depth == 4 || self.depth == 8 || self.depth == 16 || self.depth == 32) {
            return Err(BmpDecoderErrors::Generic(format!(
                "Unknown Depth + RLE combination depth {}",
                self.depth
            )));
        }

        if self.depth == 4 {
            //Handle bit depth 4
            let mut rle_code: u16;
            let mut stream_byte;
            while line >= 0 && pos <= self.width {
                rle_code = u16::from(self.bytes.read_u8());

                if rle_code == 0 {
                    /* fetch the next byte to see how to handle escape code */
                    stream_byte = self.bytes.read_u8();

                    if stream_byte == 0 {
                        // move to the next line
                        line -= 1;
                        if line < 0 {
                            return Err(BmpDecoderErrors::Generic(format!(
                                "Line less than 0 {line}"
                            )));
                        }
                        // move to the next line
                        output = &mut pixels
                            [line as usize * self.width..(line + 1) as usize * self.width];
                        pos = 0;

                        continue;
                    } else if stream_byte == 1 {
                        // decode is done
                        return Ok(pixels);
                    } else if stream_byte == 2 {
                        // reposition frame decode coordinates
                        stream_byte = self.bytes.read_u8();
                        pos += usize::from(stream_byte);
                        stream_byte = self.bytes.read_u8();
                        line -= i32::from(stream_byte);

                        if line < 0 {
                            return Err(BmpDecoderErrors::Generic(format!(
                                "Line less than 0 {line}"
                            )));
                        }
                        output = &mut pixels
                            [line as usize * self.width..(line + 1) as usize * self.width];
                    } else {
                        // copy pixels from encoded stream
                        let odd_pixel = usize::from(stream_byte & 1);
                        rle_code = (u16::from(stream_byte) + 1) / 2;
                        let extra_byte = usize::from(rle_code & 0x01);

                        for i in 0..rle_code {
                            if pos >= self.width {
                                break;
                            }

                            stream_byte = self.bytes.read_u8();
                            output[pos] = stream_byte >> 4;
                            pos += 1;

                            if i + 1 == rle_code && odd_pixel > 0 {
                                break;
                            }
                            if pos >= self.width {
                                break;
                            }
                            output[pos] = stream_byte & 0x0F;
                            pos += 1;
                        }
                        // if the RLE code is odd, skip a byte in the stream
                        self.bytes.skip(usize::from(extra_byte > 0))?;
                    }
                } else {
                    // decode a run of data
                    if pos + usize::from(rle_code) > self.width + 1 {
                        let msg = "Frame pointer went out of bounds";
                        return Err(BmpDecoderErrors::GenericStatic(msg));
                    }
                    stream_byte = self.bytes.read_u8();

                    for i in 0..rle_code {
                        if pos >= self.width {
                            break;
                        }

                        if (i & 1) == 0 {
                            output[pos] = stream_byte >> 4;
                        } else {
                            output[pos] = stream_byte & 0x0F;
                        }

                        pos += 1;
                    }
                }
            }
            Ok(pixels)
        } else {
            let (mut p1, mut p2);
            // loop until no more bytes are left
            while !self.bytes.eof()? {
                p1 = self.bytes.read_u8();
                if p1 == 0 {
                    // escape code
                    p2 = self.bytes.read_u8();
                    if p2 == 0 {
                        // end of line
                        line -= 1;
                        if line < 0 {
                            return if self.bytes.get_u16_be() == 1 {
                                // end of picture
                                Ok(pixels)
                            } else {
                                // panic!();
                                let msg = "Next line is beyond picture bounds";
                                Err(BmpDecoderErrors::GenericStatic(msg))
                            };
                        }
                        if pos > output.len() {
                            return Err(BmpDecoderErrors::Generic(format!(
                                "Invalid image, out of bounds read in pos; pos={pos},length = {:?}",
                                output.len()
                            )));
                        }
                        // move to the next line
                        output = &mut pixels
                            [line as usize * self.width..(line + 1) as usize * self.width];
                        pos = 0;

                        continue;
                    } else if p2 == 1 {
                        // end of picture
                        return Ok(pixels);
                    } else if p2 == 2 {
                        // skip
                        p1 = self.bytes.read_u8();
                        p2 = self.bytes.read_u8();

                        pos += usize::from(p1);

                        line -= i32::from(p2);
                        if line < 0 {
                            return Err(BmpDecoderErrors::Generic(format!(
                                "Line less than 0 {line}"
                            )));
                        }

                        output = &mut pixels
                            [line as usize * self.width..(line + 1) as usize * self.width];

                        continue;
                    }
                    // copy data
                    if pos + usize::from(p2) * usize::from(self.depth >> 3) > output.len() {
                        self.bytes.skip(2 * usize::from(self.depth >> 3))?;
                        continue;
                    }
                    //
                    // else if self.bytes.get_bytes_left()
                    //     < usize::from(p2) * usize::from(self.depth >> 3)
                    // {
                    //     return Err(BmpDecoderErrors::GenericStatic("Bytestream overrun"));
                    // }

                    if self.depth == 8 || self.depth == 24 {
                        let size = usize::from(p2) * usize::from(self.depth >> 3);
                        self.bytes.read_exact_bytes(&mut output[pos..pos + size])?;
                        pos += size;

                        // RLE copy is padded- runs are not
                        if self.depth == 8 && (p2 & 1) == 1 {
                            self.bytes.skip(1)?;
                        }
                    } else if self.depth == 16 {
                        output[pos..]
                            .chunks_exact_mut(2)
                            .take(usize::from(p2))
                            .for_each(|x| {
                                x[0] = self.bytes.read_u8();
                                x[1] = self.bytes.read_u8();
                            });
                        pos += 2 * usize::from(p2);
                    } else if self.depth == 32 {
                        output[pos..]
                            .chunks_exact_mut(4)
                            .take(usize::from(p2))
                            .for_each(|x| {
                                x[0] = self.bytes.read_u8();
                                x[1] = self.bytes.read_u8();
                                x[2] = self.bytes.read_u8();
                                x[3] = self.bytes.read_u8();
                            });
                        pos += 4 * usize::from(p2);
                    }
                } else {
                    // run of pixels
                    let mut pix: [u8; 4] = [0; 4];

                    if pos + ((usize::from(p1) * usize::from(self.depth)) >> 3) > output.len() {
                        return Err(BmpDecoderErrors::GenericStatic("Position overrun"));
                    }
                    match self.depth {
                        8 => {
                            pix[0] = self.bytes.read_u8();
                            output[pos..pos + usize::from(p1)].fill(pix[0]);
                            pos += usize::from(p1);
                        }
                        16 => {
                            pix[0] = self.bytes.read_u8();
                            pix[1] = self.bytes.read_u8();

                            output[pos..]
                                .chunks_exact_mut(2)
                                .take(usize::from(p1))
                                .for_each(|x| {
                                    x[0..2].copy_from_slice(&pix[..2]);
                                });
                            pos += 2 * usize::from(p1);
                        }
                        24 => {
                            pix[0] = self.bytes.read_u8();
                            pix[1] = self.bytes.read_u8();
                            pix[2] = self.bytes.read_u8();

                            output[pos..]
                                .chunks_exact_mut(3)
                                .take(usize::from(p1))
                                .for_each(|x| {
                                    x[0..3].copy_from_slice(&pix[..3]);
                                });
                            pos += 3 * usize::from(p1);

                            // for _ in 0..p1 {
                            //     buf[pos] = pix[0];
                            //     buf[pos + 1] = pix[1];
                            //     buf[pos + 2] = pix[2];
                            //     pos += 3;
                            // }
                        }
                        32 => {
                            pix[0] = self.bytes.read_u8();
                            pix[1] = self.bytes.read_u8();
                            pix[2] = self.bytes.read_u8();
                            pix[3] = self.bytes.read_u8();

                            output[pos..]
                                .chunks_exact_mut(4)
                                .take(usize::from(p1))
                                .for_each(|x| x[0..4].copy_from_slice(&pix[..4]));
                            pos += 4 * usize::from(p1);
                        }
                        _ => unreachable!("Uhh ohh")
                    }
                }
            }
            warn!("RLE warning, no end of picture code");
            Ok(pixels)
        }
    }
}

fn shift_signed(mut v: u32, shift: i32, mut bits: u32) -> u32 {
    const MUL_TABLE: [u32; 9] = [
        0,    /*Hello world*/
        0xff, /*0b11111111*/
        0x55, /*0b01010101*/
        0x49, /*0b01001001*/
        0x11, /*0b00010001*/
        0x21, /*0b00100001*/
        0x41, /*0b01000001*/
        0x81, /*0b10000001*/
        0x01  /*0b00000001*/
    ];
    const SHIFT_TABLE: [i32; 9] = [0, 0, 0, 1, 0, 2, 4, 6, 0];

    if shift < 0 {
        v <<= -shift;
    } else {
        v >>= shift;
    }

    debug_assert!(v < 256);

    bits = bits.clamp(0, 8);
    v >>= 8 - bits;
    (v * MUL_TABLE[bits as usize]) >> SHIFT_TABLE[bits as usize]
}
