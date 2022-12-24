use log::trace;
use zune_core::bytestream::ZByteReader;

use crate::error::PngErrors;

///
pub type UnkownChunkHandler = fn(
    length: usize,
    chunkType: [u8; 4],
    reader: &mut ZByteReader,
    crc: u32
) -> Result<(), PngErrors>;

fn default_chunk_handler(
    length: usize, chunk_type: [u8; 4], reader: &mut ZByteReader, _crc: u32
) -> Result<(), PngErrors>
{
    let chunk_name = std::str::from_utf8(&chunk_type).unwrap_or("XXXX");

    if chunk_type[0] & (1 << 5) == 0
    {
        return Err(PngErrors::Generic(format!(
            "Marker {chunk_name} unknown but deemed necessary",
        )));
    }

    trace!("Encountered unknown chunk {:?}", chunk_name);
    trace!("Length of chunk {}", length);
    trace!("Skipping {} bytes", length + 4);

    reader.skip(length + 4);

    Ok(())
}
#[derive(Copy, Clone)]
pub struct PngOptions
{
    pub(crate) max_width:     usize,
    pub(crate) max_height:    usize,
    pub(crate) chunk_handler: UnkownChunkHandler,
    pub(crate) _strict_mode:  bool,
    pub(crate) confirm_crc:   bool
}

impl Default for PngOptions
{
    fn default() -> Self
    {
        Self {
            max_width:     1 << 17,
            max_height:    1 << 17,
            chunk_handler: default_chunk_handler,
            _strict_mode:  false,
            confirm_crc:   true
        }
    }
}
