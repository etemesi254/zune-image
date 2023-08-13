/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec::Vec;
use alloc::{format, vec};

use log::{info, warn};
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteReader, ZReaderTrait};
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::common::{BmpCompression, BmpPixelFormat};
use crate::utils::expand_bits_to_byte;
use crate::BmpDecoderErrors;
use crate::BmpDecoderErrors::GenericStatic;

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

                return sz == 12 || sz == 40 || sz == 56 || sz == 108 || sz == 124;
            }
        }
    }
    false
}

#[derive(Clone, Copy, Default, Debug)]
struct PaletteEntry {
    red:   u8,
    green: u8,
    blue:  u8,
    alpha: u8
}

pub struct BmpDecoder<T>
where
    T: ZReaderTrait
{
    bytes:           ZByteReader<T>,
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
    T: ZReaderTrait
{
    /// Create a new bmp decoder that reads data from
    /// `data
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
    /// returns: BmpDecoder<T>
    ///
    pub fn new_with_options(data: T, options: DecoderOptions) -> BmpDecoder<T> {
        BmpDecoder {
            bytes: ZByteReader::new(data),
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
    pub fn decode_headers(&mut self) -> Result<(), BmpDecoderErrors> {
        if self.decoded_headers {
            return Ok(());
        }
        if !self.bytes.has(14) {
            // too short input
            return Err(BmpDecoderErrors::TooSmallBuffer(14, self.bytes.remaining()));
        }
        // discard

        if self.bytes.get_u8() != b'B' || self.bytes.get_u8() != b'M' {
            return Err(BmpDecoderErrors::InvalidMagicBytes);
        }
        let fszie = (self.bytes.get_u32_le()) as usize;

        if self.bytes.len() < fszie {
            return Err(BmpDecoderErrors::TooSmallBuffer(fszie, self.bytes.len()));
        }
        // skip 4 reserved bytes
        self.bytes.skip(4);

        let hsize = self.bytes.get_u32_le();
        let ihsize = self.bytes.get_u32_le();

        if ihsize.saturating_add(14) > hsize {
            return Err(BmpDecoderErrors::GenericStatic("Invalid header size"));
        }
        let (width, height);
        match ihsize {
            40 | 56 | 64 | 108 | 124 => {
                width = self.bytes.get_u32_le();
                height = self.bytes.get_u32_le();
            }
            12 => {
                width = self.bytes.get_u16_le() as u32;
                height = self.bytes.get_u16_le() as u32;
            }
            _ => {
                return Err(BmpDecoderErrors::GenericStatic(
                    "Unknown information header size"
                ))
            }
        }

        self.flip_vertically = (height as i32) > 0;

        self.height = (height as i32).unsigned_abs() as usize;
        self.width = width as usize;

        if self.height > self.options.get_max_height() {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "height",
                self.options.get_max_height(),
                self.height
            ));
        }

        if self.width > self.options.get_max_width() {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "width",
                self.options.get_max_width(),
                self.width
            ));
        }

        info!("Width: {}", self.width);
        info!("Height: {}", self.height);

        // planes
        if self.bytes.get_u16_le() != 1 {
            return Err(BmpDecoderErrors::GenericStatic("Invalid BMP header"));
        }

        let depth = self.bytes.get_u16_le();

        let compression = if hsize >= 40 {
            match BmpCompression::from_u32(self.bytes.get_u32_le()) {
                Some(c) => c,
                None => {
                    return Err(BmpDecoderErrors::GenericStatic(
                        "Unsupported BMP compression scheme"
                    ))
                }
            }
        } else {
            BmpCompression::RGB
        };
        if compression == BmpCompression::BITFIELDS {
            self.bytes.skip(20);

            self.rgb_bitfields[0] = self.bytes.get_u32_le();
            self.rgb_bitfields[1] = self.bytes.get_u32_le();
            self.rgb_bitfields[2] = self.bytes.get_u32_le();

            if ihsize > 40 {
                self.rgb_bitfields[3] = self.bytes.get_u32_le();
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
            return Err(GenericStatic("Unsupported Pixel format"));
        }

        let p = self.hsize.wrapping_sub(self.ihszie).wrapping_sub(14);

        if self.pix_fmt == BmpPixelFormat::PAL8 {
            let mut colors = 1_u32 << depth;

            if ihsize >= 36 {
                self.bytes.set_position(46);
                let t = self.bytes.get_u32_le() as i32;

                if t < 0 || t > (1 << depth) {
                    let msg = format!("Incorrect number of colors {} for depth {}", t, depth);
                    return Err(BmpDecoderErrors::Generic(msg));
                } else if t != 0 {
                    colors = t as u32;
                }
            } else {
                colors = 256.min(p / 3);
            }
            // palette location
            self.bytes.set_position((14 + ihsize) as usize);

            // OS/2 bitmap, 3 bytes per palette entry
            if hsize == 12 {
                if p < colors * 3 {
                    return Err(BmpDecoderErrors::GenericStatic("Invalid Palette entries"));
                }

                self.palette.resize(256, PaletteEntry::default());

                self.palette.iter_mut().take(colors as usize).for_each(|x| {
                    let [b, g, r] = self.bytes.get_fixed_bytes_or_err::<3>().unwrap();

                    x.red = r;
                    x.green = g;
                    x.blue = b;
                });
            } else {
                self.palette.resize(256, PaletteEntry::default());

                self.palette.iter_mut().take(colors as usize).for_each(|x| {
                    let [b, g, r, _] = self.bytes.get_fixed_bytes_or_err::<4>().unwrap();

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

        info!("Pixel format : {:?}", self.pix_fmt);
        info!("Compression  : {:?}", compression);

        self.comp = compression;
        self.depth = depth;
        self.ihszie = ihsize;
        self.hsize = hsize;
        self.bytes.set_position(hsize as usize);

        self.decoded_headers = true;

        Ok(())
    }

    pub fn output_buf_size(&self) -> Option<usize> {
        if !self.decoded_headers {
            return None;
        }
        Some(
            self.width
                .checked_mul(self.height)
                .unwrap()
                .checked_mul(self.pix_fmt.num_components())
                .unwrap()
        )
    }

    /// Return the BMP bit depth
    ///
    /// This is always [BitDepth::Eight](zune_core::bit_depth::BitDepth::Eight)
    /// since it's the only one BMP supports
    ///
    /// Images with less than 8 bits per pixel are usually scaled to be eight bits
    pub fn get_depth(&self) -> BitDepth {
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
    pub fn get_dimensions(&self) -> Option<(usize, usize)> {
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
    pub fn get_colorspace(&self) -> Option<ColorSpace> {
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
        let mut output = vec![0_u8; self.output_buf_size().unwrap()];

        self.decode_into(&mut output)?;

        Ok(output)
    }

    /// Decode an encoded image into a buffer or return an error
    /// if something bad occured
    ///
    /// Also see [`decode_into`](Self::decode_into) which decodes into
    /// a pre-allocated buffer
    pub fn decode_into(&mut self, buf: &mut [u8]) -> Result<(), BmpDecoderErrors> {
        self.decode_headers()?;

        let output_size = self.output_buf_size().unwrap();

        let buf = &mut buf[0..output_size];

        let pad = (((-(self.width as i32)) as u32) & 3) as usize;

        if self.comp == BmpCompression::RLE4 || self.comp == BmpCompression::RLE8 {
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
                        let in_bytes = self.bytes.remaining_bytes();

                        self.expand_palette(in_bytes, buf);
                        // do not flip for palette, orientation is good
                        self.flip_vertically = false;
                    } else {
                        // copy
                        let bytes = self.bytes.remaining_bytes();

                        if bytes.len() < (output_size * 8) / usize::from(self.depth) {
                            return Err(BmpDecoderErrors::TooSmallBuffer(output_size, bytes.len()));
                        }

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
                                for (out, input) in buf
                                    .rchunks_exact_mut(pad_size)
                                    .zip(bytes.chunks_exact(pad_size))
                                {
                                    for (a, b) in out.chunks_exact_mut(4).zip(input.chunks_exact(4))
                                    {
                                        a.copy_from_slice(b);
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
                                    assert_eq!(
                                        pad, 0,
                                        "An image with a depth of 32 should not have padding bytes"
                                    );
                                    for (out, input) in buf
                                        .rchunks_exact_mut(pad_size)
                                        .zip(bytes.chunks_exact(pad_size))
                                    {
                                        for (a, b) in
                                            out.chunks_exact_mut(4).zip(input.chunks_exact(4))
                                        {
                                            let v = u32::from_le_bytes(b.try_into().unwrap());

                                            a[0] = shift_signed(v & mr, rshift, rcount) as u8;
                                            a[1] = shift_signed(v & mg, gshift, gcount) as u8;
                                            a[2] = shift_signed(v & mb, bshift, bcount) as u8;
                                            a[3] = shift_signed(v & ma, ashift, acount) as u8;
                                        }
                                    }
                                } else if self.depth == 16 {
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
                                    for (out, input) in buf
                                        .rchunks_exact_mut(pad_size)
                                        .zip(bytes.chunks_exact(input_bytes_per_width * 2))
                                    {
                                        for (a, b) in out
                                            .chunks_exact_mut(self.pix_fmt.num_components())
                                            .zip(input.chunks_exact(2))
                                        {
                                            let v = u32::from(u16::from_le_bytes(
                                                b.try_into().unwrap()
                                            ));

                                            a[0] = shift_signed(v & mr, rshift, rcount) as u8;
                                            a[1] = shift_signed(v & mg, gshift, gcount) as u8;
                                            a[2] = shift_signed(v & mb, bshift, bcount) as u8;

                                            if a.len() > 3 {
                                                // handle alpha channel
                                                a[3] = shift_signed(v & ma, ashift, acount) as u8;
                                            }
                                        }
                                    }
                                }
                            }

                            self.flip_vertically = false;
                        } else {
                            // bmp rounds up each line to be a multiple of 4, padding the end if necessary
                            // remove padding bytes

                            let pad_size = self.width * self.pix_fmt.num_components();

                            for (out, input) in buf
                                .chunks_exact_mut(pad_size)
                                .zip(bytes.chunks_exact(pad_size + pad))
                            {
                                out.copy_from_slice(&input[..pad_size]);
                            }
                        }
                    }
                }
                1 | 2 | 4 => {
                    if self.pix_fmt != BmpPixelFormat::PAL8 {
                        return Err(BmpDecoderErrors::GenericStatic(
                            "Bit Depths less than 8 must have a palette"
                        ));
                    }
                    let index_bytes = self.bytes.remaining_bytes();
                    let width_bytes = ((self.width + 7) >> 3) << 3;
                    let in_width_bytes = ((self.width * usize::from(self.depth)) + 7) / 8;

                    let scanline_size = width_bytes * 3;
                    let mut scanline_bytes = vec![0_u8; scanline_size];

                    for (in_bytes, out_bytes) in index_bytes
                        .chunks_exact(in_width_bytes)
                        .zip(buf.rchunks_exact_mut((3 + usize::from(self.is_alpha)) * self.width))
                    {
                        expand_bits_to_byte(
                            self.depth as usize,
                            true,
                            in_bytes,
                            &mut scanline_bytes
                        );

                        self.expand_palette(&scanline_bytes, out_bytes);
                    }
                    self.flip_vertically = false;
                }
                d => panic!("Unhandled depth {}", d)
            }
        }

        if self.flip_vertically {
            let length = buf.len() / 2;

            let (in_img_top, in_img_bottom) = buf.split_at_mut(length);

            for (in_dim, out_dim) in in_img_top.iter_mut().zip(in_img_bottom.iter_mut().rev()) {
                core::mem::swap(in_dim, out_dim);
            }
        }

        Ok(())
    }

    fn expand_palette(&self, in_bytes: &[u8], buf: &mut [u8]) {
        let palette: &[PaletteEntry; 256] = &self.palette[0..256].try_into().unwrap();

        let pad = (((-(self.width as i32)) as u32) & 3) as usize;

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
