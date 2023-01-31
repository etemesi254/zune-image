//! Decoder and Encoder Options
//!
//! This module exposes a struct for which all implemented
//! decoders get shared options for decoding
//!
//! All supported options are put into one _Options to allow for global configurations
//! options e.g the same  `DecoderOption` can be reused for all other decoders
//!

use crate::bit_depth::BitDepth;
use crate::colorspace::ColorSpace;

/// Decoder options
///
/// Not all options are respected by decoders
/// Each option specifies decoders that respect it
///
/// To remove the annoyance of getters and setters
/// all exposed options are declared public.
#[derive(Debug, Copy, Clone)]
pub struct DecoderOptions
{
    /// Maximum width for which decoders will
    /// not try to decode images larger than
    /// the specified width.
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    pub max_width:      usize,
    /// Maximum height for which decoders will not
    /// try to decode images larger than the
    /// specified height
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    pub max_height:     usize,
    ///  Whether the routines can use unsafe platform specific
    /// intrinsics when necessary
    ///
    /// Platform intrinsics are implemented for operations which
    /// the compiler can't auto-vectorize, or we can do a marginably
    /// better job at it
    ///
    /// All decoders with unsafe routines respect it.
    ///
    /// Treat this with caution, disabling it will cause slowdowns but
    /// it's provided for mainly for debugging use.
    ///
    /// -Default value : true
    /// - Respected by: `png` and `jpeg`(decoders with unsafe routines)
    pub use_unsafe:     bool,
    /// treat some warnings as errors
    ///
    /// Some images may have recoverable errors
    /// but sometimes decoders may wish to have a more standard
    /// conforming decoder which would error out on encountering such images
    ///
    /// When set to false, this logs errors via the log crate.
    ///
    /// When set to true, this will return an `Result<Err>` on exception.
    ///
    /// - Default value: false,
    /// - Respected by: `jpeg`, `png`
    pub strict_mode:    bool,
    /// Output colorspace
    ///
    /// The jpeg decoder allows conversion to a separate colorspace
    /// than the input.
    ///
    /// I.e you can convert a RGB jpeg image to grayscale without
    /// first decoding it to RGB to get
    ///
    /// - Default value: `ColorSpace::RGB`
    /// - Respected by: `jpeg`
    pub out_colorspace: ColorSpace,

    /// Maximum number of scans allowed
    /// for progressive jpeg images
    ///
    /// Progressive jpegs have scans
    ///
    /// - Default value:100
    /// - Respected by: `jpeg`
    pub max_scans: usize
}

impl DecoderOptions
{
    /// Get maximum width supported for the decoder
    pub const fn get_max_width() {}
}

impl Default for DecoderOptions
{
    fn default() -> Self
    {
        Self {
            out_colorspace: ColorSpace::RGB,
            max_width:      1 << 14,
            max_height:     1 << 14,
            use_unsafe:     true,
            max_scans:      100,
            strict_mode:    false
        }
    }
}

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
