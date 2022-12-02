use std::fs::OpenOptions;
use std::io::Write;

use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;

use crate::constants::PNG_SIGNATURE;
use crate::enums::{FilterMethod, InterlaceMethod, PngChunkType, PngColor};
use crate::error::PngErrors;
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
    pub(crate) idat_chunks: Vec<&'a [u8]>
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

    pub fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if self.seen_hdr
        {
            return None;
        }

        Some((self.png_info.width, self.png_info.height))
    }
    pub fn get_colorspace(&self) -> ColorSpace
    {
        match self.png_info.color
        {
            PngColor::Palette => ColorSpace::RGB,
            PngColor::Luma => ColorSpace::Luma,
            PngColor::LumaA => ColorSpace::LumaA,
            PngColor::RGB => ColorSpace::RGB,
            PngColor::RGBA => ColorSpace::RGBA,
            PngColor::Unknown => todo!()
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
    pub fn decode(&mut self) -> Result<Vec<u8>, PngErrors>
    {
        let data = vec![];
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
        self.inflate();
        Ok(data)
    }
    /// Undo deflate decoding
    fn inflate(&mut self)
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
        // so choose point 2.
        //
        // So since we have IDAT chunks as references
        // we call deflate with those chunks until we have full
        // data, we can suspend
        let mut file = OpenOptions::new()
            .write(true)
            .open("/home/caleb/Documents/zune-image/zune-inflate/tests/tt.zlib")
            .unwrap();

        for chunk in &self.idat_chunks
        {
            file.write_all(chunk).unwrap();
        }
    }
}

#[test]
fn decode()
{
    let data = std::fs::read("/home/caleb/jpeg/cballs2.png").unwrap();
    let mut decoder = PngDecoder::new(&data);
    decoder.decode().unwrap();
}
