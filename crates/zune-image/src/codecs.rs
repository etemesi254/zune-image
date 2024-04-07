/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Entry point for all supported codecs the library understands
//!
//! The codecs here can be enabled and disabled at will depending on the configured interface,
//! it is recommended that you enable encoders and decoders that you only use
//!
//!
//! # Note on Compatibility with images
//!
//! - The library automatically tries to convert the image with highest compatibility
//!  this means that it will automatically convert the image to a supported bit depth
//! and supported colorspace in case the image is not in the supported colorspace
//!   E.g if you open a HDR or EXR image whose format is `f32` `[0.0-1.0]` and convert it to JPEG,
//! which understands 8 bit images`[0-255]`, the library will internally convert it to 8 bit images
//!
//! - For image depth, we convert it to the most appropriate depth, e.g trying to store F32 images in png
//! will convert it to 16 bit images, this allows us to preserve as much information as possible
//! during the conversation.
//!
//! - **Warning**: For this to work, the image will be cloned and the depth or colorspace modified on the
//!  clone. The current image is left as is, unmodified.
//!  **This may cause huge memory usage as cloning is expensive**
//!
//!
#![allow(unused_imports, unused_variables, non_camel_case_types, dead_code)]

use std::io::Cursor;
use std::path::Path;

use zune_core::bytestream::{ZByteReaderTrait, ZByteWriterTrait, ZCursor, ZReader};
use zune_core::log::trace;
use zune_core::options::{DecoderOptions, EncoderOptions};

use crate::codecs;
use crate::errors::ImgEncodeErrors::ImageEncodeErrors;
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::traits::{DecoderTrait, EncoderTrait};

pub mod bmp;
mod exr;
pub mod farbfeld;
pub mod hdr;
pub mod jpeg;
pub mod jpeg_xl;
pub mod png;
pub mod ppm;
pub mod psd;
pub mod qoi;
pub(crate) fn create_options_for_encoder(
    options: Option<EncoderOptions>, image: &Image
) -> EncoderOptions {
    // choose if we take options from pre-configured , or we create default options
    let start_options = if let Some(configured_opts) = options {
        configured_opts
    } else {
        EncoderOptions::default()
    };
    let (width, height) = image.dimensions();
    // then set image configuration
    start_options
        .set_width(width)
        .set_height(height)
        .set_depth(image.depth())
        .set_colorspace(image.colorspace())
}
/// All supported image formats
///
/// This enum contains supported image formats, either
/// encoders or decoders for a particular image
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageFormat {
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
    /// Any unknown format
    Unknown
}

impl ImageFormat {
    pub fn has_decoder(self) -> bool {
        #[cfg(feature = "jpeg-xl")]
        {
            // for jpeg-xl we  know we have a decoder when the header can be parsed
            // but here, we aren't passing any data below, and since we are using is_ok()
            // to determine if we have okay, the jxl one will always fail.
            //
            // So we lift the check out of the decoder and do it here
            if self == ImageFormat::JPEG_XL {
                return true;
            }
        }
        return self.decoder(ZCursor::new(&[])).is_ok();
    }
    pub fn decoder<'a, T>(&self, data: T) -> Result<Box<dyn DecoderTrait + 'a>, ImageErrors>
    where
        T: ZByteReaderTrait + 'a
    {
        self.decoder_with_options(data, DecoderOptions::default())
    }

    pub fn decoder_with_options<'a, T>(
        &self, data: T, options: DecoderOptions
    ) -> Result<Box<dyn DecoderTrait + 'a>, ImageErrors>
    where
        T: ZByteReaderTrait + 'a
    {
        match self {
            ImageFormat::JPEG => {
                #[cfg(feature = "jpeg")]
                {
                    Ok(Box::new(zune_jpeg::JpegDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }

            ImageFormat::PNG => {
                #[cfg(feature = "png")]
                {
                    Ok(Box::new(zune_png::PngDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "png"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::PPM => {
                #[cfg(feature = "ppm")]
                {
                    Ok(Box::new(zune_ppm::PPMDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::PSD => {
                #[cfg(feature = "psd")]
                {
                    Ok(Box::new(zune_psd::PSDDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "psd"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }

            ImageFormat::Farbfeld => {
                #[cfg(feature = "farbfeld")]
                {
                    Ok(Box::new(zune_farbfeld::FarbFeldDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "farbfeld"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }

            ImageFormat::QOI => {
                #[cfg(feature = "qoi")]
                {
                    Ok(Box::new(zune_qoi::QoiDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "qoi"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::HDR => {
                #[cfg(feature = "hdr")]
                {
                    Ok(Box::new(zune_hdr::HdrDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "hdr"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::BMP => {
                #[cfg(feature = "bmp")]
                {
                    Ok(Box::new(zune_bmp::BmpDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "bmp"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::JPEG_XL => {
                #[cfg(feature = "jpeg-xl")]
                {
                    // use a ZByteReader which implements read, this prevents unnecessary
                    // copy

                    let reader = ZReader::new(data);
                    Ok(Box::new(codecs::jpeg_xl::JxlDecoder::try_new(
                        reader, options
                    )?))
                }
                #[cfg(not(feature = "jpeg-xl"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::Unknown => Err(ImageErrors::ImageDecoderNotImplemented(*self))
        }
    }
    /// Return true if an image format has an encoder that can convert the image
    /// into that format
    pub fn has_encoder(&self) -> bool {
        // if the feature is included, means we have an encoder
        #[allow(clippy::match_like_matches_macro)]
        match self {
            ImageFormat::JPEG => cfg!(feature = "jpeg"),
            ImageFormat::PNG => cfg!(feature = "png"),
            ImageFormat::PPM => cfg!(feature = "ppm"),
            ImageFormat::Farbfeld => cfg!(feature = "farbfeld"),
            ImageFormat::QOI => cfg!(feature = "qoi"),
            ImageFormat::JPEG_XL => cfg!(feature = "jpeg-xl"),
            ImageFormat::HDR => cfg!(feature = "hdr"),
            _ => false
        }
    }
    pub fn encode<T: ZByteWriterTrait>(
        &self, image: &Image, encoder_options: EncoderOptions, sink: T
    ) -> Result<usize, ImageErrors> {
        match self {
            ImageFormat::JPEG => {
                #[cfg(feature = "jpeg")]
                {
                    let mut encoder = codecs::jpeg::JpegEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::PNG => {
                #[cfg(feature = "png")]
                {
                    let mut encoder = codecs::png::PngEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::PPM => {
                #[cfg(feature = "ppm")]
                {
                    let mut encoder = codecs::ppm::PPMEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::Farbfeld => {
                #[cfg(feature = "farbfeld")]
                {
                    let mut encoder =
                        codecs::farbfeld::FarbFeldEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::QOI => {
                #[cfg(feature = "qoi")]
                {
                    let mut encoder = codecs::qoi::QoiEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::JPEG_XL => {
                #[cfg(feature = "jpeg-xl")]
                {
                    let mut encoder =
                        codecs::jpeg_xl::JxlEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            ImageFormat::HDR => {
                #[cfg(feature = "hdr")]
                {
                    let mut encoder = codecs::hdr::HdrEncoder::new_with_options(encoder_options);
                    return encoder.encode(image, sink);
                }
            }
            _ => {}
        }
        Err(ImageErrors::EncodeErrors(
            ImgEncodeErrors::NoEncoderForFormat(*self)
        ))
    }

    pub fn guess_format<T>(bytes: T) -> Option<(ImageFormat, T)>
    where
        T: ZByteReaderTrait
    {
        guess_format(bytes)
    }

    pub fn encoder_for_extension<P: AsRef<str>>(extension: P) -> Option<ImageFormat> {
        match extension.as_ref() {
            "qoi" => {
                #[cfg(feature = "qoi")]
                {
                    Some(ImageFormat::QOI)
                }
                #[cfg(not(feature = "qoi"))]
                {
                    None
                }
            }
            "ppm" | "pam" | "pgm" | "pbm" | "pfm" => {
                #[cfg(feature = "ppm")]
                {
                    Some(ImageFormat::PPM)
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            "jpeg" | "jpg" => {
                #[cfg(feature = "jpeg")]
                {
                    Some(ImageFormat::JPEG)
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    None
                }
            }
            "jxl" => {
                #[cfg(feature = "jpeg-xl")]
                {
                    Some(ImageFormat::JPEG_XL)
                }
                #[cfg(not(feature = "jpeg-xl"))]
                {
                    None
                }
            }
            "ff" => {
                #[cfg(feature = "farbfeld")]
                {
                    Some(ImageFormat::Farbfeld)
                }
                #[cfg(not(feature = "farbfeld"))]
                {
                    None
                }
            }
            "hdr" => {
                #[cfg(feature = "hdr")]
                {
                    Some(ImageFormat::HDR)
                }
                #[cfg(not(feature = "hdr"))]
                {
                    None
                }
            }
            "png" => {
                #[cfg(feature = "png")]
                {
                    Some(ImageFormat::PNG)
                }
                #[cfg(not(feature = "png"))]
                {
                    None
                }
            }
            _ => None
        }
    }
}

// save options
impl Image {
    /// Save the image to a file and use the extension to
    /// determine the format
    ///
    /// If the extension cannot be determined from the path, it's an error.
    ///
    /// # Arguments
    ///
    /// * `file`: The file to save the image to
    ///
    /// returns: Result<(), ImageErrors>
    ///
    /// # Examples
    ///
    ///  - Encode image to jpeg format, requires `jpeg` feature
    ///
    /// ```no_run
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::image::Image;
    /// // create a luma image
    /// let image = Image::fill::<u8>(128,ColorSpace::Luma,100,100);
    /// // save to jpeg
    /// image.save("hello.jpg").unwrap();
    /// ```
    pub fn save<P: AsRef<Path>>(&self, file: P) -> Result<(), ImageErrors> {
        return if let Some(ext) = file.as_ref().extension() {
            if let Some(format) = ImageFormat::encoder_for_extension(ext.to_string_lossy()) {
                self.save_to(file, format)
            } else {
                let msg = format!("No encoder for extension {ext:?}");

                Err(ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(msg)))
            }
        } else {
            let msg = format!("No extension for file {:?}", file.as_ref());

            Err(ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(msg)))
        };
    }
    /// Save an image using a specified format to a file
    ///
    /// The image may be cloned and the clone modified to fit preferences
    /// for that specific image format, e.g if the image is in f32 and being saved
    /// to jpeg, the clone will be modified to be in u8, and that will be the format
    /// saved to jpeg.
    ///
    /// # Arguments
    ///
    /// * `file`: The file path to which the image will be saved
    /// * `format`: The format to save the image into. It's an error if the
    ///     format doesn't have an encoder(not all formats do)
    ///
    /// returns: Result<(), ImageErrors>
    ///
    ///
    /// # Examples
    ///
    ///  - Save a black grayscale image to JPEG, requires the JPEG feature
    /// ```no_run
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::codecs::ImageFormat;
    /// use zune_image::errors::ImageErrors;
    /// use zune_image::image::Image;
    /// fn main()->Result<(),ImageErrors>{
    ///     // create a simple 200x200 grayscale image consisting of pure black
    ///     let image = Image::fill::<u8>(0,ColorSpace::Luma,200,200);
    ///     // save that to jpeg
    ///     #[cfg(feature = "jpeg")]
    ///     {
    ///         image.save_to("black.jpg",ImageFormat::JPEG)?;
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn save_to<P: AsRef<Path>>(&self, file: P, format: ImageFormat) -> Result<(), ImageErrors> {
        // open a file for which we will write directly to
        let mut file = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(file)?
        );
        self.encode(format, &mut file)?;
        Ok(())
    }

    /// Encode an image returning a vector containing the result
    /// of the encoding
    ///
    /// # Arguments
    ///
    /// * `format`: The format to use for encoding, it's an error if the
    /// relevant encoder is not present either because it's not supported, or it's not
    /// included as a feature.
    ///
    /// returns: `Result<Vec<u8, Global>, ImageErrors>`
    ///
    /// # Examples
    ///
    /// - Encode a simple image to QOI format, needs qoi format to be enabled
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::codecs::ImageFormat;
    /// use zune_image::image::Image;
    /// // create an image using from fn, to generate a gradient image
    /// let image = Image::from_fn::<u8,_>(300,300,ColorSpace::RGB,|x,y,px|{
    ///         let r = (0.3 * x as f32) as u8;
    ///         let b = (0.3 * y as f32) as u8;
    ///         px[0] = r;
    ///         px[2] = b;
    /// });
    /// // write to qoi now
    /// let contents = image.write_to_vec(ImageFormat::QOI).unwrap();
    /// ```
    pub fn write_to_vec(&self, format: ImageFormat) -> Result<Vec<u8>, ImageErrors> {
        if format.has_encoder() {
            let mut sink = vec![];
            self.encode(format, &mut sink)?;
            Ok(sink)
            // encode
        } else {
            Err(ImageErrors::EncodeErrors(
                crate::errors::ImgEncodeErrors::NoEncoderForFormat(format)
            ))
        }
    }

    /// Write data to a sink using a custom encoder returning how many bytes were written if successful
    ///
    /// # Arguments
    ///
    /// * `encoder`: The encoder to use for encoding
    /// * `sink`: Where does output data goes
    ///
    /// returns: `Result<usize, ImageErrors>`
    ///
    /// # Examples
    ///
    /// - Encode a simple image to JPEG format
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// // requires jpeg feature
    /// use zune_image::codecs::jpeg::JpegEncoder;
    /// use zune_image::image::Image;
    ///
    /// let encoder = JpegEncoder::new();
    ///
    /// // create an image using from fn, to generate a gradient image
    /// let image = Image::from_fn::<u8,_>(300,300,ColorSpace::RGB,|x,y,px|{
    ///         let r = (0.3 * x as f32) as u8;
    ///         let b = (0.3 * y as f32) as u8;
    ///         px[0] = r;
    ///         px[2] = b;
    /// });
    ///
    /// let mut output = vec![];
    ///
    /// // write to jpeg now
    /// let contents = image.write_with_encoder(encoder, &mut output).unwrap();
    /// ```
    pub fn write_with_encoder<T: ZByteWriterTrait>(
        &self, mut encoder: impl EncoderTrait, sink: T
    ) -> Result<usize, ImageErrors> {
        encoder.encode(self, sink)
    }

    /// Open an encoded file for which the library has a configured decoder for it
    ///
    ///
    /// - The decoders supported can be switched on and off depending on how
    ///  you configure your Cargo.toml. It is generally recommended to not enable decoders
    ///  you will not be using as it reduces both the security attack surface and dependency
    ///
    /// # Arguments
    /// - file: The file path from which to read the file from, the file must be a supported format
    /// otherwise it's an error to try and decode
    ///
    /// See also [read](Self::read) for reading from memory
    pub fn open<P: AsRef<Path>>(file: P) -> Result<Image, ImageErrors> {
        Self::open_with_options(file, DecoderOptions::default())
    }

    /// Open an encoded file for which the library has a configured decoder for it
    /// with the specified custom decoder options
    ///
    /// This allows you to modify decoding steps  where possible by specifying how the
    /// decoder should behave
    ///
    /// # Example
    ///  -  Decode a file with strict mode enabled and only expect images with less
    ///  than 100 pixels in width
    ///
    /// ```no_run
    /// use zune_core::options::DecoderOptions;
    /// use zune_image::image::Image;
    /// let options = DecoderOptions::default().set_strict_mode(true).set_max_width(100);
    /// let image = Image::open_with_options("/a/file.jpeg",options).unwrap();
    /// ```
    pub fn open_with_options<P: AsRef<Path>>(
        file: P, options: DecoderOptions
    ) -> Result<Image, ImageErrors> {
        let reader = std::io::BufReader::new(std::fs::File::open(file)?);
        Self::read(reader, options)
    }
    /// Open a new file from memory with the configured options
    ///  
    /// # Arguments
    ///  - `src`: The encoded buffer loaded into memory
    ///  - `options`: The configured decoded options
    /// # Example
    /// - Open a memory source with the default options
    ///
    ///```no_run
    /// use zune_core::bytestream::ZCursor;
    /// use zune_core::options::DecoderOptions;
    /// use zune_image::image::Image;
    /// // create a simple ppm p5 grayscale format
    /// let image = Image::read(ZCursor::new(b"P5 1 1 255 1"),DecoderOptions::default());
    ///```
    pub fn read<T>(src: T, options: DecoderOptions) -> Result<Image, ImageErrors>
    where
        T: ZByteReaderTrait
    {
        let decoder = ImageFormat::guess_format(src);

        if let Some(format) = decoder {
            let mut image_decoder = format.0.decoder_with_options(format.1, options)?;
            // save format
            let mut image = image_decoder.decode()?;
            image.metadata.format = Some(format.0);
            Ok(image)
        } else {
            Err(ImageErrors::ImageDecoderNotImplemented(
                ImageFormat::Unknown
            ))
        }
    }

    /// Encode to a generic sink an image of a specific format
    ///
    /// # Arguments
    ///
    ///  - format: The image format to write to sink
    ///  - sink: An encapsulation of where we will be sending data to
    ///
    /// # Returns
    ///  - The size of bytes written to sink or an error if it occurs
    ///
    pub fn encode<T: ZByteWriterTrait>(
        &self, format: ImageFormat, sink: T
    ) -> Result<usize, ImageErrors> {
        self.encode_with_options(format, EncoderOptions::default(), sink)
    }
    /// Encode to a generic sink an image of a specific format
    ///
    /// # Arguments
    ///
    ///  - format: The image format to write to sink
    ///  - sink: An encapsulation of where we will be sending data to
    ///  - encoder_options: Custom options used for encoding
    /// # Returns
    ///  - The size of bytes written to sink or an error if it occurs
    ///
    fn encode_with_options<T: ZByteWriterTrait>(
        &self, format: ImageFormat, encoder_options: EncoderOptions, sink: T
    ) -> Result<usize, ImageErrors> {
        format.encode(self, encoder_options, sink)
    }

    /// Open a new file from memory with the configured decoder
    ///  
    /// # Arguments
    ///  - `decoder`: The configured decoder
    /// # Example
    /// - Open a memory source with the ppm decoder
    ///
    ///```no_run
    /// // requires ppm feature
    /// use std::io::Cursor;
    /// use zune_image::codecs::ppm::PPMDecoder;
    /// use zune_image::image::Image;
    ///
    /// // create a simple ppm p5 grayscale format
    /// let decoder = PPMDecoder::new(Cursor::new(b"P5 1 1 255 1"));
    ///
    /// let image = Image::from_decoder(decoder);
    ///```
    pub fn from_decoder(mut decoder: impl DecoderTrait) -> Result<Image, ImageErrors> {
        decoder.decode()
    }
}
/// Guess the format of an image based on it's magic bytes
///
/// # Arguments
/// - bytes: The data source containing the image pixels
///
/// # Returns
/// - Some(format,T): The image format and the data source.
/// - None: Indicates the format isn't known/understood by the library
pub fn guess_format<T>(bytes: T) -> Option<(ImageFormat, T)>
where
    T: ZByteReaderTrait
{
    let mut reader = ZReader::new(bytes);
    // stolen from imagers
    let magic_bytes: Vec<(&[u8], ImageFormat)> = vec![
        (&[137, 80, 78, 71, 13, 10, 26, 10], ImageFormat::PNG),
        // Of course with jpg we need to relax our definition of what is a jpeg
        // the best identifier would be 0xFF,0xd8 0xff but nop, some images exist
        // which do not have that
        (&[0xff, 0xd8], ImageFormat::JPEG),
        (b"P5", ImageFormat::PPM),
        (b"P6", ImageFormat::PPM),
        (b"P7", ImageFormat::PPM),
        (b"Pf", ImageFormat::PPM),
        (b"PF", ImageFormat::PPM),
        (b"8BPS", ImageFormat::PSD),
        (b"farbfeld", ImageFormat::Farbfeld),
        (b"qoif", ImageFormat::QOI),
        (b"#?RADIANCE\n", ImageFormat::HDR),
        (b"#?RGBE\n", ImageFormat::HDR),
        (
            &[
                0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A
            ],
            ImageFormat::JPEG_XL
        ),
        (&[0xFF, 0x0A], ImageFormat::JPEG_XL),
    ];

    for (magic, decoder) in magic_bytes {
        if reader.peek_at(0, magic.len()).ok()?.starts_with(magic) {
            return Some((decoder, reader.consume()));
        }
    }
    #[cfg(feature = "bmp")]
    {
        // get a slice reference
        // bmp requires 15 bytes to determine if it is a valid one.
        // so take 16 just to be safe

        let reference = reader.peek_at(0, 16).ok()?;

        if zune_bmp::probe_bmp(reference) {
            return Some((ImageFormat::BMP, reader.consume()));
        }
    }

    None
}
