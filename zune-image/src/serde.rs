#![cfg(feature = "serde-support")]

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::codecs::ImageFormat;
use crate::metadata::ImageMetadata;

impl Serialize for ImageMetadata
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        const STRUCT_FIELDS: usize = 7;
        let mut state = serializer.serialize_struct("Metadata", STRUCT_FIELDS)?;

        state.serialize_field("width", &self.width)?;
        state.serialize_field("height", &self.height)?;
        state.serialize_field("colorspace", &self.colorspace)?;
        state.serialize_field("depth", &self.depth)?;
        state.serialize_field("format", &self.format)?;
        state.serialize_field("color_transfer_characteristics", &self.color_trc)?;
        state.serialize_field("gamma_value", &self.default_gamma)?;

        state.end()
    }
}

impl Serialize for ImageFormat
{
    #[allow(clippy::uninlined_format_args)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}
