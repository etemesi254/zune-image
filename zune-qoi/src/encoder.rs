/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::vec;
use alloc::vec::Vec;

use zune_core::bytestream::ZByteWriter;
use zune_core::colorspace::{ColorCharacteristics, ColorSpace};
use zune_core::options::EncoderOptions;

use crate::constants::{
    QOI_HEADER_SIZE, QOI_MAGIC, QOI_OP_DIFF, QOI_OP_INDEX, QOI_OP_LUMA, QOI_OP_RGB, QOI_OP_RGBA,
    QOI_OP_RUN, QOI_PADDING
};
use crate::QoiEncodeErrors;

const SUPPORTED_COLORSPACES: [ColorSpace; 2] = [ColorSpace::RGB, ColorSpace::RGBA];

pub struct QoiEncoder<'a>
{
    // raw pixels, in RGB or RBGA
    pixel_data:            &'a [u8],
    options:               EncoderOptions,
    color_characteristics: ColorCharacteristics
}

impl<'a> QoiEncoder<'a>
{
    /// Create a new encoder which will encode the pixels
    #[allow(clippy::redundant_field_names)]
    pub const fn new(data: &'a [u8], options: EncoderOptions) -> QoiEncoder<'a>
    {
        QoiEncoder {
            pixel_data:            data,
            options:               options,
            color_characteristics: ColorCharacteristics::sRGB
        }
    }
    pub fn set_color_characteristics(&mut self, characteristics: ColorCharacteristics)
    {
        self.color_characteristics = characteristics;
    }

    /// Return the maximum size for which the encoder can safely
    /// encode the image without fearing for an out of space error
    fn max_size(&self) -> usize
    {
        self.options.get_width()
            * self.options.get_height()
            * (self.options.get_colorspace().num_components() + 1)
            + QOI_HEADER_SIZE
            + QOI_PADDING
    }
    fn encode_headers(&self, writer: &mut ZByteWriter) -> Result<(), QoiEncodeErrors>
    {
        if writer.has(QOI_HEADER_SIZE)
        {
            // qoif
            writer.write_all(&QOI_MAGIC.to_be_bytes()).unwrap();

            let options = &self.options;
            if (options.get_width() as u64) > u64::from(u32::MAX)
            {
                // error out
                return Err(QoiEncodeErrors::TooLargeDimensions(options.get_width()));
            }
            if (options.get_height() as u64) > u64::from(u32::MAX)
            {
                return Err(QoiEncodeErrors::TooLargeDimensions(options.get_height()));
            }
            // it's safe to convert to u32 here. since we checked
            // the number can be safely encoded.

            // width
            writer.write_u32_be(options.get_width() as u32);
            // height
            writer.write_u32_be(options.get_height() as u32);
            //channel
            let channel = match self.options.get_colorspace()
            {
                ColorSpace::RGB => 3,
                ColorSpace::RGBA => 4,

                _ =>
                {
                    return Err(QoiEncodeErrors::UnsupportedColorspace(
                        self.options.get_colorspace(),
                        &SUPPORTED_COLORSPACES
                    ))
                }
            };

            writer.write_u8(channel);
            // colorspace
            let xtic = u8::from(self.color_characteristics == ColorCharacteristics::Linear);
            writer.write_u8(xtic);
        }
        else
        {
            unreachable!("Cannot allocate internal space for headers");
        }
        Ok(())
    }
    #[allow(clippy::manual_range_contains)]
    pub fn encode(&mut self) -> Result<Vec<u8>, QoiEncodeErrors>
    {
        // set encoded data to be an array of zeroes
        let mut encoded_data = vec![0; self.max_size()];

        let mut stream = ZByteWriter::new(&mut encoded_data);

        self.encode_headers(&mut stream)?;

        let mut index = [[0_u8; 4]; 64];
        // starting pixel
        let mut px = [0, 0, 0, 255];
        let mut px_prev = [0, 0, 0, 255];

        let mut run = 0;

        let channel_count = self.options.get_colorspace().num_components();

        for pix_chunk in self.pixel_data.chunks_exact(channel_count)
        {
            px[0..channel_count].copy_from_slice(pix_chunk);

            if px == px_prev
            {
                run += 1;

                if run == 62
                {
                    stream.write_u8(QOI_OP_RUN | (run - 1));
                    run = 0;
                }
            }
            else
            {
                if run > 0
                {
                    stream.write_u8(QOI_OP_RUN | (run - 1));
                    run = 0;
                }

                let index_pos = (usize::from(px[0]) * 3
                    + usize::from(px[1]) * 5
                    + usize::from(px[2]) * 7
                    + usize::from(px[3]) * 11)
                    % 64;

                if index[index_pos] == px
                {
                    stream.write_u8(QOI_OP_INDEX | (index_pos as u8));
                }
                else
                {
                    index[index_pos] = px;

                    if px[3] == px_prev[3]
                    {
                        let vr = px[0].wrapping_sub(px_prev[0]);
                        let vg = px[1].wrapping_sub(px_prev[1]);
                        let vb = px[2].wrapping_sub(px_prev[2]);

                        let vg_r = vr.wrapping_sub(vg);
                        let vg_b = vb.wrapping_sub(vg);

                        if (vr < 2 || vr > 253) && (vg < 2 || vg > 253) && (vb < 2 || vb > 253)
                        {
                            stream.write_u8(
                                QOI_OP_DIFF
                                    | vr.wrapping_add(2) << 4
                                    | vg.wrapping_add(2) << 2
                                    | vb.wrapping_add(2)
                            );
                        }
                        else if (vg_r > 247 || vg_r < 8)
                            && (vg > 223 || vg < 32)
                            && (vg_b > 247 || vg_b < 8)
                        {
                            stream.write_u8(QOI_OP_LUMA | vg.wrapping_add(32));
                            stream.write_u8(vg_r.wrapping_add(8) << 4 | vg_b.wrapping_add(8));
                        }
                        else
                        {
                            stream.write_u8(QOI_OP_RGB);
                            stream.write_u8(px[0]);
                            stream.write_u8(px[1]);
                            stream.write_u8(px[2]);
                        }
                    }
                    else
                    {
                        stream.write_u8(QOI_OP_RGBA);

                        stream.write_u32_be(u32::from_be_bytes(px));
                    }
                }
            }

            px_prev.copy_from_slice(&px);
        }
        if run > 0
        {
            stream.write_u8(QOI_OP_RUN | (run - 1));
        }
        // write trailing bytes
        stream.write_u64_be(0x01);
        // done
        let len = stream.position();
        // reduce the length to be the expected value
        encoded_data.truncate(len);

        Ok(encoded_data)
    }
}
