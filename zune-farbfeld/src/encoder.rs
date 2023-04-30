/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Encoding support for Farbfeld image format

use alloc::vec;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteWriter;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

/// Errors possible during encoding
pub enum FarbFeldEncoderErrors
{
    /// Too large dimensions, above 2^32.
    /// Farbfeld uses 4 bytes for width and height, if image cannot fit in it
    /// then it's undefined
    TooLargeDimensions(usize),
    /// Unsupported bit depth for Farbfeld. Farbfeld only supports 16 bit images
    /// any other image format is not supported
    UnsupportedBitDepth(BitDepth),
    /// Unsupported colorspace for Farbfeld. Farbfeld only supports RGBA images
    UnsupportedColorSpace(ColorSpace),
    /// Too short of an input buffer, the buffer size is not same as expected buffer
    /// size
    TooShortInput(usize, usize)
}

impl Debug for FarbFeldEncoderErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result
    {
        match self
        {
            FarbFeldEncoderErrors::TooLargeDimensions(dims) =>
            {
                writeln!(f, "Too large dimensions {dims}")
            }
            FarbFeldEncoderErrors::UnsupportedBitDepth(depth) =>
            {
                writeln!(f, "Unsupported bit depth {depth:?}")
            }
            FarbFeldEncoderErrors::UnsupportedColorSpace(color) =>
            {
                writeln!(f, "Unsupported color space {color:?}")
            }
            FarbFeldEncoderErrors::TooShortInput(expected, found) =>
            {
                writeln!(
                    f,
                    "Too short of input, expected {expected:?}, found {found:?}",
                )
            }
        }
    }
}

/// A FarbFeld encoder
///
/// The encoder's entry point is `new` which initializes the encoder
///
///
/// # NOTE.
///  
/// Data is expected to be in16 bit NATIVE ENDIAN in RGBA format
/// and BitDepth of 16, if not so this is an error.
///
/// If one has a vector/slice of [`u16`], one can use either `align_to`
/// or convert to native endian  or the [bytemuck] crate, or just do it
/// yourself with a simple loop.
///
/// [bytemuck]:https://docs.rs/bytemuck/latest/bytemuck/index.html
///
/// # Example
/// - Encodes a 10 by 4 RGBA image
/// ```
/// use zune_core::bit_depth::BitDepth;
/// use zune_core::colorspace::ColorSpace;
/// use zune_core::options::EncoderOptions;
/// use zune_farbfeld::FarbFeldEncoder;
/// //
/// let image:[u16;160] =std::array::from_fn(|c| c as u16);
///
///  // encoder options for depth and colorspace must be specified
///let options = EncoderOptions::default()
///     .set_width(10)
///     .set_height(4)
///     .set_depth(BitDepth::Sixteen)
///     .set_colorspace(ColorSpace::RGBA);
///
/// // we need u8 as an image but we created u16, so alias to u8
/// let (_,alias,_) = unsafe{image.align_to::<u8>()};
/// assert_eq!(alias.len(),320);
///
/// // encode to farbfeld format
/// // it expects native endian so no need to change
/// FarbFeldEncoder::new(&alias,options).encode().unwrap();
/// ```
pub struct FarbFeldEncoder<'a>
{
    data:    &'a [u8],
    options: EncoderOptions
}

impl<'a> FarbFeldEncoder<'a>
{
    /// Create a new encode which will encode the specified data
    /// whose format is contained in options
    ///
    /// # Arguments
    /// - data: The data to encode
    /// - options: Meta information about the image
    ///  contains image width, color and depth
    pub fn new(data: &'a [u8], options: EncoderOptions) -> FarbFeldEncoder<'a>
    {
        FarbFeldEncoder { data, options }
    }

    fn encode_headers(&self, stream: &mut ZByteWriter) -> Result<(), FarbFeldEncoderErrors>
    {
        // these routines panic because I need them
        // to panic as it is a me problem
        stream.write_all(b"farbfeld").unwrap();

        if (self.options.get_width() as u64) > u64::from(u32::MAX)
        {
            // error out
            return Err(FarbFeldEncoderErrors::TooLargeDimensions(
                self.options.get_width()
            ));
        }
        if (self.options.get_height() as u64) > u64::from(u32::MAX)
        {
            return Err(FarbFeldEncoderErrors::TooLargeDimensions(
                self.options.get_height()
            ));
        }
        // dimensions
        stream.write_u32_be(self.options.get_width() as u32);
        stream.write_u32_be(self.options.get_height() as u32);

        Ok(())
    }

    /// Encode the contents returning a vector containing
    /// encoded contents or an error if anything occurs
    pub fn encode(&self) -> Result<Vec<u8>, FarbFeldEncoderErrors>
    {
        if self.options.get_depth() != BitDepth::Sixteen
        {
            return Err(FarbFeldEncoderErrors::UnsupportedBitDepth(
                self.options.get_depth()
            ));
        }
        if self.options.get_colorspace() != ColorSpace::RGBA
        {
            return Err(FarbFeldEncoderErrors::UnsupportedColorSpace(
                self.options.get_colorspace()
            ));
        }

        let expected = calc_expected_size(self.options);
        let found = self.data.len();

        if expected != found
        {
            return Err(FarbFeldEncoderErrors::TooShortInput(expected, found));
        }

        let out_size = calc_out_size(self.options);

        let mut out = vec![0; out_size];

        let mut stream = ZByteWriter::new(&mut out);

        self.encode_headers(&mut stream)?;

        // write in big endian
        // chunk in two and write to stream
        for slice in self.data.chunks_exact(2)
        {
            let byte = u16::from_ne_bytes(slice.try_into().unwrap());
            stream.write_u16_be(byte)
        }

        assert!(!stream.eof());
        let position = stream.position();

        // truncate to how many bytes we wrote
        out.truncate(position);

        Ok(out)
    }
}

// should be 16 but 20 is to ensure we never hit EOF
// and the check at assert!(!stream.eof()); above
// will never be true(if it's true we have bigger problems)
const FARBFELD_HEADER_SIZE: usize = 20;

#[inline]
fn calc_out_size(options: EncoderOptions) -> usize
{
    options
        .get_width()
        .checked_mul(2)
        .unwrap()
        .checked_mul(options.get_height())
        .unwrap()
        .checked_mul(4)
        .unwrap()
        .checked_add(FARBFELD_HEADER_SIZE)
        .unwrap()
}

fn calc_expected_size(options: EncoderOptions) -> usize
{
    calc_out_size(options)
        .checked_sub(FARBFELD_HEADER_SIZE)
        .unwrap()
}
