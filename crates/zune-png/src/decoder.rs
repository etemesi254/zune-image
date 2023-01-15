use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
use zune_inflate::DeflateOptions;

use crate::constants::PNG_SIGNATURE;
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngErrors;
use crate::filters::{
    handle_avg, handle_avg_first, handle_paeth, handle_paeth_first, handle_sub, handle_up
};
use crate::options::PngOptions;

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
    pub(crate) seen_hdr:     bool,
    pub(crate) stream:       ZByteReader<'a>,
    pub(crate) options:      PngOptions,
    pub(crate) png_info:     PngInfo,
    pub(crate) palette:      Vec<PLTEEntry>,
    pub(crate) idat_chunks:  Vec<u8>,
    pub(crate) out:          Vec<u8>,
    pub(crate) gama:         u32,
    pub(crate) un_palettize: bool,
    pub(crate) seen_trns:    bool
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
            out: Vec::new(),
            gama: 0, // used also to indicate if we should do gama unfiltering
            un_palettize: false,
            seen_trns: false
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
            b"gAMA" => PngChunkType::gAMA,
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
            if self.options.confirm_checksums
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
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let out_n = usize::from(info.color.num_components());

        let mut new_len =
            info.width * info.height * usize::from(info.color.num_components()) * bytes;

        if info.interlace_method == InterlaceMethod::Standard
        {
            // allocate out to be enough to hold raw decoded bytes
            let mut img_width_bytes;

            img_width_bytes = usize::from(info.component) * info.width;
            img_width_bytes *= usize::from(info.depth);
            img_width_bytes += 7;
            img_width_bytes /= 8;

            let image_len = img_width_bytes * info.height;

            self.out = vec![0; image_len];

            self.create_png_image_raw(&deflate_data, info.width, info.height)?;
        }
        else if info.interlace_method == InterlaceMethod::Adam7
        {
            // A mad idea would be to make this multithreaded :)
            // They called me a mad man - Thanos
            let out_bytes = out_n * bytes;

            let mut final_out = vec![0_u8; new_len];

            const XORIG: [usize; 7] = [0, 4, 0, 2, 0, 1, 0];
            const YORIG: [usize; 7] = [0, 0, 4, 0, 2, 0, 1];

            const XSPC: [usize; 7] = [8, 8, 4, 4, 2, 2, 1];
            const YSPC: [usize; 7] = [8, 8, 8, 4, 4, 2, 2];

            let mut image_offset = 0;

            let mut max_height = 0;
            let mut max_width = 0;

            // get the maximum height and width for the whole interlace part
            for p in 0..7
            {
                let x = (info.width - XORIG[p] + XSPC[p] - 1) / XSPC[p];
                let y = (info.height - YORIG[p] + YSPC[p] - 1) / YSPC[p];

                max_height = max_height.max(y);
                max_width = max_width.max(x);
            }
            {
                // then allocate a vector big enough to hold the largest pass
                let mut image_len = usize::from(info.color.num_components()) * max_width;

                image_len *= usize::from(info.depth);
                image_len += 7;
                image_len /= 8;
                image_len += 1;
                image_len *= max_height;

                self.out = vec![0; image_len];
            }

            for p in 0..7
            {
                let x = (info.width - XORIG[p] + XSPC[p] - 1) / XSPC[p];
                let y = (info.height - YORIG[p] + YSPC[p] - 1) / YSPC[p];

                if x != 0 && y != 0
                {
                    let mut image_len = usize::from(info.color.num_components()) * x;

                    image_len *= usize::from(info.depth);
                    image_len += 7;
                    image_len /= 8;
                    image_len += 1; // filter byte
                    image_len *= y;

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
        }

        if self.un_palettize
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
                self.png_info.color = PngColor::RGBA;
                // initially it was 1, since palette num_components is 1, so multiply
                // by 4 since for palettized images we turn it to RGBA
                new_len *= 4;
            }
            else
            {
                self.expand_palette(3);
                self.png_info.color = PngColor::RGB;
                // initially it was 1, since palette num_components is 1, so multiply
                // by three since for palettized images we turn it to RGB
                new_len *= 3;
            }
        }

        self.out.truncate(new_len);
        let out = std::mem::take(&mut self.out);

        Ok(out)
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
    #[allow(clippy::manual_memcpy)]
    fn create_png_image_raw(
        &mut self, deflate_data: &[u8], width: usize, height: usize
    ) -> Result<(), PngErrors>
    {
        let info = &self.png_info;
        let bytes = if info.depth == 16 { 2 } else { 1 };

        let mut img_width_bytes;

        img_width_bytes = usize::from(info.component) * width;
        img_width_bytes *= usize::from(info.depth);
        img_width_bytes += 7;
        img_width_bytes /= 8;

        let image_len = img_width_bytes * height;
        let out_n = usize::from(info.color.num_components());
        let out = &mut self.out[0..image_len];

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

        // add width plus colour component, this gives us number of bytes per every scan line
        chunk_size = width * out_n;
        chunk_size *= usize::from(info.depth);
        chunk_size += 7;
        chunk_size /= 8;
        // filter type
        chunk_size += 1;

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
                FilterMethod::None => current[0..width_stride].copy_from_slice(raw),

                FilterMethod::Average => handle_avg(prev_row, raw, current, components),

                FilterMethod::Sub => handle_sub(raw, current, components),

                FilterMethod::Up => handle_up(prev_row, raw, current),

                FilterMethod::Paeth => handle_paeth(prev_row, raw, current, components),

                FilterMethod::PaethFirst => handle_paeth_first(raw, current, components),

                FilterMethod::AvgFirst => handle_avg_first(raw, current, components),

                FilterMethod::Unknown => unreachable!()
            }
        }
        if self.png_info.depth < 8
        {
            self.expand_bits_to_byte(height, width, out_n)
        }

        Ok(())
    }
    /// Expand bits to bytes expand images with less than 8 bpp
    fn expand_bits_to_byte(&mut self, height: usize, width: usize, out_n: usize)
    {
        const DEPTH_SCALE_TABLE: [u8; 9] = [0, 0xff, 0x55, 0, 0x11, 0, 0, 0, 0x01];

        // okay this depth up-scaling can be done in place
        // stb_image does it, it's a performance benefit to do it that way
        // but for GOD's sake, there are too many pointer arithmetic and implicit
        // things I cannot even begin to wrap my head on how to go about it
        //
        // So just allocate a new byte, write to that and set it to be
        // out later on
        let info = &self.png_info;
        let new_size = height * width * out_n;
        let mut new_out = vec![0; new_size];

        let mut in_offset = 0;

        let mut current = 0;

        for _ in 0..height
        {
            let scale = DEPTH_SCALE_TABLE[usize::from(info.depth)];

            let mut k = width * out_n;

            if info.depth == 1
            {
                while k >= 8
                {
                    let cur: &mut [u8; 8] = new_out
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
                        new_out[current] = scale * ((in_val >> shift) & 0x01);
                        current += 1;
                    }
                }
            }
            else if info.depth == 2
            {
                while k >= 4
                {
                    let cur: &mut [u8; 4] = new_out
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
                        new_out[current] = scale * ((in_val >> shift) & 0x03);
                        current += 1;
                    }
                }
            }
            else if info.depth == 4
            {
                while k >= 2
                {
                    let cur: &mut [u8; 2] = new_out
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
                        new_out[current] = scale * ((in_val >> shift) & 0x0f);
                        current += 1;
                    }
                }
            }
        }
        // ensure we are in the right position
        // assert_eq!(in_offset, image_length);
        // assert_eq!(current, image_length * usize::from(8 / info.depth));

        self.out = new_out;
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
        let size_hint = (self.png_info.width + 1) * self.png_info.height;

        let option = DeflateOptions::default()
            .set_size_hint(size_hint)
            .set_confirm_checksum(self.options.confirm_checksums);

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
        let data = &self.out;

        let info = self.png_info;
        let out_size = info.width * info.height * components;
        // this is safe because we resized palette to be 256
        // in self.parse_plte()
        let palette: &[PLTEEntry; 256] = &self.palette[0..256].try_into().unwrap();
        let mut out = vec![0; out_size];

        if components == 3
        {
            for (px, entry) in out.chunks_exact_mut(3).zip(data)
            {
                // the & 255 may be removed as the compiler can see u8 can never be
                // above 255, but for safety
                let entry = palette[usize::from(*entry) & 255];

                px[0] = entry.red;
                px[1] = entry.green;
                px[2] = entry.blue;
            }
        }
        else if components == 4
        {
            for (px, entry) in out.chunks_exact_mut(4).zip(data)
            {
                let entry = palette[usize::from(*entry) & 255];

                px[0] = entry.red;
                px[1] = entry.green;
                px[2] = entry.blue;
                px[3] = entry.alpha;
            }
        }
        self.out = out;
    }
}

// #[test]
// fn test_black_and_white()
// {
//     let path = "/home/caleb/jpeg/png/t.png";
//     let data = std::fs::read(path).unwrap();
//     let mut decoder = PngDecoder::new(&data);
//     let px = decoder.decode().unwrap();
// }
