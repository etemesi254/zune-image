/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use alloc::format;

use zune_core::bytestream::ZByteReaderTrait;
use zune_core::log::{trace, warn};
use zune_inflate::DeflateDecoder;

use crate::apng::{ActlChunk, BlendOp, DisposeOp, FrameInfo, SingleFrame};
use crate::decoder::{ChrmInfo, CicpInfo, ItxtChunk, PLTEEntry, PngChunk, TextChunk, TimeInfo, ZtxtChunk};
use crate::enums::{FilterMethod, InterlaceMethod, PngColor};
use crate::error::PngDecodeErrors;
use crate::{ClliInfo, PhysInfo, PngDecoder};

impl<T: ZByteReaderTrait> PngDecoder<T> {
    pub(crate) fn parse_ihdr(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if self.seen_hdr {
            return Err(PngDecodeErrors::GenericStatic("Multiple IHDR, corrupt PNG"));
        }

        if chunk.length != 13 {
            return Err(PngDecodeErrors::GenericStatic("BAD IHDR length"));
        }

        self.png_info.width = self.stream.get_u32_be() as usize;
        self.png_info.height = self.stream.get_u32_be() as usize;

        if self.png_info.width == 0 || self.png_info.height == 0 {
            return Err(PngDecodeErrors::GenericStatic(
                "Width or height cannot be zero",
            ));
        }

        if self.png_info.width > self.options.max_width() {
            return Err(PngDecodeErrors::Generic(format!(
                "Image width {}, larger than maximum configured width {}, aborting",
                self.png_info.width,
                self.options.max_width()
            )));
        }

        if self.png_info.height > self.options.max_height() {
            return Err(PngDecodeErrors::Generic(format!(
                "Image height {}, larger than maximum configured height {}, aborting",
                self.png_info.height,
                self.options.max_height()
            )));
        }

        self.png_info.depth = self.stream.read_u8();
        let color = self.stream.read_u8();

        if let Some(img_color) = PngColor::from_int(color) {
            self.png_info.color = img_color;
        } else {
            return Err(PngDecodeErrors::Generic(format!(
                "Unknown color value {color}"
            )));
        }
        self.png_info.component = self.png_info.color.num_components();
        // verify colors plus bit depths
        match self.png_info.depth {
            1 | 2 | 4 | 8 => { /*silent pass through since all color types support it */ }
            16 => {
                if self.png_info.color == PngColor::Palette {
                    return Err(PngDecodeErrors::GenericStatic(
                        "Indexed colour cannot have 16 bit depth",
                    ));
                }
            }
            _ => {
                return Err(PngDecodeErrors::Generic(format!(
                    "Unknown bit depth {}",
                    self.png_info.depth
                )));
            }
        }

        if self.stream.read_u8() != 0 {
            return Err(PngDecodeErrors::GenericStatic("Unknown compression method"));
        }

        let filter_method = self.stream.read_u8();

        if let Some(method) = FilterMethod::from_int(filter_method) {
            self.png_info.filter_method = method;
        } else {
            return Err(PngDecodeErrors::Generic(format!(
                "Unknown filter method {filter_method}"
            )));
        }

        let interlace_method = self.stream.read_u8();

        if let Some(method) = InterlaceMethod::from_int(interlace_method) {
            self.png_info.interlace_method = method;
        } else {
            return Err(PngDecodeErrors::Generic(format!(
                "Unknown interlace method {interlace_method}",
            )));
        }

        // skip crc
        self.stream.skip(4)?;

        trace!("Width: {}", self.png_info.width);
        trace!("Height: {}", self.png_info.height);
        trace!("Filter type:{:?}", self.png_info.filter_method);
        trace!("Depth: {:?}", self.png_info.depth);
        trace!("Interlace :{:?}", self.png_info.interlace_method);

        self.seen_hdr = true;

        let frame_info = FrameInfo {
            seq_number: -1,
            width: self.png_info.width,
            height: self.png_info.height,
            x_offset: 0,
            y_offset: 0,
            delay_num: 0,
            delay_denom: 0,
            dispose_op: DisposeOp::None,
            blend_op: BlendOp::Source,
            is_part_of_seq: false,
        };

        self.frames.push(SingleFrame::new(frame_info));

        Ok(())
    }

    pub(crate) fn parse_plte(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if !chunk.length.is_multiple_of(3) {
            return Err(PngDecodeErrors::GenericStatic(
                "Invalid pLTE length, corrupt PNG",
            ));
        }

        // allocate palette
        self.palette.resize(256, PLTEEntry::default());

        for pal_chunk in self.palette.iter_mut().take(chunk.length / 3) {
            pal_chunk.0[0] = self.stream.read_u8();
            pal_chunk.0[1] = self.stream.read_u8();
            pal_chunk.0[2] = self.stream.read_u8();
        }

        // skip crc chunk
        self.stream.skip(4)?;
        self.seen_ptle = true;
        Ok(())
    }

    pub(crate) fn parse_trns(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        match self.png_info.color {
            PngColor::Luma => {
                let grey_sample = self.stream.get_u16_be();
                self.trns_bytes[0] = grey_sample;
            }
            PngColor::RGB => {
                self.trns_bytes[0] = self.stream.get_u16_be();
                self.trns_bytes[1] = self.stream.get_u16_be();
                self.trns_bytes[2] = self.stream.get_u16_be();
            }
            PngColor::Palette => {
                if self.palette.is_empty() {
                    return Err(PngDecodeErrors::GenericStatic("tRNS chunk before plTE"));
                }
                if self.palette.len() < chunk.length {
                    return Err(PngDecodeErrors::Generic(format!(
                        "tRNS chunk with too long entries {}",
                        chunk.length
                    )));
                }
                for i in 0..chunk.length {
                    self.palette[i].0[3] = self.stream.read_u8();
                }
            }
            _ => {
                let msg = format!("A tRNS chunk shall not appear for colour type {:?} as it is already transparent", self.png_info.color);

                return Err(PngDecodeErrors::Generic(msg));
            }
        }
        // skip crc
        self.stream.skip(4)?;
        self.seen_trns = true;

        Ok(())
    }
    pub(crate) fn parse_gama(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if self.options.strict_mode() && chunk.length != 4 {
            let error = format!("Gama chunk length is not 4 but {}", chunk.length);
            return Err(PngDecodeErrors::Generic(error));
        }

        let mut gama = (self.stream.get_u32_be() as f64 / 100000.0) as f32;
        if gama == 0.0 {
            // this is invalid gama
            // warn and set it to 2.2 which is the default gama
            warn!("Gamma value of 0.0 is invalid, setting it to 2.2");
            gama = 1.0 / 2.2;
        }
        self.png_info.gamma = Some(gama);
        // skip crc
        self.stream.skip(4)?;

        Ok(())
    }

    /// Parse the animation control chunk
    pub(crate) fn parse_actl(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 8 {
            warn!("Invalid chunk length for ACTL, skipping");
            self.stream.skip(chunk.length + 4)?;
            return Ok(());
        }
        // extract num_frames
        let num_frames = self.stream.get_u32_be();
        let num_plays = self.stream.get_u32_be();

        let actl = ActlChunk {
            num_frames,
            num_plays,
        };
        self.actl_info = Some(actl);

        // skip CRC
        self.stream.skip(4)?;

        Ok(())
    }

    /// Parse the tIME chunk if present in PNG
    pub(crate) fn parse_time(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 7 {
            if self.options.strict_mode() {
                return Err(PngDecodeErrors::GenericStatic("Invalid tIME chunk length"));
            }
            warn!("Invalid time chunk length {:?}", chunk.length);
            // skip chunk + crc
            self.stream.skip(chunk.length + 4)?;
            return Ok(());
        }

        let year = self.stream.get_u16_be();
        let month = self.stream.read_u8() % 13;
        let day = self.stream.read_u8() % 32;
        let hour = self.stream.read_u8() % 24;
        let minute = self.stream.read_u8() % 60;
        let second = self.stream.read_u8() % 61;

        let time = TimeInfo {
            year,
            month,
            day,
            hour,
            minute,
            second,
        };
        self.png_info.time_info = Some(time);
        // skip past crc
        self.stream.skip(4)?;

        Ok(())
    }

    pub(crate) fn parse_exif(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        let data = self.stream.peek_at(0, chunk.length)?;

        // recommended that we check for first four bytes compatibility
        // so do it here
        // First check does litle endian, and second big endian
        // See https://ftp-osl.osuosl.org/pub/libpng/documents/pngext-1.5.0.html#C.eXIf
        if !(data.starts_with(&[73, 73, 42, 0]) || data.starts_with(&[77, 77, 0, 42])) {
            if self.options.strict_mode() {
                return Err(PngDecodeErrors::GenericStatic(
                    "[strict-mode]: Invalid exif chunk",
                ));
            } else {
                warn!("Invalid exif chunk, it doesn't start with the magic bytes");
            }
            // do not parse
            self.stream.skip(chunk.length + 4)?;
            return Ok(());
        }
        self.png_info.exif = Some(data.to_vec());
        // skip past crc
        self.stream.skip(chunk.length + 4)?;

        Ok(())
    }

    /// Parse the iCCP chunk
    pub(crate) fn parse_iccp(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length)?;
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position {
            // skip name plus null byte
            self.stream.skip(pos + 1)?;

            let remainder = chunk
                .length
                .saturating_sub(pos)
                .saturating_sub(1) // null separator
                .saturating_sub(1); // compression method

            // read compression method
            let _ = self.stream.read_u8();

            // read remaining chunk
            let data = self.stream.peek_at(0, remainder)?;

            // decode to vec
            if let Ok(icc_uncompressed) = DeflateDecoder::new(data).decode_zlib() {
                self.png_info.icc_profile = Some(icc_uncompressed);
            } else {
                warn!("Could not decode ICC profile, error with zlib stream");
            }
            self.stream.skip(remainder)?;
        } else {
            warn!("Could not find keyword in iCCP chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length)?;
        }
        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the text chunk
    pub(crate) fn parse_text(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length)?;
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position {
            let keyword = keyword_bytes[..pos].to_vec();
            // skip name plus null byte
            self.stream.skip(pos + 1)?;

            let remainder = chunk.length.saturating_sub(pos).saturating_sub(1); // null byte

            // read remaining chunk

            let text = self.stream.peek_at(0, remainder)?.to_vec();

            let text_chunk = TextChunk { keyword, text };
            self.png_info.text_chunk.push(text_chunk);

            self.stream.skip(remainder)?;
        } else {
            warn!("Could not find keyword in text chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length)?;
        }
        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }
    /// Parse the itXT chunk
    pub(crate) fn parse_itxt(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length)?;
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position {
            let keyword = keyword_bytes[..pos].to_vec();
            // skip name plus null byte
            let bytes_to_skip = pos + 1 // null separator
                + 1  // compression flag
                + 1  // compression method
                + 1  // null separator
                + 1; // null separator

            self.stream.skip(bytes_to_skip)?;
            let remainder = chunk.length.saturating_sub(bytes_to_skip);
            let raw_data = self.stream.peek_at(0, remainder)?.to_vec();

            let itxt_chunk = ItxtChunk {
                keyword,
                text: raw_data,
            };
            self.png_info.itxt_chunk.push(itxt_chunk);
            // skip bytes we read
            self.stream.skip(remainder)?;
        } else {
            warn!("Possibly corrupt iTXT chunk");
            self.stream.skip(chunk.length)?;
        }
        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse zTxt chunk
    pub(crate) fn parse_ztxt(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        let length = core::cmp::min(chunk.length, 79);
        let keyword_bytes = self.stream.peek_at(0, length)?;
        let keyword_position = keyword_bytes.iter().position(|x| *x == 0);

        if let Some(pos) = keyword_position {
            let keyword = keyword_bytes[..pos].to_vec();

            // skip name plus null byte
            self.stream.skip(pos + 1)?;

            let remainder = chunk
                .length
                .saturating_sub(pos)
                .saturating_sub(1) // null separator
                .saturating_sub(1); // compression method

            // read compression method
            let _ = self.stream.read_u8();

            // read remaining chunk
            let data = self.stream.peek_at(0, remainder)?;

            // decode to vec
            if let Ok(ztxt) = DeflateDecoder::new(data).decode_zlib() {
                let chunk = ZtxtChunk {
                    keyword,
                    text: ztxt,
                };
                self.png_info.ztxt_chunk.push(chunk);
            } else {
                warn!("Could not decode ztxt profile, error with zlib stream");
            }
            self.stream.skip(remainder)?;
        } else {
            warn!("Could not find keyword in iCCP chunk, possibly corrupt chunk");
            // skip the length
            self.stream.skip(chunk.length)?;
        }
        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the FCTL chunk
    pub(crate) fn parse_fctl(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        // From https://wiki.mozilla.org/APNG_Specification
        //
        // The `fcTL` chunk is an ancillary chunk as defined in the PNG Specification.
        // It must appear before the `IDAT` or `fdAT` chunks of the frame to which it applies,
        //
        let fctl_info = self.parse_fctl_external(chunk)?;
        // confirm dimensions
        if fctl_info.width > self.png_info.width {
            return Err(PngDecodeErrors::Generic(format!(
                "Frame FCTL width ({}) is larger than image width ({})",
                fctl_info.width, self.png_info.width
            )));
        }
        if fctl_info.height > self.png_info.height {
            return Err(PngDecodeErrors::Generic(format!(
                "Frame FCTL height ({}) is larger than image height ({})",
                fctl_info.height, self.png_info.height
            )));
        }

        self.num_fctl_seen += 1;
        self.frames.push(SingleFrame::new(fctl_info));
        // let the current frame be the last frame there
        self.current_frame = self.frames.len() - 1;

        Ok(())
    }

    pub(crate) fn parse_fctl_external(
        &mut self, chunk: PngChunk,
    ) -> Result<FrameInfo, PngDecodeErrors> {
        if chunk.length != 26 {
            return Err(PngDecodeErrors::GenericStatic("Invalid fcTL length"));
        }
        let seq_number = self.stream.get_u32_be() as i32;
        let width = self.stream.get_u32_be() as usize;
        let height = self.stream.get_u32_be() as usize;
        let x_offset = self.stream.get_u32_be() as usize;
        let y_offset = self.stream.get_u32_be() as usize;
        let delay_num = self.stream.get_u16_be();
        let delay_denom = self.stream.get_u16_be();
        let dispose_op = DisposeOp::from_int(self.stream.read_u8())?;
        let blend_op = BlendOp::from_int(self.stream.read_u8())?;

        let fctl_info = FrameInfo {
            seq_number,
            width,
            height,
            x_offset,
            y_offset,
            delay_num,
            delay_denom,
            dispose_op,
            blend_op,
            is_part_of_seq: true,
        };
        // skip crc
        self.stream.skip(4)?;
        Ok(fctl_info)
    }
    /// Parse the pHYs chunk (Physical Pixel Dimensions)
    pub(crate) fn parse_phys(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 9  {
            return Err(PngDecodeErrors::Generic(format!("Invalid pHYs chunk length {:?}",chunk.length)));
        }

        let ppu_x = self.stream.get_u32_be();
        let ppu_y = self.stream.get_u32_be();
        let unit_specifier = self.stream.read_u8();

        let phys_info = PhysInfo {
            ppu_x,
            ppu_y,
            unit_specifier, // 0 = unknown, 1 = meter
        };
        self.png_info.phys_info = Some(phys_info);

        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the cLLI chunk (Content Light Level Information)
    pub(crate) fn parse_clli(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 8 {
            return Err(PngDecodeErrors::GenericStatic("Invalid cLLI chunk length"));
        }

        let max_cll = self.stream.get_u32_be();
        let max_fall = self.stream.get_u32_be();

        let clli_info = ClliInfo { max_cll, max_fall };
        self.png_info.clli_info = Some(clli_info);

        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the cICP chunk (Coding-Independent Code Points)
    pub(crate) fn parse_cicp(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 4 {
            return Err(PngDecodeErrors::GenericStatic("Invalid cICP chunk length"));
        }

        let color_primaries = self.stream.read_u8();
        let transfer_function = self.stream.read_u8();
        let matrix_coefficients = self.stream.read_u8();
        let video_full_range_flag = self.stream.read_u8();

        let cicp_info = CicpInfo {
            color_primaries,
            transfer_function,
            matrix_coefficients,
            video_full_range_flag,
        };

        self.png_info.cicp_info = Some(cicp_info);

        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the sBIT chunk (Significant Bits)
    pub(crate) fn parse_sbit(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        // sBIT length depends on the color type (1 to 4 bytes).
        // It's safest to read exactly the chunk length and validate it against max expected.
        if chunk.length == 0 || chunk.length > 4 {
            return Err(PngDecodeErrors::GenericStatic("Invalid sBIT chunk length"));
        }

        let mut sbit_bytes = [0u8; 4];
        for i in 0..chunk.length {
            sbit_bytes[i] = self.stream.read_u8();
        }

        self.png_info.sbit_info = Some(sbit_bytes);

        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }

    /// Parse the cHRM chunk (Primary Chromaticities and White Point)
    pub(crate) fn parse_chrm(&mut self, chunk: PngChunk) -> Result<(), PngDecodeErrors> {
        if chunk.length != 32 {
            return Err(PngDecodeErrors::GenericStatic("Invalid cHRM chunk length"));
        }

        // Values are encoded as (value * 100,000)
        let white_point_x = self.stream.get_u32_be();
        let white_point_y = self.stream.get_u32_be();
        let red_x = self.stream.get_u32_be();
        let red_y = self.stream.get_u32_be();
        let green_x = self.stream.get_u32_be();
        let green_y = self.stream.get_u32_be();
        let blue_x = self.stream.get_u32_be();
        let blue_y = self.stream.get_u32_be();

        let chrm_info = ChrmInfo {
            white_point_x,
            white_point_y,
            red_x,
            red_y,
            green_x,
            green_y,
            blue_x,
            blue_y,
        };
        self.png_info.chrm_info = Some(chrm_info);

        // skip crc
        self.stream.skip(4)?;
        Ok(())
    }
}
