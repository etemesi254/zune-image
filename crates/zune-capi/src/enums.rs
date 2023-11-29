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
pub enum ZImageFormat {
    /// Any unknown format
    UnknownFormat = 0,
    /// Joint Photographic Experts Group
    JPEG,
    /// Portable Network Graphics
    PNG,
    /// Portable Pixel Map image
    PPM,
    /// Photoshop PSD component
    PSD,
    /// Farbfeld format
    Farbfeld,
    /// Quite Okay Image
    QOI,
    /// JPEG XL, new format
    JPEG_XL,
    /// Radiance HDR decoder
    HDR,
    /// Windows Bitmap Files
    BMP,
}

impl From<ImageFormat> for ZImageFormat {
    fn from(value: ImageFormat) -> Self {
        match value {
            ImageFormat::JPEG => ZImageFormat::JPEG,
            ImageFormat::PNG => ZImageFormat::PNG,
            ImageFormat::PPM => ZImageFormat::PPM,
            ImageFormat::PSD => ZImageFormat::PSD,
            ImageFormat::Farbfeld => ZImageFormat::Farbfeld,
            ImageFormat::QOI => ZImageFormat::QOI,
            ImageFormat::JPEG_XL => ZImageFormat::JPEG_XL,
            ImageFormat::HDR => ZImageFormat::HDR,
            ImageFormat::BMP => ZImageFormat::BMP,
            _ => ZImageFormat::UnknownFormat,
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
pub enum ZImageDepth {
    /// Image depth is unknown
    UnknownDepth = 0,
    ///8-bit images
    U8 = 1,
    /// 16 bit images
    U16 = 2,
    /// Float 32 images   
    F32 = 4,
}
impl From<BitDepth> for ZImageDepth {
    fn from(value: BitDepth) -> Self {
        match value {
            BitDepth::Eight => ZImageDepth::U8,
            BitDepth::Sixteen => ZImageDepth::U16,
            BitDepth::Float32 => ZImageDepth::F32,
            _ => ZImageDepth::UnknownDepth,
        }
    }
}
#[derive(Copy, Clone)]
#[repr(C)]
pub enum ZImageColorspace {
    /// Unknown image colorspace
    UnknownColorspace = 0,
    /// Red, Green , Blue
    RGB,
    /// Red, Green, Blue, Alpha
    RGBA,
    /// YUV colorspace
    YCbCr,
    /// Grayscale colorspace
    Luma,
    /// Grayscale with alpha colorspace
    LumaA,
    YCCK,
    /// Cyan , Magenta, Yellow, Black
    CMYK,
    /// Blue, Green, Red
    BGR,
    /// Blue, Green, Red, Alpha
    BGRA,
}

impl From<ColorSpace> for ZImageColorspace {
    fn from(value: ColorSpace) -> Self {
        match value {
            ColorSpace::RGB => ZImageColorspace::RGB,
            ColorSpace::RGBA => ZImageColorspace::RGBA,
            ColorSpace::YCbCr => ZImageColorspace::YCbCr,
            ColorSpace::Luma => ZImageColorspace::Luma,
            ColorSpace::LumaA => ZImageColorspace::LumaA,
            ColorSpace::YCCK => ZImageColorspace::YCCK,
            ColorSpace::CMYK => ZImageColorspace::CMYK,
            ColorSpace::BGR => ZImageColorspace::BGR,
            ColorSpace::BGRA => ZImageColorspace::BGRA,
            _ => ZImageColorspace::UnknownColorspace,
        }
    }
}

///\brief Creates a new depth that can be passed to functions that require
/// depth
///
/// \returns ImageDepth with a value of ImageDepth::Unknown
#[no_mangle]
extern "C" fn zil_imdepth_new() -> ZImageDepth {
    ZImageDepth::UnknownDepth
}
