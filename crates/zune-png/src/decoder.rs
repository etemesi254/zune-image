use alloc::vec::Vec;
use alloc::{format, vec};

use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_core::result::DecodingResult;
use zune_inflate::DeflateOptions;

use crate::constants::PNG_SIGNATURE;
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngErrors;
use crate::filters::{
    handle_avg, handle_avg_first, handle_avg_special, handle_avg_special_first,
    handle_none_special, handle_paeth, handle_paeth_first, handle_paeth_special,
    handle_paeth_special_first, handle_sub, handle_sub_special, handle_up, handle_up_special
};
use crate::options::{default_chunk_handler, UnkownChunkHandler};

/// A palette entry.
///
/// The alpha field is used if the image has a tRNS
/// chunk and pLTE chunk.
#[derive(Copy, Clone)]
pub(crate) struct PLTEEntry
{
    pub red:   u8,
    pub green: u8,
    pub blue:  u8,
    pub alpha: u8
}

impl Default for PLTEEntry
{
    fn default() -> Self
    {
        // but a tRNS chunk may contain fewer values than there are palette entries.
        // In this case, the alpha value for all remaining palette entries is assumed to be 255
        PLTEEntry {
            red:   0,
            green: 0,
            blue:  0,
            alpha: 255
        }
    }
}

#[derive(Copy, Clone)]
pub(crate) struct PngChunk
{
    pub length:     usize,
    pub chunk_type: PngChunkType,
    pub chunk:      [u8; 4],
    pub crc:        u32
}

/// Represents PNG information that can be extracted
/// from a png file.
///
/// The properties are read from IHDR chunk,
/// but may be changed by decoder during decoding
#[derive(Default, Debug, Copy, Clone)]
pub struct PngInfo
{
    pub width:            usize,
    pub height:           usize,
    pub depth:            u8,
    pub color:            PngColor,
    pub component:        u8,
    pub filter_method:    FilterMethod,
    pub interlace_method: InterlaceMethod
}

/// A PNG decoder instance.
///
/// This is the main decoder for png image decoding.
///
/// Instantiate the decoder with either the [new](PngDecoder::new)
/// or [new_with_options](PngDecoder::new_with_options) and
/// using either  of the [`decode_raw`](PngDecoder::decode_raw) or
/// [`decode`](PngDecoder::decode) will return pixels present in that image
///
/// # Note
/// The decoder currently expands images less than 8 bits per pixels to 8 bits per pixel
/// if this is not desired, then I'd suggest another png decoder
pub struct PngDecoder<'a>
{
    pub(crate) stream:          ZByteReader<'a>,
    pub(crate) options:         DecoderOptions,
    pub(crate) png_info:        PngInfo,
    pub(crate) palette:         Vec<PLTEEntry>,
    pub(crate) idat_chunks:     Vec<u8>,
    pub(crate) out:             Vec<u8>,
    pub(crate) expanded_stride: Vec<u8>,
    pub(crate) previous_stride: Vec<u8>,
    pub(crate) gama:            f32,
    pub(crate) trns_bytes:      [u16; 4],
    pub(crate) chunk_handler:   UnkownChunkHandler,
    pub(crate) seen_gamma:      bool,
    pub(crate) seen_hdr:        bool,
    pub(crate) seen_ptle:       bool,
    pub(crate) seen_headers:    bool,
    pub(crate) seen_trns:       bool,
    pub(crate) use_sse2:        bool,
    pub(crate) use_sse4:        bool
}

impl<'a> PngDecoder<'a>
{
    pub fn new(data: &'a [u8]) -> PngDecoder<'a>
    {
        let default_opt = DecoderOptions::default();

        PngDecoder::new_with_options(data, default_opt)
    }
    #[allow(unused_mut, clippy::redundant_field_names)]
    pub fn new_with_options(data: &'a [u8], options: DecoderOptions) -> PngDecoder<'a>
    {
        let mut use_sse2 = options.use_sse2();
        let mut use_sse4 = options.use_sse41();

        PngDecoder {
            seen_hdr:        false,
            stream:          ZByteReader::new(data),
            options:         options,
            palette:         Vec::new(),
            png_info:        PngInfo::default(),
            previous_stride: vec![],
            idat_chunks:     Vec::with_capacity(37), // randomly chosen size, my favourite number,
            out:             Vec::new(),
            gama:            0.0,
            expanded_stride: vec![],
            seen_ptle:       false,
            seen_trns:       false,
            seen_headers:    false,
            seen_gamma:      false,
            trns_bytes:      [0; 4],
            use_sse2:        use_sse2,
            use_sse4:        use_sse4,
            chunk_handler:   default_chunk_handler
        }
    }

    /// Get image dimensions or none if they aren't decoded
    pub const fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if !self.seen_hdr
        {
            return None;
        }

        Some((self.png_info.width, self.png_info.height))
    }
    pub const fn get_depth(&self) -> Option<BitDepth>
    {
        if !self.seen_hdr
        {
            return None;
        }
        match self.png_info.depth
        {
            1 | 2 | 4 | 8 => Some(BitDepth::Eight),
            16 => Some(BitDepth::Sixteen),
            _ => unreachable!()
        }
    }
    /// Get image gamma
    pub const fn get_gamma(&self) -> Option<f32>
    {
        if self.seen_gamma
        {
            Some(self.gama)
        }
        else
        {
            None
        }
    }
    /// Get image colorspace
    pub fn get_colorspace(&self) -> Option<ColorSpace>
    {
        if !self.seen_hdr
        {
            return None;
        }
        if !self.seen_trns
        {
            match self.png_info.color
            {
                PngColor::Palette => Some(ColorSpace::RGB),
                PngColor::Luma => Some(ColorSpace::Luma),
                PngColor::LumaA => Some(ColorSpace::LumaA),
                PngColor::RGB => Some(ColorSpace::RGB),
                PngColor::RGBA => Some(ColorSpace::RGBA),
                PngColor::Unknown => unreachable!()
            }
        }
        else
        {
            // for tRNS chunks, RGB=>RGBA
            // Luma=>LumaA, but if we are already in RGB and RGBA, just return
            // them
            match self.png_info.color
            {
                PngColor::Palette | PngColor::RGB => Some(ColorSpace::RGBA),
                PngColor::Luma => Some(ColorSpace::LumaA),
                PngColor::LumaA => Some(ColorSpace::LumaA),
                PngColor::RGBA => Some(ColorSpace::RGBA),
                _ => unreachable!()
            }
        }
    }
    fn read_chunk_header(&mut self) -> Result<PngChunk, PngErrors>
    {
        // Format is length - chunk type - [data] -  crc chunk, load crc chunk now
        let chunk_length = self.stream.get_u32_be_err()? as usize;
        let chunk_type_int = self.stream.get_u32_be_err()?.to_be_bytes();

        let mut crc_bytes = [0; 4];

        let crc_ref = self.stream.peek_at(chunk_length, 4)?;

        crc_bytes.copy_from_slice(crc_ref);

        let crc = u32::from_be_bytes(crc_bytes);

        let chunk_type = match &chunk_type_int
        {
            b"IHDR" => PngChunkType::IHDR,
            b"tRNS" => PngChunkType::tRNS,
            b"PLTE" => PngChunkType::PLTE,
            b"IDAT" => PngChunkType::IDAT,
            b"IEND" => PngChunkType::IEND,
            b"pHYs" => PngChunkType::pHYs,
            b"tIME" => PngChunkType::tIME,
            b"gAMA" => PngChunkType::gAMA,
            b"acTL" => PngChunkType::acTL,
            _ => PngChunkType::unkn
        };

        if !self.stream.has(chunk_length + 4 /*crc stream*/)
        {
            let err = format!(
                "Not enough bytes for chunk {:?}, bytes requested are {}, but bytes present are {}",
                chunk_type,
                chunk_length + 4,
                self.stream.remaining()
            );

            return Err(PngErrors::Generic(err));
        }
        // Confirm the CRC here.
        #[cfg(feature = "crc")]
        {
            if self.options.png_get_confirm_crc()
            {
                use crate::crc::crc32_slice8;

                // go back and point to chunk type.
                self.stream.rewind(4);
                // read chunk type + chunk data
                let bytes = self.stream.peek_at(0, chunk_length + 4).unwrap();

                // calculate crc
                let calc_crc = !crc32_slice8(bytes, u32::MAX);

                if crc != calc_crc
                {
                    return Err(PngErrors::BadCrc(crc, calc_crc));
                }
                // go point after the chunk type
                // The other parts expect the bit-reader to point to the
                // start of the chunk data.
                self.stream.skip(4);
            }
        }

        Ok(PngChunk {
            length: chunk_length,
            chunk: chunk_type_int,
            chunk_type,
            crc
        })
    }
    pub fn decode_headers(&mut self) -> Result<(), PngErrors>
    {
        // READ PNG signature
        let signature = self.stream.get_u64_be_err()?;

        if signature != PNG_SIGNATURE
        {
            return Err(PngErrors::BadSignature);
        }

        // check if first chunk is ihdr here
        if self.stream.peek_at(4, 4)? != b"IHDR"
        {
            return Err(PngErrors::GenericStatic(
                "First chunk not IHDR, Corrupt PNG"
            ));
        }
        loop
        {
            let header = self.read_chunk_header()?;

            match header.chunk_type
            {
                PngChunkType::IHDR =>
                {
                    self.parse_ihdr(header)?;
                }
                PngChunkType::PLTE =>
                {
                    self.parse_plte(header)?;
                }
                PngChunkType::IDAT =>
                {
                    self.parse_idat(header)?;
                }
                PngChunkType::tRNS =>
                {
                    self.parse_trns(header)?;
                }

                PngChunkType::gAMA =>
                {
                    self.parse_gama(header)?;
                }
                PngChunkType::acTL =>
                {
                    self.parse_actl(header)?;
                }
                PngChunkType::IEND =>
                {
                    break;
                }
                _ =>
                {
                    (self.chunk_handler)(header.length, header.chunk, &mut self.stream, header.crc)?
                }
            }
        }
        self.seen_headers = true;
        Ok(())
    }

    /// Decode PNG encoded images and return the vector of raw
    /// pixels
    ///
    /// The resulting vec may be bigger or smaller than expected
    /// depending on the bit depth of the image.
    ///
    /// The endianness is big endian for 16 bit images represented as two u8 slices
    ///
    /// This does not do gamma correction as opposed to decode,but may change
    /// in the future.
    pub fn decode_raw(&mut self) -> Result<Vec<u8>, PngErrors>
    {
        // decode headers
        if !self.seen_headers
        {
            self.decode_headers()?;
        }

        if self.expanded_stride.is_empty() && self.png_info.depth < 8
        {
            // add space for single stride
            // this will be used for small bit depths of less than 8 to expand
            // to 8 bits
            self.expanded_stride.resize(
                self.png_info.width * self.get_colorspace().unwrap().num_components(),
                0
            );
            self.previous_stride.resize(
                self.png_info.width * self.get_colorspace().unwrap().num_components(),
                0
            );
        }
        info!("Colorspace: {:?} ", self.get_colorspace().unwrap());

        let colorspace = self.get_colorspace().unwrap();
        let info = self.png_info;
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let mut img_width_bytes;

        img_width_bytes = colorspace.num_components() * info.width;
        img_width_bytes *= bytes;

        let image_len = img_width_bytes * info.height;

        self.out = vec![0; image_len];

        // go parse IDAT chunks returning the inflate
        let deflate_data = self.inflate()?;

        // remove idat chunks from memory
        // we are already done with them.
        self.idat_chunks = Vec::new();

        let new_len = info.width * info.height * colorspace.num_components() * bytes;

        if info.interlace_method == InterlaceMethod::Standard
        {
            // allocate out to be enough to hold raw decoded bytes

            self.create_png_image_raw(&deflate_data, info.width, info.height)?;
        }
        else if info.interlace_method == InterlaceMethod::Adam7
        {
            self.decode_interlaced(&deflate_data)?;
        }

        if self.seen_trns && self.png_info.color != PngColor::Palette
        {
            if info.depth == 8
            {
                self.compute_transparency();
            }
            else if info.depth == 16
            {
                // Tested by test_palette_trns_16bit.
                self.compute_transparency_16();
            }
        }
        // only un-palettize images if color type type is indexed
        // palette entries for true color and true color with alpha
        // are a suggestion for image viewers.
        // See https://www.w3.org/TR/2003/REC-PNG-20031110/#11PLTE
        if self.seen_ptle && self.png_info.color == PngColor::Palette
        {
            if self.palette.is_empty()
            {
                return Err(PngErrors::EmptyPalette);
            }
            if self.seen_trns
            {
                // if tRNS chunk is present in paletted images, it contains
                // alpha byte values, so that means we create alpha data from
                // raw bytes
                self.expand_palette(4);
            }
            else
            {
                // Normal expansion
                self.expand_palette(3);
            }
        }

        self.out.truncate(new_len);
        let out = core::mem::take(&mut self.out);

        Ok(out)
    }

    fn decode_interlaced(&mut self, deflate_data: &[u8]) -> Result<(), PngErrors>
    {
        let info = self.png_info;
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let out_n = self.get_colorspace().unwrap().num_components();

        let new_len = info.width * info.height * out_n * bytes;

        // A mad idea would be to make this multithreaded :)
        // They called me a mad man - Thanos
        let out_bytes = out_n * bytes;

        let mut final_out = vec![0_u8; new_len];

        const XORIG: [usize; 7] = [0, 4, 0, 2, 0, 1, 0];
        const YORIG: [usize; 7] = [0, 0, 4, 0, 2, 0, 1];

        const XSPC: [usize; 7] = [8, 8, 4, 4, 2, 2, 1];
        const YSPC: [usize; 7] = [8, 8, 8, 4, 4, 2, 2];

        let mut image_offset = 0;

        // get the maximum height and width for the whole interlace part
        for p in 0..7
        {
            let x = (info
                .width
                .saturating_sub(XORIG[p])
                .saturating_add(XSPC[p])
                .saturating_sub(1))
                / XSPC[p];

            let y = (info
                .height
                .saturating_sub(YORIG[p])
                .saturating_add(YSPC[p])
                .saturating_sub(1))
                / YSPC[p];

            if x != 0 && y != 0
            {
                let mut image_len = usize::from(info.color.num_components()) * x;

                image_len *= usize::from(info.depth);
                image_len += 7;
                image_len /= 8;
                image_len += 1; // filter byte
                image_len *= y;

                if image_offset + image_len > deflate_data.len()
                {
                    return Err(PngErrors::GenericStatic("Too short data"));
                }

                let deflate_slice = &deflate_data[image_offset..image_offset + image_len];

                self.create_png_image_raw(deflate_slice, x, y)?;

                for j in 0..y
                {
                    for i in 0..x
                    {
                        let out_y = j * YSPC[p] + YORIG[p];
                        let out_x = i * XSPC[p] + XORIG[p];

                        let final_start = out_y * info.width * out_bytes + out_x * out_bytes;
                        let out_start = (j * x + i) * out_bytes;

                        final_out[final_start..final_start + out_bytes]
                            .copy_from_slice(&self.out[out_start..out_start + out_bytes]);
                    }
                }
                image_offset += image_len;
            }
        }
        self.out = final_out;
        Ok(())
    }
    fn compute_transparency(&mut self)
    {
        // for images whose color types are not paletted
        // presence of a tRNS chunk indicates that the image
        // has transparency.
        // When the pixel specified  in the tRNS chunk is encountered in the resulting stream,
        // it is to be treated as fully transparent.
        // We indicate that by replacing the pixel with pixel+alpha and setting alpha to be zero;
        // to indicate fully transparent.
        //
        //
        // OPTIMIZATION: This can be done in place, stb does it in place.
        // but it needs some complexity in the de-filtering, which I won't do.
        // but anyone reading this is free to do it

        let info = &self.png_info;

        match info.color
        {
            PngColor::Luma =>
            {
                let trns_byte = ((self.trns_bytes[0]) & 255) as u8;

                for chunk in self.out.chunks_exact_mut(2)
                {
                    chunk[1] = u8::from(chunk[0] != trns_byte) * 255;
                }
                // change color type to be the one with alpha
            }
            PngColor::RGB =>
            {
                let r = (self.trns_bytes[0] & 255) as u8;
                let g = (self.trns_bytes[1] & 255) as u8;
                let b = (self.trns_bytes[2] & 255) as u8;

                let r_matrix = [r, g, b];

                for chunk in self.out.chunks_exact_mut(4)
                {
                    if &chunk[0..3] != &r_matrix
                    {
                        chunk[3] = 255;
                    }
                }
            }
            _ => unreachable!()
        }
    }
    fn compute_transparency_16(&mut self)
    {
        // for explanation see compute_transparency
        //
        // OPT_GUIDE:
        //
        // There is a quirky optimization here, and it has something
        // to do with the _to_ne_bytes.
        // The tRNS items were read as big endian but the data is
        // currently in native endian, so matching won't work across byte
        // boundaries.
        // But if we convert the tRNS data to native endian, the two endianness
        // match and we can do comparisons

        let info = &self.png_info;

        match info.color
        {
            PngColor::Luma =>
            {
                let trns_byte = self.trns_bytes[0].to_ne_bytes();

                for chunk in self.out.chunks_exact_mut(4)
                {
                    if trns_byte != &chunk[0..2]
                    {
                        chunk[2] = 255;
                        chunk[3] = 255;
                    }
                }
                // change color type to be the one with alpha
                self.png_info.color = PngColor::LumaA;
            }
            PngColor::RGB =>
            {
                let r = self.trns_bytes[0].to_ne_bytes();
                let g = self.trns_bytes[1].to_ne_bytes();
                let b = self.trns_bytes[2].to_ne_bytes();

                // copy all trns chunks into one big vector
                let mut all: [u8; 6] = [0; 6];

                all[0..2].copy_from_slice(&r);
                all[2..4].copy_from_slice(&g);
                all[4..6].copy_from_slice(&b);

                for chunk in self.out.chunks_exact_mut(8)
                {
                    // the read does not match the bytes
                    // so set it to opaque
                    if all != &chunk[0..6]
                    {
                        chunk[6] = 255;
                        chunk[7] = 255;
                    }
                }
                // change color type to be the one with alpha
                self.png_info.color = PngColor::RGBA;
            }
            _ => unreachable!()
        }
        //self.out = new_out;
    }
    /// Decode PNG encoded images and return the vector of raw pixels but for 16-bit images
    /// represent them in a Vec<u16>
    ///
    /// This does one extra allocation when compared to `decode_raw` for 16 bit images to create the
    /// necessary representation of 16 bit images.
    pub fn decode(&mut self) -> Result<DecodingResult, PngErrors>
    {
        let out = self.decode_raw()?;

        if self.png_info.depth <= 8
        {
            return Ok(DecodingResult::U8(out));
        }
        if self.png_info.depth == 16
        {
            // https://github.com/etemesi254/zune-image/issues/36
            let new_array: Vec<u16> = out
                .chunks_exact(2)
                .map(|chunk| {
                    let value: [u8; 2] = chunk.try_into().unwrap();
                    u16::from_be_bytes(value)
                })
                .collect();

            return Ok(DecodingResult::U16(new_array));
        }
        Err(PngErrors::GenericStatic("Not implemented"))
    }
    /// Create the png data from post deflated data
    ///
    /// `self.out` needs to have enough space to hold data, otherwise
    /// this will panic
    ///
    /// This is to allow reuse e.g interlaced images use one big allocation
    /// to and since that ends up calling this multiple times, allocation was moved
    /// away from this method to the caller of this method
    #[allow(clippy::manual_memcpy, clippy::comparison_chain)]
    fn create_png_image_raw(
        &mut self, deflate_data: &[u8], width: usize, height: usize
    ) -> Result<(), PngErrors>
    {
        let use_sse4 = self.use_sse4;
        let use_sse2 = self.use_sse2;

        let info = self.png_info;
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let out_colorspace = self.get_colorspace().unwrap();
        let out_components = out_colorspace.num_components() * bytes;

        let mut img_width_bytes;

        img_width_bytes = usize::from(info.component) * width;
        img_width_bytes *= usize::from(info.depth);
        img_width_bytes += 7;
        img_width_bytes /= 8;

        let out_n = usize::from(info.color.num_components());

        let image_len = img_width_bytes * height;

        if deflate_data.len() < image_len + height
        // account for filter bytes
        {
            let msg = format!(
                "Not enough pixels, expected {} but found {}",
                image_len,
                deflate_data.len()
            );
            return Err(PngErrors::Generic(msg));
        }
        // do png  un-filtering
        let mut chunk_size;
        let mut components = usize::from(info.color.num_components()) * bytes;

        if info.depth < 8
        {
            // if the bit depth is 8, the spec says the byte before
            // X to be used by the filter
            components = 1;
        }

        // if info.depth < 8
        // {
        //     // for bit packed components, do not allocate space, do it the normal way
        //     out_components = components;
        // }

        // add width plus colour component, this gives us number of bytes per every scan line
        chunk_size = width * out_n;
        chunk_size *= usize::from(info.depth);
        chunk_size += 7;
        chunk_size /= 8;
        // filter type
        chunk_size += 1;

        let out_chunk_size = width * out_colorspace.num_components() * bytes;

        // each chunk is a width stride of unfiltered data
        let chunks = deflate_data.chunks_exact(chunk_size);

        // Begin doing loop un-filtering.
        let width_stride = chunk_size - 1;

        let mut prev_row_start = 0;
        let mut first_row = true;
        let mut out_position = 0;

        for in_stride in chunks.take(height)
        {
            // Split output into current and previous
            // current points to the start of the row where we are writing de-filtered output to
            // prev is all rows we already wrote output to.

            let (prev, mut current) = self.out.split_at_mut(out_position);

            current = &mut current[0..out_chunk_size];

            // get the previous row.
            //Set this to a dummy to handle special case of first row, if we aren't in the first
            // row, we actually take the real slice a line down
            let mut prev_row: &[u8] = &[0_u8];

            if !first_row
            {
                if info.depth < 8
                {
                    // for lower depths, we expanded the previous row,
                    // so we can't use the previous row as expected.

                    // But we backed up the unexpanded back row  in
                    // self.previous_stride (only for depth <8), and
                    // hence we use it here as our backup row
                    prev_row = &self.previous_stride[0..chunk_size];
                }
                else
                {
                    // normal bit depth, use the previous row as normal
                    prev_row = &prev[prev_row_start..prev_row_start + out_chunk_size];
                    prev_row_start += out_chunk_size;
                }
            }

            out_position += out_chunk_size;

            // take filter
            let filter_byte = in_stride[0];
            // raw image bytes
            let raw = &in_stride[1..];

            // get it's type
            let mut filter = FilterMethod::from_int(filter_byte)
                .ok_or_else(|| PngErrors::Generic(format!("Unknown filter {filter_byte}")))?;

            if first_row
            {
                // match our filters to special filters for first row
                // these special filters do not need the previous scanline and treat it
                // as zero

                if filter == FilterMethod::Paeth
                {
                    filter = FilterMethod::PaethFirst;
                }
                if filter == FilterMethod::Up
                {
                    // up for the first row becomes a memcpy
                    filter = FilterMethod::None;
                }
                if filter == FilterMethod::Average
                {
                    filter = FilterMethod::AvgFirst;
                }

                first_row = false;
            }
            if components == out_components || info.depth < 8
            {
                match filter
                {
                    FilterMethod::None => current[0..width_stride].copy_from_slice(raw),

                    FilterMethod::Average =>
                    {
                        handle_avg(prev_row, raw, current, components, use_sse4)
                    }

                    FilterMethod::Sub => handle_sub(raw, current, components, use_sse2),

                    FilterMethod::Up => handle_up(prev_row, raw, current),

                    FilterMethod::Paeth =>
                    {
                        handle_paeth(prev_row, raw, current, components, use_sse4)
                    }

                    FilterMethod::PaethFirst => handle_paeth_first(raw, current, components),

                    FilterMethod::AvgFirst => handle_avg_first(raw, current, components),

                    FilterMethod::Unknown => unreachable!()
                }
            }
            else if components < out_components
            {
                // in and out don't match. Like paletted images, images with tRNS chunks that
                // we will expand later on.
                match filter
                {
                    FilterMethod::Average =>
                    {
                        handle_avg_special(raw, prev_row, current, components, out_components)
                    }
                    FilterMethod::AvgFirst =>
                    {
                        handle_avg_special_first(raw, current, components, out_components)
                    }
                    FilterMethod::None =>
                    {
                        handle_none_special(raw, current, components, out_components)
                    }
                    FilterMethod::Up =>
                    {
                        handle_up_special(raw, prev_row, current, components, out_components)
                    }
                    FilterMethod::Sub =>
                    {
                        handle_sub_special(raw, current, components, out_components)
                    }
                    FilterMethod::PaethFirst =>
                    {
                        handle_paeth_special_first(raw, current, components, out_components)
                    }
                    FilterMethod::Paeth =>
                    {
                        handle_paeth_special(raw, prev_row, current, components, out_components)
                    }

                    FilterMethod::Unknown =>
                    {
                        unreachable!()
                    }
                }
            }
            // Expand images less than 8 bit to 8 bpp
            //
            // This has some complexity because we run it in the same scanline
            // that we just decoded
            if info.depth < 8
            {
                let in_offset = out_position - out_chunk_size;
                let out_len = width * out_n;

                self.expand_bits_to_byte(width, in_offset, out_n);

                let out_slice = &mut self.out[out_position - out_chunk_size..out_position];
                // save the previous row to be used in the next pass
                self.previous_stride[..chunk_size].copy_from_slice(&out_slice[..chunk_size]);

                if out_n == out_components
                {
                    // This includes cases like 1bpp RGBA image which
                    // has no tRNS or palette images

                    //
                    // input components match output components.
                    // simple memory copy
                    out_slice.copy_from_slice(&self.expanded_stride[0..out_len]);
                }
                else if out_n < out_components
                {
                    // For the case where we have images with less than 8 bpp and
                    // also have more post processing steps.
                    // E.g an image with 1 bpp that use palettes, or has a tRNS chunk

                    // the output chunks, these are where we will be filling our
                    // expanded bytes
                    let out_chunks = out_slice.chunks_exact_mut(out_components);
                    // where we are reading the expanded bytes from
                    let in_chunks = self.expanded_stride[..out_len].chunks_exact(out_n);

                    // loop copying from in_bytes to out bytes.
                    for (out_c, in_c) in out_chunks.zip(in_chunks)
                    {
                        out_c[..in_c.len()].copy_from_slice(in_c);
                    }
                }
            }
        }
        Ok(())
    }
    /// Expand bits to bytes expand images with less than 8 bpp
    fn expand_bits_to_byte(&mut self, width: usize, mut in_offset: usize, out_n: usize)
    {
        // How it works
        // ------------
        // We want to expand the current row, current row was written in
        // self.out[in_offset..] up until ceil_div((width*depth),8)
        //
        // We want to write the expanded row to self.expanded_stride
        //
        // So we get in_offset from the caller, for lower bit depths,
        // numbers are clumped up together, even if they have tRNS and PLTE chunks
        //
        // So we expand the lower bit depths starting from self.out[in_offset] incrementing
        // it, until we have expanded a single row, then we return to the caller
        //
        //
        //
        const DEPTH_SCALE_TABLE: [u8; 9] = [0, 0xff, 0x55, 0, 0x11, 0, 0, 0, 0x01];

        let info = &self.png_info;

        let mut current = 0;

        let mut scale = DEPTH_SCALE_TABLE[usize::from(info.depth)];

        // for pLTE chunks with lower bit depths
        // do not scale values just expand.
        // The palette pass will expand values to the right pixels.
        if self.seen_ptle && self.png_info.depth < 8
        {
            scale = 1;
        }

        let mut k = width * out_n;

        if info.depth == 1
        {
            while k >= 8
            {
                let cur: &mut [u8; 8] = self
                    .expanded_stride
                    .get_mut(current..current + 8)
                    .unwrap()
                    .try_into()
                    .unwrap();

                let in_val = self.out[in_offset];

                cur[0] = scale * ((in_val >> 7) & 0x01);
                cur[1] = scale * ((in_val >> 6) & 0x01);
                cur[2] = scale * ((in_val >> 5) & 0x01);
                cur[3] = scale * ((in_val >> 4) & 0x01);
                cur[4] = scale * ((in_val >> 3) & 0x01);
                cur[5] = scale * ((in_val >> 2) & 0x01);
                cur[6] = scale * ((in_val >> 1) & 0x01);
                cur[7] = scale * ((in_val) & 0x01);

                in_offset += 1;
                current += 8;

                k -= 8;
            }
            if k > 0
            {
                let in_val = self.out[in_offset];

                for p in 0..k
                {
                    let shift = (7_usize).wrapping_sub(p);
                    self.expanded_stride[current] = scale * ((in_val >> shift) & 0x01);
                    current += 1;
                }
            }
        }
        else if info.depth == 2
        {
            while k >= 4
            {
                let cur: &mut [u8; 4] = self
                    .expanded_stride
                    .get_mut(current..current + 4)
                    .unwrap()
                    .try_into()
                    .unwrap();

                let in_val = self.out[in_offset];

                cur[0] = scale * ((in_val >> 6) & 0x03);
                cur[1] = scale * ((in_val >> 4) & 0x03);
                cur[2] = scale * ((in_val >> 2) & 0x03);
                cur[3] = scale * ((in_val) & 0x03);

                k -= 4;

                in_offset += 1;
                current += 4;
            }
            if k > 0
            {
                let in_val = self.out[in_offset];

                for p in 0..k
                {
                    let shift = (6_usize).wrapping_sub(p * 2);
                    self.expanded_stride[current] = scale * ((in_val >> shift) & 0x03);
                    current += 1;
                }
            }
        }
        else if info.depth == 4
        {
            while k >= 2
            {
                let cur: &mut [u8; 2] = self
                    .expanded_stride
                    .get_mut(current..current + 2)
                    .unwrap()
                    .try_into()
                    .unwrap();
                let in_val = self.out[in_offset];

                cur[0] = scale * ((in_val >> 4) & 0x0f);
                cur[1] = scale * ((in_val) & 0x0f);

                k -= 2;

                in_offset += 1;
                current += 2;
            }

            if k > 0
            {
                let in_val = self.out[in_offset];

                // leftovers
                for p in 0..k
                {
                    let shift = (4_usize).wrapping_sub(p * 4);
                    self.expanded_stride[current] = scale * ((in_val >> shift) & 0x0f);
                    current += 1;
                }
            }
        }
    }
    /// Undo deflate decoding
    #[allow(clippy::manual_memcpy)]
    fn inflate(&mut self) -> Result<Vec<u8>, PngErrors>
    {
        // An annoying thing is that deflate doesn't
        // store its uncompressed size,
        // so we can't pre-allocate storage and pass that willy nilly
        //
        // Meaning we are left with some design choices
        // 1. Have deflate resize at will
        // 2. Have deflate return incomplete, to indicate we need to extend
        // the vec, extend and go back to inflate.
        //
        //
        // so choose point 1.
        //
        // This allows the zlib decoder to optimize its own paths(which it does)
        // because it controls the allocation and doesn't have to check for near EOB
        // runs.
        //
        let depth_scale = if self.png_info.depth == 16 { 2 } else { 1 };

        let size_hint = (self.png_info.width + 1)
            * self.png_info.height
            * depth_scale
            * usize::from(self.png_info.color.num_components());

        let option = DeflateOptions::default()
            .set_size_hint(size_hint)
            .set_limit(size_hint + 4 * (self.png_info.height))
            .set_confirm_checksum(self.options.inflate_get_confirm_adler());

        let mut decoder = zune_inflate::DeflateDecoder::new_with_options(&self.idat_chunks, option);

        decoder.decode_zlib().map_err(PngErrors::ZlibDecodeErrors)
    }

    /// Expand a palettized image to the number of components
    fn expand_palette(&mut self, components: usize)
    {
        if components == 0
        {
            return;
        }

        // this is safe because we resized palette to be 256
        // in self.parse_plte()
        let palette: &[PLTEEntry; 256] = &self.palette[0..256].try_into().unwrap();

        if components == 3
        {
            for px in self.out.chunks_exact_mut(3)
            {
                // the & 255 may be removed as the compiler can see u8 can never be
                // above 255, but for safety
                let entry = palette[usize::from(px[0]) & 255];

                px[0] = entry.red;
                px[1] = entry.green;
                px[2] = entry.blue;
            }
        }
        else if components == 4
        {
            for px in self.out.chunks_exact_mut(4)
            {
                let entry = palette[usize::from(px[0]) & 255];

                px[0] = entry.red;
                px[1] = entry.green;
                px[2] = entry.blue;
                px[3] = entry.alpha;
            }
        }
    }
}
