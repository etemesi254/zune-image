use alloc::format;

use log::trace;
use zune_core::bytestream::ZByteReader;

use crate::error::PngDecodeErrors;

///
pub type UnkownChunkHandler = fn(
    length: usize,
    chunk_type: [u8; 4],
    reader: &mut ZByteReader,
    crc: u32
) -> Result<(), PngDecodeErrors>;

pub fn default_chunk_handler(
    length: usize, chunk_type: [u8; 4], reader: &mut ZByteReader, _crc: u32
) -> Result<(), PngDecodeErrors>
{
    let chunk_name = core::str::from_utf8(&chunk_type).unwrap_or("XXXX");

    if chunk_type[0] & (1 << 5) == 0
    {
        return Err(PngDecodeErrors::Generic(format!(
            "Marker {chunk_name} unknown but deemed necessary",
        )));
    }

    trace!("Encountered unknown chunk {:?}", chunk_name);
    trace!("Length of chunk {}", length);
    trace!("Skipping {} bytes", length + 4);

    reader.skip(length + 4);

    Ok(())
}
