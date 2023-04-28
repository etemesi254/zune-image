/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Radiance HDR encoder

use alloc::vec::Vec;
use alloc::{format, vec};

use zune_core::bytestream::ZByteWriter;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

use crate::errors::HdrEncodeErrors;

/// A simple HDR encoder
pub struct HdrEncoder<'a>
{
    data:    &'a [f32],
    options: EncoderOptions
}

impl<'a> HdrEncoder<'a>
{
    /// Create a new HDR encoder context that can encode
    /// the provided data
    ///
    /// # Arguments
    ///  - `data`: Data to encode
    ///  - `options`: Contains metadata for data, including width and height
    pub fn new(data: &'a [f32], options: EncoderOptions) -> HdrEncoder<'a>
    {
        Self { data, options }
    }

    /// Calculate buffer with padding size needed for
    /// encoding this into a vec
    fn expected_buffer_size(&self) -> usize
    {
        self.options
            .get_width()
            .checked_mul(self.options.get_height())
            .unwrap()
            .checked_mul(4)
            .unwrap()
            .checked_add(1024)
            .unwrap()
    }

    /// Encode data in HDR format
    ///
    /// The encoder expects colorspace to be in RGB and the  length to match
    ///
    /// otherwise it's an error to try and encode
    ///
    /// The floating point data is expected to be normalized between 0.0 and 1.0, it will be clipped
    /// if not in this range
    pub fn encode(&self) -> Result<Vec<u8>, HdrEncodeErrors>
    {
        let expected = self
            .options
            .get_width()
            .checked_mul(self.options.get_height())
            .unwrap()
            .checked_mul(3) // RGB
            .unwrap();
        let found = self.data.len();

        if expected != found
        {
            return Err(HdrEncodeErrors::WrongInputSize(expected, found));
        }
        if self.options.get_colorspace() != ColorSpace::RGB
        {
            return Err(HdrEncodeErrors::UnsupportedColorspace(
                self.options.get_colorspace()
            ));
        }

        let mut out = vec![0_u8; self.expected_buffer_size()];

        let mut writer = ZByteWriter::new(&mut out);

        // write headers
        {
            writer.write_all(b"#?RADIANCE\n").unwrap();
            writer.write_all(b"SOFTWARE=zune-hdr\n").unwrap();
            writer.write_all(b"FORMAT=32-bit_rle_rgbe\n\n").unwrap();

            // write lengths
            let length_format = format!(
                "-Y {} +X {}\n",
                self.options.get_height(),
                self.options.get_width()
            );

            writer.write_all(length_format.as_bytes()).unwrap();
        }
        let width = self.options.get_width();

        let scanline_stride = width * 3;

        let mut in_scanline = vec![0_u8; width * 4]; // RGBE

        for scanline in self.data.chunks_exact(scanline_stride)
        {
            if !(8..=0x7fff).contains(&width)
            {
                for (pixels, out) in scanline
                    .chunks_exact(3)
                    .zip(in_scanline.chunks_exact_mut(4))
                {
                    float_to_rgbe(pixels.try_into().unwrap(), out.try_into().unwrap());
                }
                writer.write_all(&in_scanline).unwrap();
            }
            else
            {
                writer.write_u8(2);
                writer.write_u8(2);
                writer.write_u8((width >> 8) as u8);
                writer.write_u8((width & 255) as u8);

                for (pixels, out) in scanline
                    .chunks_exact(3)
                    .zip(in_scanline.chunks_exact_mut(4))
                {
                    float_to_rgbe(pixels.try_into().unwrap(), out.try_into().unwrap());
                }
                for i in 0..4
                {
                    rle(&in_scanline[i..], &mut writer, width)
                }
            }
        }
        // truncate position to where we reached
        let position = writer.position();
        out.truncate(position);
        Ok(out)
    }
}

fn rle(data: &[u8], writer: &mut ZByteWriter, width: usize)
{
    const MIN_RLE: usize = 4;
    let mut cur = 0;

    while cur < width
    {
        let mut run_count = 0;
        let mut old_run_count = 0;
        let mut beg_run = cur;
        let mut buf: [u8; 2] = [0; 2];

        while run_count < MIN_RLE && beg_run < width
        {
            beg_run += run_count;
            old_run_count = run_count;
            run_count = 1;

            while (beg_run + run_count < width)
                && (run_count < 127)
                && (data[beg_run * 4] == data[(beg_run + run_count) * 4])
            {
                run_count += 1;
            }
        }

        if (old_run_count > 1) && (old_run_count == beg_run - cur)
        {
            buf[0] = (128 + old_run_count) as u8;
            buf[1] = data[cur * 4];
            writer.write_all(&buf).unwrap();
            cur = beg_run;
        }

        while cur < beg_run
        {
            let nonrun_count = 128.min(beg_run - cur);
            buf[0] = nonrun_count as u8;
            writer.write_u8(buf[0]);
            for i in 0..nonrun_count
            {
                writer.write_u8(data[(cur + i) * 4])
            }

            cur += nonrun_count;
        }

        if run_count >= MIN_RLE
        {
            buf[0] = (128 + run_count) as u8;
            buf[1] = data[beg_run * 4];
            writer.write_all(&buf).unwrap();
            cur += run_count;
        }
    }
}

fn float_to_rgbe(rgb: &[f32; 3], rgbe: &mut [u8; 4])
{
    let v = rgb.iter().fold(f32::MIN, |x, y| x.max(*y));

    if v > 1e-32
    {
        let old_v = v;
        let (mut v, e) = frexp(v);
        v = v * 256. / old_v;
        rgbe[0] = (rgb[0] * v).clamp(0.0, 255.0) as u8;
        rgbe[1] = (rgb[1] * v).clamp(0.0, 255.0) as u8;
        rgbe[2] = (rgb[2] * v).clamp(0.0, 255.0) as u8;

        rgbe[3] = (e.wrapping_add(128)) as u8;
    }
    else
    {
        rgbe.fill(0);
    }
}

#[rustfmt::skip]
pub fn abs(num: f32) -> f32
{
    if num < 0.0 { -num } else { num }
}

/// Implementation of signum that works in no_std
#[rustfmt::skip]
pub fn signum(num: f32) -> f32
{
    if num.is_nan() { f32::NAN } else if num.is_infinite()
    {
        if num.is_sign_positive() { 0.0 } else { -0.0 }
    } else if num > 0.0 { 1.0 } else { -1.0 }
}

/// Fast log2 approximation
/// (we really don't need that accurate)
#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn fast_log2(x: f32) -> f32
{
    /*
     * Fast log approximation from
     * https://github.com/romeric/fastapprox
     *
     * Some pretty good stuff.
     */
    let vx = x.to_bits();
    let mx = (vx & 0x007F_FFFF) | 0x3f00_0000;
    let mx_f = f32::from_bits(mx);

    let mut y = vx as f32;
    // 1/(1<<23)
    y *= 1.192_092_9e-7;

    y - 124.225_52 - 1.498_030_3 * mx_f - 1.725_88 / (0.352_088_72 + mx_f)
}

/// non standard frexp implementation
fn frexp(s: f32) -> (f32, i32)
{
    // from https://stackoverflow.com/a/55696477
    if 0.0 == s
    {
        (s, 0)
    }
    else
    {
        let lg = fast_log2(abs(s));
        let x = (lg - lg.floor() - 1.0).exp2();
        let exp = lg.floor() + 1.0;
        (signum(s) * x, exp as i32)
    }
}
