#![cfg(feature = "serde")]

use alloc::format;

use serde::ser::*;

use crate::bit_depth::BitDepth;
use crate::colorspace::{ColorCharacteristics, ColorSpace};

impl Serialize for ColorSpace
{
    #[allow(clippy::uninlined_format_args)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        // colorspace serialization is simply it's debug value
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl Serialize for BitDepth
{
    #[allow(clippy::uninlined_format_args)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl Serialize for ColorCharacteristics
{
    #[allow(clippy::uninlined_format_args)]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}
