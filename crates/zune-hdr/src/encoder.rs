/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Radiance HDR encoder

use alloc::{format, vec};
use std::collections::HashMap;

use zune_core::bytestream::{ZByteIoError, ZByteWriterTrait, ZWriter};
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

use crate::errors::HdrEncodeErrors;

/// A simple HDR encoder
///
/// Data is expected to be in `f32` and its size should be
/// `width*height*3`
pub struct HdrEncoder<'a> {
    data:    &'a [f32],
    headers: Option<&'a HashMap<String, String>>,
    options: EncoderOptions
}

impl<'a> HdrEncoder<'a> {
    /// Create a new HDR encoder context that can encode
    /// the provided data
    ///
    /// # Arguments
    ///  - `data`: Data to encode
    ///  - `options`: Contains metadata for data, including width and height
    pub fn new(data: &'a [f32], options: EncoderOptions) -> HdrEncoder<'a> {
        Self {
            data,
            headers: None,
            options
        }
    }
    /// Add extra headers to be encoded  with the image
    ///
    /// This must be called before you call [`encode`](crate::encoder::HdrEncoder::encode)
    /// otherwise it will have no effect.
    ///
    /// # Arguments:
    /// - headers: A hashmap containing keys and values, the values will be encoded as key=value
    /// in the hdr header before encoding
    pub fn add_headers(&mut self, headers: &'a HashMap<String, String>) {
        self.headers = Some(headers)
    }

    /// Calculate buffer with padding size needed for
    /// encoding this into a vec
    ///
    /// This is a given upper limit and doesn't specifically mean that
    /// your buffer should exactly be that size.
    ///
    /// The size of the output will depend on the nature of your data
    pub fn expected_buffer_size(&self) -> Option<usize> {
        self.options
            .width()
            .checked_mul(self.options.height())?
            .checked_mul(4)?
            .checked_add(1024)
    }

    /// Encode into a sink
    ///
    /// The encoder expects colorspace to be in RGB and the  length to match
    ///
    /// otherwise it's an error to try and encode
    ///
    /// The floating point data is expected to be normalized between 0.0 and 1.0, it will not be clipped
    /// if not in this range
    ///
    /// The size of output cannot be determined up until compression is over hence
    /// we cannot be sure if the output buffer will be enough.
    ///
    /// The library provides [expected_buffer_size](crate::HdrEncoder::expected_buffer_size) which
    /// guarantees that if your buffer is that big, encoding will always succeed
    ///
    /// # Arguments:
    /// - out: The output buffer to write bytes into
    ///
    /// # Returns
    /// - Ok(usize):  The number of bytes written into out
    /// - Err(HdrEncodeErrors): An error if something occurred
    ///
    /// # Examples
    /// - Encode a black image of 10x10
    ///```
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_core::options::EncoderOptions;
    /// use zune_hdr::HdrEncoder;
    /// let w = 10;
    /// let h = 10;
    /// let comp = 3;
    /// let data = vec![0.0_f32;w*h*comp];
    /// let opts = EncoderOptions::new(w,h,ColorSpace::RGB,BitDepth::Float32);
    /// let encoder = HdrEncoder::new(&data,opts);
    /// // create output buffer , this is the upper limit on it
    /// let mut output = Vec::with_capacity(encoder.expected_buffer_size().unwrap());
    /// let size = encoder.encode(&mut output).unwrap();
    ///```  
    ///
    /// - Encode but directly write to a file
    ///```no_run
    /// use std::fs::OpenOptions;
    /// use std::io::{BufReader, BufWriter};
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_core::options::EncoderOptions;
    /// use zune_hdr::HdrEncoder;
    /// let w = 10;
    /// let h = 10;
    /// let comp = 3;
    /// let data = vec![0.0_f32;w*h*comp];
    /// let opts = EncoderOptions::new(w,h,ColorSpace::RGB,BitDepth::Float32);
    /// let encoder = HdrEncoder::new(&data,opts);
    /// // create output buffer , this is the upper limit on it
    /// let mut output = OpenOptions::new().create(true).write(true).truncate(true).open("./black.hdr").unwrap();
    /// let mut buffered_output = BufWriter::new(output);
    /// let size = encoder.encode(&mut buffered_output).unwrap();
    ///
    /// ```
    pub fn encode<T: ZByteWriterTrait>(&self, out: T) -> Result<usize, HdrEncodeErrors> {
        let expected = self
            .options
            .width()
            .checked_mul(self.options.height())
            .ok_or(HdrEncodeErrors::Static("overflow detected"))?
            .checked_mul(3)
            .ok_or(HdrEncodeErrors::Static("overflow detected"))?;

        let found = self.data.len();

        if expected != found {
            return Err(HdrEncodeErrors::WrongInputSize(expected, found));
        }
        if self.options.colorspace() != ColorSpace::RGB {
            return Err(HdrEncodeErrors::UnsupportedColorspace(
                self.options.colorspace()
            ));
        }
        let mut writer = ZWriter::new(out);
        // reserve space
        let size = self
            .expected_buffer_size()
            .ok_or(HdrEncodeErrors::Static("overflow detected"))?;
        writer.reserve(size)?;
        // write headers
        {
            writer.write_all(b"#?RADIANCE\n")?;
            writer.write_all(b"SOFTWARE=zune-hdr\n")?;
            if let Some(headers) = self.headers {
                for (k, v) in headers {
                    writer.write_all(format!("{}={}\n", k, v).as_bytes())?;
                }
            }
            writer.write_all(b"FORMAT=32-bit_rle_rgbe\n\n")?;

            // write lengths
            let length_format =
                format!("-Y {} +X {}\n", self.options.height(), self.options.width());

            writer.write_all(length_format.as_bytes())?;
        }
        let width = self.options.width();

        let scanline_stride = width * 3;

        let mut in_scanline = vec![0_u8; width * 4]; // RGBE

        for scanline in self.data.chunks_exact(scanline_stride) {
            if !(8..=0x7fff).contains(&width) {
                for (pixels, out) in scanline
                    .chunks_exact(3)
                    .zip(in_scanline.chunks_exact_mut(4))
                {
                    float_to_rgbe(pixels.try_into().unwrap(), out.try_into().unwrap());
                }
                writer.write_all(&in_scanline)?;
            } else {
                let bytes = [2, 2, (width >> 8) as u8, (width & 255) as u8];
                writer.write_const_bytes(&bytes)?;

                for (pixels, out) in scanline
                    .chunks_exact(3)
                    .zip(in_scanline.chunks_exact_mut(4))
                {
                    float_to_rgbe(pixels.try_into().unwrap(), out.try_into().unwrap());
                }
                for i in 0..4 {
                    rle(&in_scanline[i..], &mut writer, width)?;
                }
            }
        }
        // truncate position to where we reached
        let position = writer.bytes_written();
        Ok(position)
    }
}

fn rle<T: ZByteWriterTrait>(
    data: &[u8], writer: &mut ZWriter<T>, width: usize
) -> Result<(), ZByteIoError> {
    const MIN_RLE: usize = 4;
    let mut cur = 0;

    while cur < width {
        let mut run_count = 0;
        let mut old_run_count = 0;
        let mut beg_run = cur;
        let mut buf: [u8; 2] = [0; 2];

        while run_count < MIN_RLE && beg_run < width {
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

        if (old_run_count > 1) && (old_run_count == beg_run - cur) {
            buf[0] = (128 + old_run_count) as u8;
            buf[1] = data[cur * 4];
            writer.write_all(&buf)?;
            cur = beg_run;
        }

        while cur < beg_run {
            let nonrun_count = 128.min(beg_run - cur);
            buf[0] = nonrun_count as u8;
            writer.write_u8(buf[0]);
            for i in 0..nonrun_count {
                writer.write_u8_err(data[(cur + i) * 4])?;
            }

            cur += nonrun_count;
        }

        if run_count >= MIN_RLE {
            buf[0] = (128 + run_count) as u8;
            buf[1] = data[beg_run * 4];
            writer.write_all(&buf)?;
            cur += run_count;
        }
    }
    Ok(())
}

fn float_to_rgbe(rgb: &[f32; 3], rgbe: &mut [u8; 4]) {
    let v = rgb.iter().fold(f32::MIN, |x, y| x.max(*y));

    if v > 1e-32 {
        let old_v = v;
        let (mut v, e) = frexp(v);
        v = v * 256. / old_v;
        rgbe[0] = (rgb[0] * v).clamp(0.0, 255.0) as u8;
        rgbe[1] = (rgb[1] * v).clamp(0.0, 255.0) as u8;
        rgbe[2] = (rgb[2] * v).clamp(0.0, 255.0) as u8;

        rgbe[3] = (e.wrapping_add(128)) as u8;
    } else {
        rgbe.fill(0);
    }
}

#[rustfmt::skip]
pub fn abs(num: f32) -> f32
{
    // standard compliant
    // handles NAN and infinity.
    // pretty cool
    f32::from_bits(num.to_bits() & (i32::MAX as u32))
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

fn floor(num: f32) -> f32 {
    if num.is_nan() || num.is_infinite() {
        /* handle infinities and nan */
        return num;
    }
    let n = num as u64;
    let d = n as f32;

    if d == num || num >= 0.0 {
        d
    } else {
        d - 1.0
    }
}

/// Fast log2 approximation
/// (we really don't need that accurate)
#[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)]
fn fast_log2(x: f32) -> f32 {
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
fn frexp(s: f32) -> (f32, i32) {
    // from https://stackoverflow.com/a/55696477
    if 0.0 == s {
        (s, 0)
    } else {
        let lg = fast_log2(abs(s));
        let lg_floor = floor(lg);
        // Note: This is the only reason we need the standard library
        // I haven't found a goof exp2 function, fast_exp2 doesn't work
        // and libm/musl exp2 introduces visible color distortions and is slow, so for
        // now let's stick to whatever the platform provides
        let x = (lg - lg_floor - 1.0).exp2();
        let exp = lg_floor + 1.0;
        (signum(s) * x, exp as i32)
    }
}
