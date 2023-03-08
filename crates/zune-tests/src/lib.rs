#![allow(unused_imports)]

use std::fs::read;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use xxhash_rust::xxh3::xxh3_128;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

mod jpeg;
mod png;
mod psd;

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsonColorspace
{
    RGB,
    RGBA,
    CMYK,
    YCbCr,
    Luma,
    YCCK
}

impl JsonColorspace
{
    pub fn to_colorspace(self) -> ColorSpace
    {
        match self
        {
            Self::CMYK => ColorSpace::CMYK,
            Self::YCCK => ColorSpace::YCCK,
            Self::RGB => ColorSpace::RGB,
            Self::YCbCr => ColorSpace::YCbCr,
            Self::Luma => ColorSpace::Luma,
            Self::RGBA => ColorSpace::RGBA
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct TestEntry
{
    pub name:       String,
    pub hash:       u128,
    pub colorspace: Option<JsonColorspace>,
    pub comment:    Option<String>
}

pub fn sample_path() -> PathBuf
{
    let path = Path::new(env!("CARGO_MANIFEST_DIR"));
    // get parent path
    path.parent().unwrap().to_owned()
}

fn hash(contents: &[u8]) -> u128
{
    xxh3_128(contents)
}
