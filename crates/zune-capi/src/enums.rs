use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_image::codecs::ImageFormat;

/// Various image formats that are supported by the library
/// in one way or another
///
/// Some of them have partial support, i.e there is only a decoder bundled in
/// while others have full support
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[repr(C)]
#[allow(non_camel_case_types)]
#[allow(clippy::upper_case_acronyms)]
pub enum ZImageFormat {
    /// Any unknown format
    ZilUnknownFormat = 0,
    /// Joint Photographic Experts Group
    ZilJPEG,
    /// Portable Network Graphics
    ZilPNG,
    /// Portable Pixel Map image
    ZilPPM,
    /// Photoshop PSD component
    ZilPSD,
    /// Farbfeld format
    ZilFarbfeld,
    /// Quite Okay Image
    ZilQOI,
    /// JPEG XL, new format
    ZilJPEG_XL,
    /// Radiance HDR decoder
    ZilHDR,
    /// Windows Bitmap Files
    ZilBMP
}

impl ZImageFormat {
    /// Convert back to rust image format
    pub fn to_format(self) -> ImageFormat {
        match self {
            ZImageFormat::ZilJPEG => ImageFormat::JPEG,
            ZImageFormat::ZilPNG => ImageFormat::PNG,
            ZImageFormat::ZilPPM => ImageFormat::PPM,
            ZImageFormat::ZilPSD => ImageFormat::PSD,
            ZImageFormat::ZilFarbfeld => ImageFormat::Farbfeld,
            ZImageFormat::ZilQOI => ImageFormat::QOI,
            ZImageFormat::ZilJPEG_XL => ImageFormat::JPEG_XL,
            ZImageFormat::ZilHDR => ImageFormat::HDR,
            ZImageFormat::ZilBMP => ImageFormat::BMP,
            _ => ImageFormat::Unknown
        }
    }
}
impl From<ImageFormat> for ZImageFormat {
    fn from(value: ImageFormat) -> Self {
        match value {
            ImageFormat::JPEG => ZImageFormat::ZilJPEG,
            ImageFormat::PNG => ZImageFormat::ZilPNG,
            ImageFormat::PPM => ZImageFormat::ZilPPM,
            ImageFormat::PSD => ZImageFormat::ZilPSD,
            ImageFormat::Farbfeld => ZImageFormat::ZilFarbfeld,
            ImageFormat::QOI => ZImageFormat::ZilQOI,
            ImageFormat::JPEG_XL => ZImageFormat::ZilJPEG_XL,
            ImageFormat::HDR => ZImageFormat::ZilHDR,
            ImageFormat::BMP => ZImageFormat::ZilBMP,
            _ => ZImageFormat::ZilUnknownFormat
        }
    }
}

/// Image depth information
///
/// This enum gives information about image depths
///
/// The image depths also give you the information for
/// which we represent image pixels
///
/// U8  -> 8 bit depth, image is represented as uint8_t (unsigned char)
/// U16 -> 16 bit depth, image is represented as uint16_t ( unsigned short)
/// F32  -> using float32, image is represented as float
#[repr(C)]
#[derive(Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum ZImageDepth {
    /// Image depth is unknown
    ZilUnknownDepth = 0,
    ///8-bit images
    ZilU8 = 1,
    /// 16 bit images
    ZilU16 = 2,
    /// Float 32 images   
    ZilF32 = 4
}

impl ZImageDepth {
    pub(crate) fn to_depth(self) -> BitDepth {
        match self {
            ZImageDepth::ZilUnknownDepth => BitDepth::Unknown,
            ZImageDepth::ZilU8 => BitDepth::Eight,
            ZImageDepth::ZilU16 => BitDepth::Sixteen,
            ZImageDepth::ZilF32 => BitDepth::Float32
        }
    }
}
impl From<BitDepth> for ZImageDepth {
    fn from(value: BitDepth) -> Self {
        match value {
            BitDepth::Eight => ZImageDepth::ZilU8,
            BitDepth::Sixteen => ZImageDepth::ZilU16,
            BitDepth::Float32 => ZImageDepth::ZilF32,
            _ => ZImageDepth::ZilUnknownDepth
        }
    }
}
#[derive(Copy, Clone)]
#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
pub enum ZImageColorspace {
    /// Unknown image colorspace
    ZilUnknownColorspace = 0,
    /// Red, Green , Blue
    ZilRGB,
    /// Red, Green, Blue, Alpha
    ZilRGBA,
    /// YUV colorspace
    ZilYCbCr,
    /// Grayscale colorspace
    ZilLuma,
    /// Grayscale with alpha colorspace
    ZilLumaA,
    ZilYCCK,
    /// Cyan , Magenta, Yellow, Black
    ZilCMYK,
    /// Blue, Green, Red
    ZilBGR,
    /// Blue, Green, Red, Alpha
    ZilBGRA,
    /// Alpha, Blue Green, Red
    ZilARGB,
    /// Hue, Saturation, Lightness,
    ZilHSL,
    /// Hue, Saturation,Variance
    ZilHSV
}

impl ZImageColorspace {
    pub fn to_colorspace(self) -> ColorSpace {
        match self {
            Self::ZilRGB => ColorSpace::RGB,
            Self::ZilRGBA => ColorSpace::RGBA,
            Self::ZilYCbCr => ColorSpace::YCbCr,
            Self::ZilLuma => ColorSpace::Luma,
            Self::ZilLumaA => ColorSpace::LumaA,
            Self::ZilYCCK => ColorSpace::YCCK,
            Self::ZilCMYK => ColorSpace::CMYK,
            Self::ZilBGR => ColorSpace::BGR,
            Self::ZilBGRA => ColorSpace::BGRA,
            Self::ZilARGB => ColorSpace::ARGB,
            Self::ZilHSL => ColorSpace::HSL,
            Self::ZilHSV => ColorSpace::HSV,
            Self::ZilUnknownColorspace => ColorSpace::Unknown
        }
    }
}
impl From<ColorSpace> for ZImageColorspace {
    fn from(value: ColorSpace) -> Self {
        // Remember to also do for to_colorspace
        match value {
            ColorSpace::RGB => ZImageColorspace::ZilRGB,
            ColorSpace::RGBA => ZImageColorspace::ZilRGBA,
            ColorSpace::YCbCr => ZImageColorspace::ZilYCbCr,
            ColorSpace::Luma => ZImageColorspace::ZilLuma,
            ColorSpace::LumaA => ZImageColorspace::ZilLumaA,
            ColorSpace::YCCK => ZImageColorspace::ZilYCCK,
            ColorSpace::CMYK => ZImageColorspace::ZilCMYK,
            ColorSpace::BGR => ZImageColorspace::ZilBGR,
            ColorSpace::BGRA => ZImageColorspace::ZilBGRA,
            ColorSpace::ARGB => ZImageColorspace::ZilARGB,
            ColorSpace::HSV => ZImageColorspace::ZilHSV,
            ColorSpace::HSL => ZImageColorspace::ZilHSL,
            _ => ZImageColorspace::ZilUnknownColorspace
        }
    }
}

///\brief Creates a new depth that can be passed to functions that require
/// depth
///
/// \returns ImageDepth with a value of ImageDepth::Unknown
#[no_mangle]
extern "C" fn zil_imdepth_new() -> ZImageDepth {
    ZImageDepth::ZilUnknownDepth
}
