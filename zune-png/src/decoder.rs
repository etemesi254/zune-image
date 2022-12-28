use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::DecodingResult;

use crate::constants::PNG_SIGNATURE;
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngErrors;
use crate::filters::{
    handle_avg, handle_avg_first, handle_paeth, handle_paeth_first, handle_sub, handle_up
};
use crate::options::PngOptions;

#[derive(Copy, Clone)]
pub(crate) struct PngChunk
{
    pub length:     usize,
    pub chunk_type: PngChunkType,
    pub chunk:      [u8; 4],
    pub crc:        u32
}

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

pub struct PngDecoder<'a>
{
    pub(crate) seen_hdr:    bool,
    pub(crate) stream:      ZByteReader<'a>,
    pub(crate) options:     PngOptions,
    pub(crate) png_info:    PngInfo,
    pub(crate) palette:     Vec<u8>,
    pub(crate) idat_chunks: Vec<u8>,
    pub(crate) out:         Vec<u8>
}

impl<'a> PngDecoder<'a>
{
    pub fn new(data: &'a [u8]) -> PngDecoder<'a>
    {
        let default_opt = PngOptions::default();

        PngDecoder::new_with_options(data, default_opt)
    }
    pub fn new_with_options(data: &'a [u8], options: PngOptions) -> PngDecoder<'a>
    {
        PngDecoder {
            seen_hdr: false,
            stream: ZByteReader::new(data),
            options,
            palette: Vec::new(),
            png_info: PngInfo::default(),
            idat_chunks: Vec::with_capacity(37), // randomly chosen size, my favourite number,
            out: Vec::new()
        }
    }

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
    pub fn get_colorspace(&self) -> Option<ColorSpace>
    {
        if !self.seen_hdr
        {
            return None;
        }
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
            if self.options.confirm_crc
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

    /// Decode PNG encoded images and return the vector of raw
    /// pixels
    pub fn decode(&mut self) -> Result<DecodingResult, PngErrors>
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
                    self.parse_plt(header)?;
                }
                PngChunkType::IDAT =>
                {
                    self.parse_idat(header)?;
                }
                PngChunkType::tRNS =>
                {
                    self.parse_trns(header)?;
                }

                PngChunkType::IEND =>
                {
                    break;
                }
                _ => (self.options.chunk_handler)(
                    header.length,
                    header.chunk,
                    &mut self.stream,
                    header.crc
                )?
            }
        }
        // go parse IDAT chunks returning the inflate
        let deflate_data = self.inflate()?;
        // remove idat chunks from memory
        // we are already done with them.
        self.idat_chunks = Vec::new();

        let info = self.png_info;

        let out_n = usize::from(info.color.num_components());

        if info.interlace_method == InterlaceMethod::Standard
        {
            self.create_png_image_raw(&deflate_data, info.width, info.height)?;

            // images with depth of 8, no interlace or filter can proceed to be returned
            if info.depth == 8
            {
                let new_len = info.width * info.height * usize::from(info.color.num_components());
                self.out.truncate(new_len);
                let out = std::mem::take(&mut self.out);
                return Ok(DecodingResult::U8(out));
            }
        }
        else if info.interlace_method == InterlaceMethod::Adam7
        {
            let new_len = info.width * info.height * usize::from(info.color.num_components());

            let mut final_out = vec![0_u8; new_len];
            const XORIG: [usize; 7] = [0, 4, 0, 2, 0, 1, 0];
            const YORIG: [usize; 7] = [0, 0, 4, 0, 2, 0, 1];

            const XSPC: [usize; 7] = [8, 8, 4, 4, 2, 2, 1];
            const YSPC: [usize; 7] = [8, 8, 8, 4, 4, 2, 2];

            let mut image_offset = 0;

            for p in 0..7
            {
                let x = (info.width - XORIG[p] + XSPC[p] - 1) / XSPC[p];
                let y = (info.height - YORIG[p] + YSPC[p] - 1) / YSPC[p];

                if x != 0 && y != 0
                {
                    let mut image_len = usize::from(info.color.num_components()) * x;

                    image_len *= usize::from(info.depth);
                    image_len += 7;
                    image_len >>= 3;
                    image_len += 1;
                    image_len *= y;

                    self.create_png_image_raw(
                        &deflate_data[image_offset..image_offset + image_len],
                        x,
                        y
                    )?;
                    let bytes = if info.depth == 16 { 2 } else { 1 };
                    let out_bytes = out_n * bytes;

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

            if info.depth <= 8
            {
                let new_len = info.width * info.height * usize::from(info.color.num_components());
                self.out.truncate(new_len);
                let out = std::mem::take(&mut self.out);
                return Ok(DecodingResult::U8(out));
            }
        }

        Err(PngErrors::GenericStatic("Not yet done"))
    }
    /// Create the png data from post deflated data
    #[allow(clippy::manual_memcpy)]
    fn create_png_image_raw(
        &mut self, deflate_data: &[u8], width: usize, height: usize
    ) -> Result<(), PngErrors>
    {
        const DEPTH_SCALE_TABLE: [u8; 9] = [0, 0xff, 0x55, 0, 0x11, 0, 0, 0, 0x01];

        let info = &self.png_info;

        let bytes = if info.depth == 16 { 2 } else { 1 };

        let mut img_width_bytes;

        let out_n = usize::from(info.color.num_components());

        img_width_bytes = usize::from(info.component) * width;
        img_width_bytes *= usize::from(info.depth);
        img_width_bytes += 7;
        img_width_bytes >>= 3;

        let image_len = (img_width_bytes + 1) * height;

        self.out = vec![0; image_len];

        let out = &mut self.out;

        let stride = width * out_n * bytes;

        if deflate_data.len() < image_len
        {
            let msg = format!(
                "Not enough pixels, expected {} but found {}",
                image_len,
                deflate_data.len()
            );
            return Err(PngErrors::Generic(msg));
        }
        // stride
        // do png  un-filtering
        let mut chunk_size;

        let mut components = usize::from(info.color.num_components());

        if info.depth < 8
        {
            // if the bit depth is 8, the spec says the byte before
            // X to be used by the filter
            components = 1;
        }

        // add width plus colour component, this gives us number of bytes per every scan line
        chunk_size = width * out_n;
        // add depth, and
        chunk_size *= usize::from(info.depth);
        chunk_size += 7;
        chunk_size /= 8;
        // filter type
        chunk_size += 1;

        let chunks = deflate_data.chunks_exact(chunk_size);

        //
        // ┌─────┬─────┐
        // │ c   │  b  │
        // ├─────┼─────┤
        // │ a   │ x   │
        // └─────┴─────┘
        //
        // Begin doing loop un-filtering.

        let mut prev_row_start = 0;
        let width_stride = chunk_size - 1;

        let mut first_row = true;

        let mut out_position = 0;

        for in_stride in chunks.take(height)
        {
            // Split output into current and previous
            // current points to the start of the row where we are writing de-filtered output to
            // prev is all rows we already wrote output to.
            let (prev, current) = out.split_at_mut(out_position);

            // get the previous row.
            //Set this to a dummy to handle special case of first row, if we aren't in the first
            // row, we actually take the real slice a line down
            let mut prev_row: &[u8] = &[0_u8];

            if !first_row
            {
                prev_row = &prev[prev_row_start..prev_row_start + width_stride];
                prev_row_start += width_stride;
            }

            out_position += width_stride;

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

            match filter
            {
                FilterMethod::None =>
                {
                    // Memcpy
                    current[0..width_stride].copy_from_slice(raw)
                }

                FilterMethod::Average => handle_avg(prev_row, raw, current, components),

                FilterMethod::Sub => handle_sub(raw, current, components),

                FilterMethod::Up => handle_up(prev_row, raw, current),

                FilterMethod::Paeth => handle_paeth(prev_row, raw, current, components),

                FilterMethod::PaethFirst => handle_paeth_first(raw, current, components),

                FilterMethod::AvgFirst => handle_avg_first(raw, current, components),

                FilterMethod::Unknown => unreachable!()
            }
        }
        // make a separate pass to expand bits to pixels
        if info.depth < 8
        {
            // okay this depth up-scaling can be done in place
            // stb_image does it, it's a performance benefit to do it that way
            // but for GOD's sake, there are too many pointer arithmetic and implicit
            // things I cannot even begin to wrap my head on how to go about it
            //
            // So just allocate a new byte, write to that and set it to be
            // out later on
            let new_size = height * width * out_n;
            let mut new_out = vec![0; new_size];

            let mut in_offset = 0;

            for i in 0..height
            {
                let mut current = stride * i;

                let scale = if info.color == PngColor::Luma
                {
                    DEPTH_SCALE_TABLE[usize::from(info.depth)]
                }
                else
                {
                    1
                };

                let mut k = width * out_n;
                let mut in_val = out[in_offset];

                if info.depth == 1
                {
                    while k >= 8
                    {
                        let cur: &mut [u8; 8] =
                            &mut new_out[current..current + 8].try_into().unwrap();

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

                        in_val = out[in_offset];

                        k -= 8;
                    }
                    for p in 0..k
                    {
                        let shift = (7_usize).wrapping_sub(p);
                        new_out[current] = scale * ((in_val >> shift) & 0x01);
                        current += 1;
                    }
                }
                else if info.depth == 2
                {
                    while k >= 4
                    {
                        let cur: &mut [u8; 4] = &mut out[current..current + 4].try_into().unwrap();

                        cur[0] = scale * ((in_val >> 6) & 0x03);
                        cur[1] = scale * ((in_val >> 4) & 0x03);
                        cur[2] = scale * ((in_val >> 2) & 0x03);
                        cur[3] = scale * ((in_val) & 0x03);

                        k -= 4;

                        in_offset += 1;
                        current += 4;

                        in_val = out[in_offset];
                    }

                    for p in 0..k
                    {
                        let shift = (6_usize).wrapping_sub(p * 2);
                        new_out[current] = scale * ((in_val >> shift) & 0x03);
                        current += 1;
                    }
                }
                else if info.depth == 4
                {
                    while k >= 2
                    {
                        let cur: &mut [u8; 2] = &mut out[current..current + 2].try_into().unwrap();

                        cur[1] = scale * ((in_val >> 4) & 0x0f);
                        cur[2] = scale * ((in_val) & 0x0f);

                        k -= 2;

                        in_offset += 1;
                        current += 2;

                        in_val = out[in_offset];
                    }

                    // leftovers
                    for p in 0..k
                    {
                        let shift = (4_usize).wrapping_sub(p * 4);
                        new_out[current] = scale * ((in_val >> shift) & 0x0f);
                        current += 1;
                    }
                }
                in_offset += 1;
            }
            self.out = new_out;
        }

        Ok(())
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

        let mut decoder = zune_inflate::DeflateDecoder::new(&self.idat_chunks);

        decoder.decode_zlib().map_err(PngErrors::ZlibDecodeErrors)
    }
}
