use std::ffi::OsString;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use zune_image::metadata::ImageMetadata;

pub struct Metadata<'a>
{
    file:     OsString,
    metadata: &'a ImageMetadata
}

impl<'a> Metadata<'a>
{
    pub fn new(file: OsString, metadata: &ImageMetadata) -> Metadata
    {
        Metadata { file, metadata }
    }
}

impl<'a> Serialize for Metadata<'a>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut state = serializer.serialize_struct("ImageMetadata", 2)?;

        state.serialize_field("file", &self.file.to_string_lossy())?;
        state.serialize_field("metadata", &self.metadata)?;

        state.end()
    }
}
