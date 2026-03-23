use zune_core::bytestream::{ZByteReaderTrait, ZReader};

use crate::bmf_reader::FourCC;
use crate::errors::HeicErrors;

pub(crate) fn read_sized_int<R: ZByteReaderTrait>(
    reader: &mut ZReader<R>, size: u8
) -> Result<u64, HeicErrors> {
    match size {
        0 => Ok(0),
        4 => Ok(reader.get_u32_be_err()? as u64),
        8 => Ok(reader.get_u64_be_err()?),
        _ => Err(HeicErrors::ParseError {
            box_type: FourCC(*b"iloc"),
            msg:      format!("Unsupported variable integer size: {}", size)
        })
    }
}
