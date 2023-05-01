/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec::Vec;
use alloc::{format, vec};

use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::common::{BmpCompression, BmpPixelFormat};
use crate::BmpDecoderErrors;
use crate::BmpDecoderErrors::GenericStatic;

/// Probe some bytes to see
/// if they consist of a BMP image
pub fn probe_bmp(bytes: &[u8]) -> bool
{
    if let Some(magic_bytes) = bytes.get(0..2)
    {
        if magic_bytes == b"BM"
        {
            // skip file_size   -> 4
            // skip reserved    -> 4
            // skip data offset -> 4
            // read sz
            if let Some(sz) = bytes.get(14)
            {
                let sz = *sz;

                return sz == 12 || sz == 40 || sz == 56 || sz == 108 || sz == 124;
            }
        }
    }
    false
}

#[derive(Clone, Copy, Default, Debug)]
struct PaletteEntry
{
    red:   u8,
    green: u8,
    blue:  u8,
    alpha: u8
}

pub struct BmpDecoder<'a>
{
    bytes:           ZByteReader<'a>,
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
    is_alpha:        bool
}

impl<'a> BmpDecoder<'a>
{
    pub fn new(data: &'a [u8]) -> BmpDecoder<'a>
    {
        BmpDecoder::new_with_options(data, DecoderOptions::default())
    }
    pub fn new_with_options(data: &'a [u8], options: DecoderOptions) -> BmpDecoder<'a>
    {
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
            is_alpha: false
        }
    }
    #[rustfmt::skip]
    pub fn decode_headers(&mut self) -> Result<(), BmpDecoderErrors>
    {
        if self.decoded_headers
        {
            return Ok(());
        }
        if !self.bytes.has(14)
        {
            // too short input
            return Err(BmpDecoderErrors::TooSmallBuffer(14, self.bytes.remaining()));
        }
        // discard

        if self.bytes.get_u8() != b'B' || self.bytes.get_u8() != b'M'
        {
            return Err(BmpDecoderErrors::InvalidMagicBytes);
        }
        let fszie = (self.bytes.get_u32_le()) as usize;

        if self.bytes.len() < fszie
        {
            return Err(BmpDecoderErrors::TooSmallBuffer(fszie, self.bytes.len()));
        }
        // skip 4 reserved bytes
        self.bytes.skip(4);

        let hsize = self.bytes.get_u32_le();
        let ihsize = self.bytes.get_u32_le();

        if ihsize.saturating_add(14) > hsize
        {
            return Err(BmpDecoderErrors::GenericStatic("Invalid header size"));
        }
        let (width, height);
        match ihsize
        {
            40 | 56 | 64 | 108 | 124 =>
                {
                    width = self.bytes.get_u32_le();
                    height = self.bytes.get_u32_le();
                }
            12 =>
                {
                    width = self.bytes.get_u16_le() as u32;
                    height = self.bytes.get_u16_le() as u32;
                }
            _ =>
                {
                    return Err(BmpDecoderErrors::GenericStatic(
                        "Unknown information header size"
                    ))
                }
        }

        self.flip_vertically = (height as i32) > 0;

        self.height = (height as i32).unsigned_abs() as usize;
        self.width = width as usize;

        if self.height > self.options.get_max_height()
        {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "height",
                self.options.get_max_height(),
                self.height,
            ));
        }

        if self.width > self.options.get_max_width()
        {
            return Err(BmpDecoderErrors::TooLargeDimensions(
                "width",
                self.options.get_max_width(),
                self.width,
            ));
        }

        info!("Width: {}", self.width);
        info!("Height: {}", self.height);

        // planes
        if self.bytes.get_u16_le() != 1
        {
            return Err(BmpDecoderErrors::GenericStatic("Invalid BMP header"));
        }

        let depth = self.bytes.get_u16_le();

        let compression = if hsize >= 40
        {
            match BmpCompression::from_u32(self.bytes.get_u32_le())
            {
                Some(c) => c,
                None =>
                    {
                        return Err(BmpDecoderErrors::GenericStatic(
                            "Unsupported BMP compression scheme"
                        ))
                    }
            }
        } else {
            BmpCompression::RGB
        };
        if compression == BmpCompression::BITFIELDS
        {
            self.bytes.skip(20);

            self.rgb_bitfields[0] = self.bytes.get_u32_le();
            self.rgb_bitfields[1] = self.bytes.get_u32_le();
            self.rgb_bitfields[2] = self.bytes.get_u32_le();

            if ihsize > 40
            {
                self.rgb_bitfields[3] = self.bytes.get_u32_le();
            }
        }

        match depth
        {
            32 =>
                {
                    if compression == BmpCompression::BITFIELDS
                    {
                        let alpha = self.rgb_bitfields[3] != 0;

                        if self.rgb_bitfields[0] == 0xFF000000
                            && self.rgb_bitfields[1] == 0x00FF0000
                            && self.rgb_bitfields[2] == 0x0000FF00
                        {
                            self.pix_fmt =
                                if alpha { BmpPixelFormat::ABGR } else { BmpPixelFormat::OBGR };
                        } else if self.rgb_bitfields[0] == 0x00FF0000
                            && self.rgb_bitfields[1] == 0x0000FF00
                            && self.rgb_bitfields[2] == 0x000000FF
                        {
                            self.pix_fmt =
                                if alpha { BmpPixelFormat::BGRA } else { BmpPixelFormat::BGRO };
                        } else if self.rgb_bitfields[0] == 0x0000FF00
                            && self.rgb_bitfields[1] == 0x00FF0000
                            && self.rgb_bitfields[2] == 0xFF000000
                        {
                            self.pix_fmt =
                                if alpha { BmpPixelFormat::ARGB } else { BmpPixelFormat::ORGB };
                        } else if self.rgb_bitfields[0] == 0x000000FF
                            && self.rgb_bitfields[1] == 0x0000FF00
                            && self.rgb_bitfields[2] == 0x00FF0000
                        {
                            self.pix_fmt = if alpha { BmpPixelFormat::RGBA } else { BmpPixelFormat::RGB0 };
                        } else {
                            let message = format!(
                                "Unknown bitfields {:x} {:x} {:x}",
                                self.rgb_bitfields[0], self.rgb_bitfields[1], self.rgb_bitfields[2]
                            );
                            return Err(BmpDecoderErrors::Generic(message));
                        }
                    } else {
                        self.pix_fmt = BmpPixelFormat::BGRA;
                    }
                }
            24 => self.pix_fmt = BmpPixelFormat::RGB,
            16 =>
                {
                    if compression == BmpCompression::RGB
                    {
                        self.pix_fmt = BmpPixelFormat::RGB555
                    } else if compression == BmpCompression::BITFIELDS
                    {
                        if self.rgb_bitfields[0] == 0xF800
                            && self.rgb_bitfields[1] == 0x07E0
                            && self.rgb_bitfields[2] == 0x001F
                        {
                            self.pix_fmt = BmpPixelFormat::RGB565;
                        } else if self.rgb_bitfields[0] == 0x7C00
                            && self.rgb_bitfields[1] == 0x03E0
                            && self.rgb_bitfields[2] == 0x001F
                        {
                            self.pix_fmt = BmpPixelFormat::RGB555;
                        } else if self.rgb_bitfields[0] == 0x0F00
                            && self.rgb_bitfields[1] == 0x00F0
                            && self.rgb_bitfields[2] == 0x000F
                        {
                            self.pix_fmt = BmpPixelFormat::RGB444;
                        } else {
                            let message = format!(
                                "Unknown bitfields {:x} {:x} {:x}",
                                self.rgb_bitfields[0], self.rgb_bitfields[1], self.rgb_bitfields[2]
                            );
                            return Err(BmpDecoderErrors::Generic(message));
                        }
                    }
                }
            8 =>
                {
                    if hsize.wrapping_sub(ihsize).wrapping_sub(14) > 0
                    {
                        self.pix_fmt = BmpPixelFormat::PAL8;
                    } else {
                        self.pix_fmt = BmpPixelFormat::GRAY8;
                    }
                }
            1 | 4 =>
                {
                    if hsize.wrapping_sub(ihsize).wrapping_sub(14) > 0
                    {
                        self.pix_fmt = BmpPixelFormat::PAL8;
                    } else {
                        let message = format!("Unknown palette for {}-color bmp", 1 << depth);
                        return Err(BmpDecoderErrors::Generic(message));
                    }
                }

            _ =>
                {
                    let message = format!("Depth {depth} unsupported");
                    return Err(BmpDecoderErrors::Generic(message));
                }
        };
        if self.pix_fmt == BmpPixelFormat::None
        {
            return Err(GenericStatic("Unsupported Pixel format"));
        }

        let p = self.hsize.wrapping_sub(self.ihszie).wrapping_sub(14);

        if self.pix_fmt == BmpPixelFormat::PAL8
        {
            let mut colors = 1_u32 << depth;

            if ihsize >= 36
            {
                self.bytes.set_position(46);
                let t = self.bytes.get_u32_le() as i32;

                if t < 0 || t > (1 << depth)
                {
                    let msg = format!("Incorrect number of colors {} for depth {}", t, depth);
                    return Err(BmpDecoderErrors::Generic(msg));
                } else if t != 0
                {
                    colors = t as u32;
                }
            } else {
                colors = 256.min(p / 3);
            }
            // palette location
            self.bytes.set_position((14 + ihsize) as usize);

            // OS/2 bitmap, 3 bytes per palette entry
            if hsize == 12
            {
                if p < colors * 3
                {
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
                self.is_alpha = true;
            }
        }

        self.comp = compression;
        self.depth = depth;
        self.ihszie = ihsize;
        self.hsize = hsize;
        self.bytes.set_position(hsize as usize);

        self.decoded_headers = true;

        Ok(())
    }

    pub fn output_buf_size(&self) -> Option<usize>
    {
        if !self.decoded_headers
        {
            return None;
        }
        Some(
            self.width
                .checked_mul(self.height)
                .unwrap()
                .checked_mul(self.pix_fmt.num_components(self.is_alpha))
                .unwrap()
        )
    }

    /// Return the BMP bit depth
    pub fn get_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }
    pub fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if !self.decoded_headers
        {
            return None;
        }
        Some((self.width, self.height))
    }
    /// Convert to the needed colorspace
    pub fn get_colorspace(&self) -> Option<ColorSpace>
    {
        if !self.decoded_headers
        {
            return None;
        }

        Some(self.pix_fmt.into_colorspace(self.is_alpha))
    }
    pub fn decode(&mut self) -> Result<Vec<u8>, BmpDecoderErrors>
    {
        self.decode_headers()?;
        let mut output = vec![0_u8; self.output_buf_size().unwrap()];

        self.decode_into(&mut output)?;

        Ok(output)
    }

    pub fn decode_into(&mut self, buf: &mut [u8]) -> Result<(), BmpDecoderErrors>
    {
        self.decode_headers()?;
        let output_size = self.output_buf_size().unwrap();

        let buf = &mut buf[0..output_size];

        let pad = ((-(self.width as i32)) as u32) & 3;

        if self.depth == 1
        {
            self.width = (self.width + 7) >> 3;
        }
        else if self.depth == 4
        {
            self.width = (self.width + 1) >> 1;
        }

        if self.comp == BmpCompression::RLE4 || self.comp == BmpCompression::RLE8
        {
        }
        else
        {
            match self.depth
            {
                8 | 24 | 32 =>
                {
                    if self.pix_fmt == BmpPixelFormat::PAL8
                    {
                        self.expand_palette(buf);
                        // do not flip for palette, orientation is good
                        self.flip_vertically = false;
                    }
                    else
                    {
                        // copy
                        let bytes = self.bytes.remaining_bytes();

                        if bytes.len() < output_size
                        {
                            return Err(BmpDecoderErrors::TooSmallBuffer(output_size, bytes.len()));
                        }

                        // bmp rounds up each line to be a multiple of 4, padding the end if necessary
                        // remove padding bytes
                        for i in 0..self.height
                        {
                            let start = i * self.width * self.pix_fmt.num_components(self.is_alpha);
                            let end =
                                (i + 1) * self.width * self.pix_fmt.num_components(self.is_alpha);

                            let offset = pad as usize * i;

                            buf[start..end].copy_from_slice(&bytes[start + offset..end + offset]);
                        }
                    }
                }
                1 =>
                {
                    if self.pix_fmt == BmpPixelFormat::PAL8
                    {
                        self.expand_palette(buf);
                        // do not flip for simple rle
                        self.flip_vertically = false;
                    }
                }
                _ => unreachable!()
            }
        }

        if self.flip_vertically
        {
            let length = buf.len() / 2;

            let (in_img_top, in_img_bottom) = buf.split_at_mut(length);

            for (in_dim, out_dim) in in_img_top.iter_mut().zip(in_img_bottom.iter_mut().rev())
            {
                core::mem::swap(in_dim, out_dim);
            }
        }

        Ok(())
    }

    fn expand_palette(&self, buf: &mut [u8])
    {
        let palette: &[PaletteEntry; 256] = &self.palette[0..256].try_into().unwrap();

        // copy
        let bytes = self.bytes.remaining_bytes();
        let pad = ((-(self.width as i32)) as u32) & 3;

        if self.is_alpha
        {
            // bmp rounds up each line to be a multiple of 4, padding the end if necessary
            // remove padding bytes

            for (i, height_chunk) in buf
                .rchunks_exact_mut(self.width * 4)
                .enumerate()
                .take(self.height)
            {
                let start = i * self.width;
                let end = (i + 1) * self.width;

                let offset = pad as usize * i;

                for (pal_byte, chunks) in bytes[start + offset..end + offset]
                    .iter()
                    .zip(height_chunk.chunks_exact_mut(4))
                {
                    let entry = palette[usize::from(*pal_byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                    chunks[3] = entry.alpha;
                }
            }
        }
        else
        {
            for (i, height_chunk) in buf
                .rchunks_exact_mut(self.width * 3)
                .enumerate()
                .take(self.height)
            {
                let start = i * self.width;
                let end = (i + 1) * self.width;

                let offset = pad as usize * i;

                for (pal_byte, chunks) in bytes[start + offset..end + offset]
                    .iter()
                    .zip(height_chunk.chunks_exact_mut(3))
                {
                    let entry = palette[usize::from(*pal_byte)];

                    chunks[0] = entry.red;
                    chunks[1] = entry.green;
                    chunks[2] = entry.blue;
                }
            }
        }
    }
}
