use log::info;

use crate::decoder::PngChunk;
use crate::enums::{FilterMethod, InterlaceMethod, PngColor};
use crate::error::PngErrors;
use crate::PngDecoder;

impl<'a> PngDecoder<'a>
{
    pub(crate) fn parse_ihdr(&mut self, chunk: PngChunk) -> Result<(), PngErrors>
    {
        if self.seen_hdr
        {
            return Err(PngErrors::GenericStatic("Multiple IHDR, corrupt PNG"));
        }

        if chunk.length != 13
        {
            return Err(PngErrors::GenericStatic("BAD IHDR length"));
        }

        let pos_start = self.stream.get_position();

        self.png_info.width = self.stream.get_u32_be() as usize;
        self.png_info.height = self.stream.get_u32_be() as usize;

        if self.png_info.width == 0 || self.png_info.height == 0
        {
            return Err(PngErrors::GenericStatic("Width or height cannot be zero"));
        }

        if self.png_info.width > self.options.max_width
        {
            return Err(PngErrors::Generic(format!(
                "Image width {}, larger than maximum configured width {}, aborting",
                self.png_info.width, self.options.max_width
            )));
        }

        if self.png_info.height > self.options.max_height
        {
            return Err(PngErrors::Generic(format!(
                "Image height {}, larger than maximum configured height {}, aborting",
                self.png_info.height, self.options.max_height
            )));
        }

        self.png_info.depth = self.stream.get_u8();
        let color = self.stream.get_u8();

        if let Some(img_color) = PngColor::from_int(color)
        {
            self.png_info.color = img_color;
        }
        else
        {
            return Err(PngErrors::Generic(format!("Unknown color value {color}")));
        }
        self.png_info.component = self.png_info.color.num_components();
        // verify colors plus bit depths
        match self.png_info.depth
        {
            1 | 2 | 4 =>
            {
                if !matches!(self.png_info.color, PngColor::Luma | PngColor::LumaA)
                {
                    let err_msg=format!("Bit depth of {} only allows Greyscale or Indexed color types, but found {:?}",
                                        self.png_info.depth,self.png_info.color);

                    return Err(PngErrors::Generic(err_msg));
                }
            }
            8 =>
            { /*silent pass through since all color types support it */ }
            16 =>
            {
                if self.png_info.color == PngColor::Palette
                {
                    return Err(PngErrors::GenericStatic(
                        "Indexed colour cannot have 16 bit depth"
                    ));
                }
            }
            _ =>
            {
                return Err(PngErrors::Generic(format!(
                    "Unknown bit depth {}",
                    self.png_info.depth
                )))
            }
        }

        if self.stream.get_u8() != 0
        {
            return Err(PngErrors::GenericStatic("Unknown compression method"));
        }

        let filter_method = self.stream.get_u8();

        if let Some(method) = FilterMethod::from_int(filter_method)
        {
            self.png_info.filter_method = method;
        }
        else
        {
            return Err(PngErrors::Generic(format!(
                "Unknown filter method {filter_method}"
            )));
        }

        let interlace_method = self.stream.get_u8();

        if let Some(method) = InterlaceMethod::from_int(interlace_method)
        {
            self.png_info.interlace_method = method;
        }
        else
        {
            return Err(PngErrors::Generic(format!(
                "Unknown interlace method {interlace_method}",
            )));
        }

        let pos_end = self.stream.get_position();

        assert_eq!(pos_end - pos_start, 13); //we read all bytes

        // skip crc
        self.stream.skip(4);

        info!("Width: {}", self.png_info.width);
        info!("Height: {}", self.png_info.height);
        info!("Color type: {:?}", self.png_info.color);
        info!("Filter type:{:?}", self.png_info.filter_method);
        info!("Depth: {:?}", self.png_info.depth);
        info!("Interlace :{:?}", self.png_info.interlace_method);

        self.seen_hdr = true;

        Ok(())
    }

    pub(crate) fn parse_plt(&mut self, chunk: PngChunk) -> Result<(), PngErrors>
    {
        if chunk.length % 3 != 0
        {
            return Err(PngErrors::GenericStatic("Invalid pLTE length, corrupt PNG"));
        }

        // allocate palette
        self.palette.resize(256 * 3, 0);

        for pal_chunk in self.palette.chunks_exact_mut(3).take(chunk.length / 3)
        {
            pal_chunk[0] = self.stream.get_u8();
            pal_chunk[1] = self.stream.get_u8();
            pal_chunk[2] = self.stream.get_u8();
        }

        // skip crc chunk
        self.stream.skip(4);
        self.un_palettize = true;
        Ok(())
    }

    pub(crate) fn parse_idat(&mut self, png_chunk: PngChunk) -> Result<(), PngErrors>
    {
        // get a reference to the IDAT chunk stream and push it,
        // we will later pass these to the deflate decoder as a whole, to get the whole
        // uncompressed stream.

        let idat_stream = self.stream.get_as_ref(png_chunk.length)?;

        self.idat_chunks.extend_from_slice(idat_stream);

        // skip crc
        self.stream.skip(4);

        Ok(())
    }

    pub(crate) fn parse_trns(&mut self, chunk: PngChunk) -> Result<(), PngErrors>
    {
        match self.png_info.color
        {
            PngColor::Luma =>
            {
                let _grey_sample = self.stream.get_u16_be();
            }
            PngColor::RGB =>
            {
                let _red_sample = self.stream.get_u16_be();
                let _blue_sample = self.stream.get_u16_be();
                let _green_sample = self.stream.get_u16_be();
            }
            PngColor::Palette =>
            {
                if self.palette.is_empty()
                {
                    return Err(PngErrors::GenericStatic("tRNS chunk before plTE"));
                }
                if self.palette.len() <= chunk.length * 4
                {
                    return Err(PngErrors::GenericStatic("tRNS chunk with too long entries"));
                }
                for i in 0..chunk.length
                {
                    self.palette[i * 4 + 3] = self.stream.get_u8();
                }
            }
            _ =>
            {
                let msg = format!("A tRNS chunk shall not appear for colour type {:?} as it is already transparent", self.png_info.color);

                return Err(PngErrors::Generic(msg));
            }
        }
        // skip crc
        self.stream.skip(4);

        Ok(())
    }
    pub(crate) fn parse_gama(&mut self, chunk: PngChunk) -> Result<(), PngErrors>
    {
        if self.options.strict_mode && chunk.length != 4
        {
            let error = format!("Gama chunk length is not 4 but {}", chunk.length);
            return Err(PngErrors::Generic(error));
        }

        self.gama = self.stream.get_u32_be();

        // skip crc
        self.stream.skip(4);

        Ok(())
    }
}
