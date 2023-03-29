use alloc::format;

use log::{error, info, warn};
use zune_inflate::DeflateDecoder;

use crate::decoder::{ItxtChunk, PLTEEntry, PngChunk, TextChunk, TimeInfo, ZtxtChunk};
use crate::enums::{FilterMethod, InterlaceMethod, PngColor};
use crate::error::PngDecodeErrors;
use crate::PngDecoder;

impl<'a> PngDecoder<'a>
{
    pub(crate) fn parse_ihdr(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if self.seen_hdr
        {
            return Err(PngDecodeErrors::GenericStatic("Multiple IHDR, corrupt PNG"));
        }

        if chunk.length != 13
        {
            return Err(PngDecodeErrors::GenericStatic("BAD IHDR length"));
        }

        let pos_start = self.stream.get_position();

        self.png_info.width = self.stream.get_u32_be() as usize;
        self.png_info.height = self.stream.get_u32_be() as usize;

        if self.png_info.width == 0 || self.png_info.height == 0
        {
            return Err(PngDecodeErrors::GenericStatic(
                "Width or height cannot be zero"
            ));
        }

        if self.png_info.width > self.options.get_max_width()
        {
            return Err(PngDecodeErrors::Generic(format!(
                "Image width {}, larger than maximum configured width {}, aborting",
                self.png_info.width,
                self.options.get_max_width()
            )));
        }

        if self.png_info.height > self.options.get_max_height()
        {
            return Err(PngDecodeErrors::Generic(format!(
                "Image height {}, larger than maximum configured height {}, aborting",
                self.png_info.height,
                self.options.get_max_height()
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
            return Err(PngDecodeErrors::Generic(format!(
                "Unknown color value {color}"
            )));
        }
        self.png_info.component = self.png_info.color.num_components();
        // verify colors plus bit depths
        match self.png_info.depth
        {
            1 | 2 | 4 | 8 =>
            { /*silent pass through since all color types support it */ }
            16 =>
            {
                if self.png_info.color == PngColor::Palette
                {
                    return Err(PngDecodeErrors::GenericStatic(
                        "Indexed colour cannot have 16 bit depth"
                    ));
                }
            }
            _ =>
            {
                return Err(PngDecodeErrors::Generic(format!(
                    "Unknown bit depth {}",
                    self.png_info.depth
                )))
            }
        }

        if self.stream.get_u8() != 0
        {
            return Err(PngDecodeErrors::GenericStatic("Unknown compression method"));
        }

        let filter_method = self.stream.get_u8();

        if let Some(method) = FilterMethod::from_int(filter_method)
        {
            self.png_info.filter_method = method;
        }
        else
        {
            return Err(PngDecodeErrors::Generic(format!(
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
            return Err(PngDecodeErrors::Generic(format!(
                "Unknown interlace method {interlace_method}",
            )));
        }

        let pos_end = self.stream.get_position();

        assert_eq!(pos_end - pos_start, 13); //we read all bytes

        // skip crc
        self.stream.skip(4);

        info!("Width: {}", self.png_info.width);
        info!("Height: {}", self.png_info.height);
        info!("Filter type:{:?}", self.png_info.filter_method);
        info!("Depth: {:?}", self.png_info.depth);
        info!("Interlace :{:?}", self.png_info.interlace_method);

        self.seen_hdr = true;

        Ok(())
    }

    pub(crate) fn parse_plte(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if chunk.length % 3 != 0
        {
            return Err(PngDecodeErrors::GenericStatic(
                "Invalid pLTE length, corrupt PNG"
            ));
        }

        // allocate palette
        self.palette.resize(256, PLTEEntry::default());

        for pal_chunk in self.palette.iter_mut().take(chunk.length / 3)
        {
            pal_chunk.red = self.stream.get_u8();
            pal_chunk.green = self.stream.get_u8();
            pal_chunk.blue = self.stream.get_u8();
        }

        // skip crc chunk
        self.stream.skip(4);
        self.seen_ptle = true;
        Ok(())
    }

    pub(crate) fn parse_idat(&mut self, png_chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        // get a reference to the IDAT chunk stream and push it,
        // we will later pass these to the deflate decoder as a whole, to get the whole
        // uncompressed stream.

        let idat_stream = self.stream.get(png_chunk.length)?;

        self.idat_chunks.extend_from_slice(idat_stream);

        // skip crc
        self.stream.skip(4);

        Ok(())
    }

    pub(crate) fn parse_trns(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        match self.png_info.color
        {
            PngColor::Luma =>
            {
                let grey_sample = self.stream.get_u16_be();
                self.trns_bytes[0] = grey_sample;
            }
            PngColor::RGB =>
            {
                self.trns_bytes[0] = self.stream.get_u16_be();
                self.trns_bytes[1] = self.stream.get_u16_be();
                self.trns_bytes[2] = self.stream.get_u16_be();
            }
            PngColor::Palette =>
            {
                if self.palette.is_empty()
                {
                    return Err(PngDecodeErrors::GenericStatic("tRNS chunk before plTE"));
                }
                if self.palette.len() < chunk.length
                {
                    return Err(PngDecodeErrors::Generic(format!(
                        "tRNS chunk with too long entries {}",
                        chunk.length
                    )));
                }
                for i in 0..chunk.length
                {
                    self.palette[i].alpha = self.stream.get_u8();
                }
            }
            _ =>
            {
                let msg = format!("A tRNS chunk shall not appear for colour type {:?} as it is already transparent", self.png_info.color);

                return Err(PngDecodeErrors::Generic(msg));
            }
        }
        // skip crc
        self.stream.skip(4);
        self.seen_trns = true;

        Ok(())
    }
    pub(crate) fn parse_gama(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if self.options.get_strict_mode() && chunk.length != 4
        {
            let error = format!("Gama chunk length is not 4 but {}", chunk.length);
            return Err(PngDecodeErrors::Generic(error));
        }

        let mut gama = (self.stream.get_u32_be() as f64 / 100000.0) as f32;
        if gama == 0.0
        {
            // this is invalid gama
            // warn and set it to 2.2 which is the default gama
            warn!("Gamma value of 0.0 is invalid, setting it to 2.2");
            gama = 1.0 / 2.2;
        }
        self.png_info.gamma = Some(gama);
        // skip crc
        self.stream.skip(4);

        Ok(())
    }

    /// Parse the animation control chunk
    pub(crate) fn parse_actl(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if self.options.get_strict_mode()
        {
            return Err(PngDecodeErrors::UnsupportedAPNGImage);
        }
        else
        {
            error!("APNG support is not yet present,this will only decode the first frame of the image");
        }
        // skip bytes plus CRC
        self.stream.skip(chunk.length + 4);

        Ok(())
    }

    /// Parse the tIME chunk if present in PNG
    pub(crate) fn parse_time(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if chunk.length != 7
        {
            if self.options.get_strict_mode()
            {
                return Err(PngDecodeErrors::GenericStatic("Invalid tIME chunk length"));
            }
            warn!("Invalid time chunk length {:?}", chunk.length);
            // skip chunk + crc
            self.stream.skip(chunk.length + 4);
            return Ok(());
        }

        let year = self.stream.get_u16_be();
        let month = self.stream.get_u8() % 13;
        let day = self.stream.get_u8() % 32;
        let hour = self.stream.get_u8() % 24;
        let minute = self.stream.get_u8() % 60;
        let second = self.stream.get_u8() % 61;

        let time = TimeInfo {
            year,
            month,
            day,
            hour,
            minute,
            second
        };
        self.png_info.time_info = Some(time);
        // skip past crc
        self.stream.skip(4);

        Ok(())
    }

    pub(crate) fn parse_exif(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors>
    {
        if !self.stream.has(chunk.length)
        {
            warn!("Too large exif chunk");
            self.stream.skip(chunk.length + 4);

            return Ok(());
        }
        let data = self.stream.peek_at(0, chunk.length).unwrap();

        // recommended that we check for first four bytes compatibility
        // so do it here
        // First check does litle endian, and second big endian
        // See https://ftp-osl.osuosl.org/pub/libpng/documents/pngext-1.5.0.html#C.eXIf
        if !(data.starts_with(&[73, 73, 42, 0]) || data.starts_with(&[77, 77, 0, 42]))
        {
            if self.options.get_strict_mode()
            {
                return Err(PngDecodeErrors::GenericStatic(
                    "[strict-mode]: Invalid exif chunk"
                ));
            }
            else
            {
                warn!("Invalid exif chunk, it doesn't start with the magic bytes")
            }
            // do not parse
            self.stream.skip(chunk.length + 4);
            return Ok(());
        }
        self.png_info.exif = Some(data);
        // skip past crc
        self.stream.skip(chunk.length + 4);

        Ok(())
    }

    /// Parse the iCCP chunk
    pub(crate) fn parse_iccp(&mut self, chunk: PngChunk)
    {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length).unwrap();
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position
        {
            // skip name plus null byte
            self.stream.skip(pos + 1);

            let remainder = chunk
                .length
                .saturating_sub(pos)
                .saturating_sub(1) // null separator
                .saturating_sub(1); // compression method

            // read compression method
            let _ = self.stream.get_u8();

            // read remaining chunk
            let data = self.stream.peek_at(0, remainder).unwrap();

            // decode to vec
            if let Ok(icc_uncompressed) = DeflateDecoder::new(data).decode_zlib()
            {
                self.png_info.icc_profile = Some(icc_uncompressed);
            }
            else
            {
                warn!("Could not decode ICC profile, error with zlib stream");
            }
            self.stream.skip(remainder);
        }
        else
        {
            warn!("Could not find keyword in iCCP chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length);
        }
        // skip crc
        self.stream.skip(4);
    }

    /// Parse the text chunk
    pub(crate) fn parse_text(&mut self, chunk: PngChunk)
    {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length).unwrap();
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position
        {
            let keyword = &keyword_bytes[..pos];
            // skip name plus null byte
            self.stream.skip(pos + 1);

            let remainder = chunk.length.saturating_sub(pos).saturating_sub(1); // null byte

            // read remaining chunk

            let text = self.stream.peek_at(0, remainder).unwrap();

            let text_chunk = TextChunk { keyword, text };
            self.png_info.text_chunk.push(text_chunk);

            self.stream.skip(remainder);
        }
        else
        {
            warn!("Could not find keyword in text chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length);
        }
        // skip crc
        self.stream.skip(4);
    }
    /// Parse the itXT chunk
    pub(crate) fn parse_itxt(&mut self, chunk: PngChunk)
    {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length).unwrap();
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position
        {
            let keyword = &keyword_bytes[..pos];
            // skip name plus null byte
            let bytes_to_skip = pos + 1 // null separator
                + 1  // compression flag
                + 1  // compression method
                + 1  // null separator
                + 1; // null separator

            self.stream.skip(bytes_to_skip);
            let remainder = chunk.length.saturating_sub(bytes_to_skip);
            let raw_data = self.stream.peek_at(0, remainder).unwrap();

            let itxt_chunk = ItxtChunk {
                keyword,
                text: raw_data
            };
            self.png_info.itxt_chunk.push(itxt_chunk);
            // skip bytes we read
            self.stream.skip(remainder);
        }
        else
        {
            warn!("Possibly corrupt iTXT chunk");
            self.stream.skip(chunk.length);
        }
        // skip crc
        self.stream.skip(4);
    }

    /// Parse zTxt chunk
    pub(crate) fn parse_ztxt(&mut self, chunk: PngChunk)
    {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length).unwrap();
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position
        {
            let keyword = &keyword_bytes[..pos];

            // skip name plus null byte
            self.stream.skip(pos + 1);

            let remainder = chunk
                .length
                .saturating_sub(pos)
                .saturating_sub(1) // null separator
                .saturating_sub(1); // compression method

            // read compression method
            let _ = self.stream.get_u8();

            // read remaining chunk
            let data = self.stream.peek_at(0, remainder).unwrap();

            // decode to vec
            if let Ok(ztxt) = DeflateDecoder::new(data).decode_zlib()
            {
                let chunk = ZtxtChunk {
                    keyword,
                    text: ztxt
                };
                self.png_info.ztxt_chunk.push(chunk);
            }
            else
            {
                warn!("Could not decode ztxt profile, error with zlib stream");
            }
            self.stream.skip(remainder);
        }
        else
        {
            warn!("Could not find keyword in iCCP chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length);
        }
        // skip crc
        self.stream.skip(4);
    }
}
