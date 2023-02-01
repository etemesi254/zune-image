//! Decoder and Encoder Options
//!
//! This module exposes a struct for which all implemented
//! decoders get shared options for decoding
//!
//! All supported options are put into one _Options to allow for global configurations
//! options e.g the same  `DecoderOption` can be reused for all other decoders
//!
pub use decoder::DecoderOptions;

use crate::bit_depth::BitDepth;
use crate::colorspace::ColorSpace;

mod decoder;

#[derive(Debug, Copy, Clone)]
pub struct EncoderOptions
{
    /// The image width
    ///
    /// This is the width the image will be encoded in
    ///
    ///
    /// # Note
    /// Images have different width and height limit,
    /// encoding an image larger than that limit is an error.
    ///
    /// Consult with your favourite image codec on its limit
    pub width:      usize,
    /// The image height
    ///
    /// This is the height the image will be encoded in.
    ///
    /// # Note
    /// Images have different width and height limit,
    /// encoding an image larger than that limit is an error.
    ///
    /// Consult with your favourite image codec on its limit
    pub height:     usize,
    /// The colorspace the pixels are in.
    ///
    /// # Note
    /// Each encoder has different set of supported colorspaces.
    ///
    /// Check with your favourite image codec on its limit.
    pub colorspace: ColorSpace,
    /// The quality expected for the image
    ///
    /// This has different results depending on encoder.
    ///
    /// - lossy encoders: Higher values, good visual quality/larger file, lower values bad visual quality/smaller file.
    /// - lossless encoders: Higher values, more encoding time/smaller image, lower value, less encoding time/image.
    ///
    ///
    /// Though this is not respected by some encoders, e.g `ppm` doesn't have a notion of quality.
    pub quality:    u8,
    /// The bit depth of the data
    ///
    ///
    /// The data is expected in native endian
    /// the encoder will convert the data to whatever endian
    /// is needed by the format.
    ///
    /// - Respected by: `png`,`ppm`
    pub depth:      BitDepth
}

impl Default for EncoderOptions
{
    fn default() -> Self
    {
        Self {
            width:      0,
            height:     0,
            colorspace: ColorSpace::RGB,
            quality:    100,
            depth:      BitDepth::Eight
        }
    }
}

impl EncoderOptions
{
    pub const fn get_width(&self) -> usize
    {
        self.width
    }

    pub const fn get_height(&self) -> usize
    {
        self.height
    }
    pub const fn get_depth(&self) -> BitDepth
    {
        self.depth
    }
    pub const fn get_quality(&self) -> u8
    {
        self.quality
    }
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.colorspace
    }
}
