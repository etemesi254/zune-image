//! A simple PSD reader.
//!
//! This crate features a simple and performant PSD reader
//! based on STB implementation.
//!
//! It currently does not support a lot of spec details.
//! Only extracting the image without respecting blend layers
//! and masks but such functionality will be added with time
//!
//!
use std::cmp::Ordering;

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::DecodingResult;

use crate::constants::{ColorModes, CompressionMethod, PSD_IDENTIFIER_BE};
use crate::errors::PSDDecodeErrors;

#[derive(Copy, Clone)]
pub struct ZunePSDOptions
{
    max_width:  usize,
    max_height: usize
}

impl Default for ZunePSDOptions
{
    fn default() -> Self
    {
        ZunePSDOptions {
            max_height: 1 << 17,
            max_width:  1 << 17
        }
    }
}

/// A simple Photoshop PSD reader.
///
/// This currently doesn't support layer flattening
/// but it's useful enough in that we can extract images
/// from it.
///
/// Further work will go onto adding a renderer that flattens
/// image pixels. But for now this is a good basis.
pub struct PSDDecoder<'a>
{
    width:          usize,
    height:         usize,
    decoded_header: bool,
    stream:         ZByteReader<'a>,
    options:        ZunePSDOptions,
    depth:          BitDepth,
    compression:    CompressionMethod,
    channel_count:  usize
}

impl<'a> PSDDecoder<'a>
{
    pub fn new(data: &'a [u8]) -> PSDDecoder<'a>
    {
        Self::new_with_options(data, ZunePSDOptions::default())
    }

    pub fn new_with_options(data: &'a [u8], options: ZunePSDOptions) -> PSDDecoder<'a>
    {
        PSDDecoder {
            width: 0,
            height: 0,
            decoded_header: false,
            stream: ZByteReader::new(data),
            options,
            depth: BitDepth::Eight,
            compression: CompressionMethod::NoCompression,
            channel_count: 0
        }
    }

    pub fn decode_headers(&mut self) -> Result<(), PSDDecodeErrors>
    {
        // Check identifier
        let magic = self.stream.get_u32_be_err()?;

        if magic != PSD_IDENTIFIER_BE
        {
            return Err(PSDDecodeErrors::WrongMagicBytes(magic));
        }

        //  file version
        let version = self.stream.get_u16_be_err()?;

        if version != 1
        {
            return Err(PSDDecodeErrors::UnsupportedFileType(version));
        }
        // Skip 6 reserved bytes
        self.stream.skip(6);
        // Read the number of channels (R, G, B, A, etc).
        let channel_count = self.stream.get_u16_be_err()?;

        if channel_count > 16
        {
            return Err(PSDDecodeErrors::UnsupportedChannelCount(channel_count));
        }
        self.channel_count = usize::from(channel_count);

        let height = self.stream.get_u32_be_err()? as usize;
        let width = self.stream.get_u32_be_err()? as usize;

        if width > self.options.max_width
        {
            return Err(PSDDecodeErrors::LargeDimensions(
                self.options.max_width,
                width
            ));
        }

        if height > self.options.max_height
        {
            return Err(PSDDecodeErrors::LargeDimensions(
                self.options.max_height,
                height
            ));
        }

        self.width = width;
        self.height = height;

        let depth = self.stream.get_u16_be_err()?;

        if depth != 8 && depth != 16
        {
            return Err(PSDDecodeErrors::UnsupportedBitDepth(depth));
        }
        let im_depth = match depth
        {
            8 => BitDepth::Eight,
            16 => BitDepth::Sixteen,
            _ => unreachable!()
        };

        self.depth = im_depth;

        let color_mode = self.stream.get_u16_be_err()?;

        let color_enum = ColorModes::from_int(color_mode);

        if color_enum != Some(ColorModes::RGB)
        {
            return Err(PSDDecodeErrors::UnsupportedColorFormat(color_enum));
        }

        // skip mode data
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes);

        // skip image resources
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes);

        // skip reserved data
        let bytes = self.stream.get_u32_be_err()? as usize;
        self.stream.skip(bytes);

        // find out if data is compressed
        let compression = self.stream.get_u16_be_err()?;
        if compression > 1
        {
            return Err(PSDDecodeErrors::UnknownCompression);
        }

        self.compression = CompressionMethod::from_int(compression).unwrap();

        self.decoded_header = true;

        Ok(())
    }

    /// Decode a PSD file extracting the image only
    ///
    /// Currently this does it without respect to  layers
    /// and such, only extracting the PSD image, hence might not be the
    /// most useful one.
    ///
    /// But such functionality will be added soon.
    pub fn decode(&mut self) -> Result<DecodingResult, PSDDecodeErrors>
    {
        self.decode_headers()?;

        let pixel_count = self.width * self.height;

        let mut result = match (self.compression, self.depth)
        {
            (CompressionMethod::RLE, BitDepth::Eight) =>
            {
                // RLE
                // Loop until you get the number of unpacked bytes you are expecting:
                //     Read the next source byte into n.
                //     If n is between 0 and 127 inclusive, copy the next n+1 bytes
                //     literally. Else if n is between -127 and -1 inclusive, copy the next
                //     byte -n+1 times. Else if n is 128, noop.
                // Endloop

                // The RLE-compressed data is preceded by a 2-byte data count for each row
                // in the data, which we're going to just skip.
                let skipped = self.height * self.channel_count * 2;
                self.stream.skip(skipped);

                let mut out_channel = vec![0; pixel_count * self.channel_count + 10];

                for channel in 0..self.channel_count
                {
                    let pixel_count = self.width * self.height;
                    self.psd_decode_rle(pixel_count, &mut out_channel[channel..])?;
                }

                out_channel.truncate(pixel_count * self.channel_count);

                DecodingResult::U8(out_channel)
            }
            (CompressionMethod::NoCompression, BitDepth::Eight) =>
            {
                // We're at the raw image data.  It's each channel in order (Red, Green,
                // Blue, Alpha, ...) where each channel consists of an 8-bit
                // value for each pixel in the image.

                // Read the data by channel.

                let mut out_channel = vec![0; self.width * self.height * self.channel_count + 10];
                let pixel_count = self.width * self.height;

                // check we have enough data
                if !self.stream.has(pixel_count * self.channel_count)
                {
                    return Err(PSDDecodeErrors::Generic("Incomplete bitstream"));
                }

                for channel in 0..self.channel_count
                {
                    let mut i = channel;

                    while i < pixel_count
                    {
                        out_channel[i] = self.stream.get_u8();
                        i += self.channel_count;
                    }
                }

                out_channel.truncate(pixel_count * self.channel_count);
                DecodingResult::U8(out_channel)
            }

            (CompressionMethod::NoCompression, BitDepth::Sixteen) =>
            {
                // We're at the raw image data.  It's each channel in order (Red, Green,
                // Blue, Alpha, ...) where each channel consists of an 8-bit
                // value for each pixel in the image.

                // Read the data by channel.

                let mut out_channel = vec![0; self.width * self.height * self.channel_count + 10];
                let pixel_count = self.width * self.height;

                // check we have enough data
                if !self.stream.has(pixel_count * self.channel_count)
                {
                    return Err(PSDDecodeErrors::Generic("Incomplete bitstream"));
                }

                for channel in 0..self.channel_count
                {
                    let mut i = channel;

                    while i < pixel_count
                    {
                        out_channel[i] = self.stream.get_u16_be();
                        i += self.channel_count;
                    }
                }
                out_channel.truncate(pixel_count * self.channel_count);
                DecodingResult::U16(out_channel)
            }
            _ => todo!()
        };
        // remove white matte from psd
        if self.channel_count >= 4
        {
            match result
            {
                DecodingResult::U16(ref mut pixels) =>
                {
                    for pixel in pixels.chunks_exact_mut(4)
                    {
                        if pixel[3] != 0 && pixel[3] != 65535
                        {
                            let a = f32::from(pixel[3]) / 65535.0;
                            let ra = 1.0 / a;
                            let inv_a = 65535.0 * (1.0 - ra);
                            pixel[0] = (f32::from(pixel[0]) * ra + inv_a) as u16;
                            pixel[1] = (f32::from(pixel[1]) * ra + inv_a) as u16;
                            pixel[2] = (f32::from(pixel[2]) * ra + inv_a) as u16;
                        }
                    }
                }
                DecodingResult::U8(ref mut pixels) =>
                {
                    for pixel in pixels.chunks_exact_mut(4)
                    {
                        if pixel[3] != 0 && pixel[3] != 255
                        {
                            let a = f32::from(pixel[3]) / 255.0;
                            let ra = 1.0 / a;
                            let inv_a = 255.0 * (1.0 - ra);
                            pixel[0] = (f32::from(pixel[0]) * ra + inv_a) as u8;
                            pixel[1] = (f32::from(pixel[1]) * ra + inv_a) as u8;
                            pixel[2] = (f32::from(pixel[2]) * ra + inv_a) as u8;
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn psd_decode_rle(
        &mut self, pixel_count: usize, buffer: &mut [u8]
    ) -> Result<(), PSDDecodeErrors>
    {
        let mut count = 0;
        let mut nleft = pixel_count - count;

        let mut position = 0;

        while nleft > 0
        {
            let mut len = usize::from(self.stream.get_u8());

            match len.cmp(&128)
            {
                Ordering::Less =>
                {
                    // copy next len+1 bytes literally
                    len += 1;
                    if len > nleft
                    {
                        return Err(PSDDecodeErrors::BadRLE);
                    }
                    count += len;

                    while len > 0
                    {
                        buffer[position] = self.stream.get_u8();
                        position += self.channel_count;
                        len -= 1;
                    }
                }
                Ordering::Equal => (),
                Ordering::Greater =>
                {
                    // Next -len+1 bytes in the dest are replicated from next source byte.
                    // (Interpret len as a negative 8-bit int.)
                    len = 257_usize.wrapping_sub(len) & 255;

                    if len > nleft
                    {
                        return Err(PSDDecodeErrors::BadRLE);
                    }
                    count += len;
                    let val = self.stream.get_u8();

                    while len > 0
                    {
                        buffer[position] = val;
                        position += self.channel_count;
                        len -= 1;
                    }
                }
            }

            nleft = pixel_count - count;
        }
        Ok(())
    }

    /// Get image bit depth
    pub const fn get_bit_depth(&self) -> Option<BitDepth>
    {
        if self.decoded_header
        {
            return Some(self.depth);
        }
        None
    }

    /// Get image width and height respectively
    pub fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if self.decoded_header
        {
            return Some((self.width, self.height));
        }
        None
    }
    /// Get image bit depth
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        // currently only RGB
        if self.channel_count == 4
        {
            ColorSpace::RGBA
        }
        else
        {
            ColorSpace::RGB
        }
    }
}
