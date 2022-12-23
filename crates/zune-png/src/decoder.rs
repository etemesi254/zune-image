use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::DecodingResult;

use crate::constants::PNG_SIGNATURE;
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngErrors;
use crate::filters::{handle_avg, handle_paeth, handle_sub, paeth};
use crate::options::PngOptions;

#[derive(Copy, Clone)]
pub(crate) struct PngChunk
{
    pub length:     usize,
    pub chunk_type: PngChunkType,
    pub chunk:      [u8; 4],
    pub crc:        u32
}

#[derive(Default, Debug)]
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
    pub(crate) idat_chunks: Vec<u8>
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
            idat_chunks: Vec::with_capacity(37) // randomly chosen size, my favourite number
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
        // go parse IDAT chunks
        let data = self.inflate()?;
        // now we have uncompressed data from zlib. Undo filtering

        // images with depth of 8, no interlace or filter can proceed to be returned
        if self.png_info.depth == 8
            && self.png_info.filter_method == FilterMethod::None
            && self.png_info.interlace_method == InterlaceMethod::Standard
        {
            return Ok(DecodingResult::U8(data));
        }

        Err(PngErrors::GenericStatic("Not yet done"))
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

        let deflate_data = decoder.decode_zlib().unwrap();

        let info = &self.png_info;

        let mut img_width_bytes;

        img_width_bytes = usize::from(info.component) * info.width;
        img_width_bytes *= usize::from(info.depth);
        img_width_bytes += 7;
        img_width_bytes >>= 3;

        let image_len = (img_width_bytes + 1) * info.height;

        if deflate_data.len() < image_len
        {
            let msg = format!(
                "Not enough pixels, expected {} but found {}",
                image_len,
                deflate_data.len()
            );
            return Err(PngErrors::Generic(msg));
        }
        let mut out = vec![0; image_len];
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
        chunk_size = info.width * usize::from(info.color.num_components());
        // add depth, and
        chunk_size *= usize::from(info.depth);
        chunk_size /= 8;
        // filter type
        chunk_size += 1;

        let mut chunks = deflate_data.chunks_exact(chunk_size);

        let filter_byte = deflate_data[0];
        // handle first loop explicitly
        // side effect is to ensure filter is always known
        let filter = FilterMethod::from_int(filter_byte)
            .ok_or_else(|| PngErrors::Generic(format!("Unknown filter {filter_byte}")))?;

        let first_scanline = chunks.next().unwrap();

        match filter
        {
            FilterMethod::None | FilterMethod::Up =>
            {
                out[0..chunk_size - 1].copy_from_slice(&first_scanline[1..]);
            }
            FilterMethod::Sub =>
            {
                let mut max_recon: [u8; 4] = [0; 4];

                for (filt, recon_x) in first_scanline[1..chunk_size]
                    .chunks(components)
                    .zip(out.chunks_exact_mut(components))
                {
                    for ((recon_a, filt_x), recon_v) in max_recon.iter_mut().zip(filt).zip(recon_x)
                    {
                        *recon_v = (*filt_x).wrapping_add(*recon_a);
                        *recon_a = *recon_v;
                    }
                }
            }
            FilterMethod::Average =>
            {
                let mut max_recon: [u8; 4] = [0; 4];
                // handle leftmost byte explicitly
                for i in 0..components
                {
                    out[i] = first_scanline[i + 1];
                    max_recon[i] = out[i];
                }
                for (filt, recon_x) in first_scanline[1 + components..]
                    .chunks(components)
                    .zip(out[components..].chunks_exact_mut(components))
                {
                    for ((recon_a, filt_x), recon_v) in max_recon.iter_mut().zip(filt).zip(recon_x)
                    {
                        let recon_x = *recon_a >> 1;

                        *recon_v = (*filt_x).wrapping_add(recon_x);
                        *recon_a = *recon_v;
                    }
                }
            }
            FilterMethod::Paeth =>
            {
                let mut max_recon: [u8; 4] = [0; 4];
                // handle leftmost byte explicitly
                for i in 0..components
                {
                    out[i] = first_scanline[i + 1];
                    max_recon[i] = out[i];
                }

                for (filt, out_px) in first_scanline[1 + components..]
                    .chunks(components)
                    .zip(out[components..].chunks_exact_mut(components))
                {
                    for ((recon_a, filt_x), out_p) in max_recon.iter_mut().zip(filt).zip(out_px)
                    {
                        let paeth_res = paeth(*recon_a, 0, 0);

                        *out_p = (*filt_x).wrapping_add(paeth_res);
                        *recon_a = *out_p;
                    }
                }
            }
            _ => unreachable!()
        }

        // so now we have the first row/scanline copied, we become a little fancy
        //
        //
        // ┌─────┬─────┐
        // │ c   │  b  │
        // ├─────┼─────┤
        // │ a   │ x   │
        // └─────┴─────┘
        //
        // This is the loop filter,
        // the only trick we have is how we do get the top row
        //
        // Complicated filters are handled in filters.rs

        let mut prev_row_start = 0;
        let width_stride = chunk_size - 1;

        let mut out_position = width_stride;

        for in_stride in chunks
        {
            // Split output into current and previous
            let (prev, current) = out.split_at_mut(out_position);
            // get the previous row.
            let prev_row = &prev[prev_row_start..prev_row_start + width_stride];

            prev_row_start += width_stride;
            out_position += width_stride;
            // take filter
            let filter_byte = in_stride[0];
            let raw = &in_stride[1..];
            // get it's type
            let filter = FilterMethod::from_int(filter_byte)
                .ok_or_else(|| PngErrors::Generic(format!("Unknown filter {filter_byte}")))?;

            assert_eq!(prev_row.len(), in_stride.len() - 1);

            match filter
            {
                FilterMethod::None =>
                {
                    // memcpy
                    current[0..width_stride].copy_from_slice(raw);
                }
                FilterMethod::Average =>
                {
                    handle_avg(prev_row, raw, current, components);
                }
                FilterMethod::Sub =>
                {
                    handle_sub(raw, current, components);
                }
                FilterMethod::Up =>
                {
                    for ((filt, recon), up) in raw.iter().zip(current).zip(prev_row)
                    {
                        *recon = (*filt).wrapping_add(*up)
                    }
                }
                FilterMethod::Paeth =>
                {
                    handle_paeth(prev_row, &in_stride[1..], current, components);
                }
                FilterMethod::Unkwown =>
                {
                    unreachable!()
                }
            }
        }

        // trim out
        let new_len = info.width * info.height * usize::from(info.color.num_components());

        out.truncate(new_len);

        Ok(out)
    }
}
