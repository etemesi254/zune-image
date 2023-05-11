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
//!
//! E.g if you open a HDR or EXR image whose format is `f32``[0.0-1.0]` and convert it to JPEG,
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
#![allow(unused_imports, unused_variables, non_camel_case_types)]

use std::path::Path;

use log::trace;
use zune_core::options::{DecoderOptions, EncoderOptions};

use crate::codecs;
use crate::errors::ImgEncodeErrors::ImageEncodeErrors;
use crate::errors::{ImageErrors, ImgEncodeErrors};
use crate::image::Image;
use crate::traits::{DecoderTrait, EncoderTrait};

pub mod bmp;
pub mod exr;
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
) -> EncoderOptions
{
    // choose if we take options from pre-configured , or we create default options
    let start_options = if let Some(configured_opts) = options
    {
        configured_opts
    }
    else
    {
        EncoderOptions::default()
    };
    let (width, height) = image.get_dimensions();
    // then set image configuration
    start_options
        .set_width(width)
        .set_height(height)
        .set_depth(image.get_depth())
        .set_colorspace(image.get_colorspace())
}
/// All supported image formats
///
/// This enum contains supported image formats, either
/// encoders or decoders for a particular image
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ImageFormat
{
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

impl ImageFormat
{
    /// Return true if an image format has an encoder that can convert the image
    /// into that format
    pub fn has_encoder(self) -> bool
    {
        return self.get_encoder().is_some();
    }

    pub fn has_decoder(self) -> bool
    {
        return self.get_decoder(&[]).is_ok();
    }
    pub fn get_decoder<'a>(&self, data: &'a [u8])
        -> Result<Box<dyn DecoderTrait + 'a>, ImageErrors>
    {
        self.get_decoder_with_options(data, DecoderOptions::default())
    }

    pub fn get_decoder_with_options<'a>(
        &self, data: &'a [u8], options: DecoderOptions
    ) -> Result<Box<dyn DecoderTrait + 'a>, ImageErrors>
    {
        match self
        {
            ImageFormat::JPEG =>
            {
                #[cfg(feature = "jpeg")]
                {
                    Ok(Box::new(zune_jpeg::JpegDecoder::new_with_options(
                        options, data
                    )))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }

            ImageFormat::PNG =>
            {
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
            ImageFormat::PPM =>
            {
                #[cfg(feature = "ppm")]
                {
                    Ok(Box::new(zune_ppm::PPMDecoder::new_with_options(
                        options, data
                    )))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }
            ImageFormat::PSD =>
            {
                #[cfg(feature = "ppm")]
                {
                    Ok(Box::new(zune_psd::PSDDecoder::new_with_options(
                        data, options
                    )))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    Err(ImageErrors::ImageDecoderNotIncluded(*self))
                }
            }

            ImageFormat::Farbfeld =>
            {
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

            ImageFormat::QOI =>
            {
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
            ImageFormat::HDR =>
            {
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
            ImageFormat::BMP =>
            {
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
            ImageFormat::JPEG_XL => Err(ImageErrors::ImageDecoderNotImplemented(*self)),
            ImageFormat::Unknown => Err(ImageErrors::ImageDecoderNotImplemented(*self))
        }
    }

    pub fn get_encoder(&self) -> Option<Box<dyn EncoderTrait>>
    {
        self.get_encoder_with_options(EncoderOptions::default())
    }
    pub fn get_encoder_with_options(&self, options: EncoderOptions)
        -> Option<Box<dyn EncoderTrait>>
    {
        match self
        {
            Self::PPM =>
            {
                #[cfg(feature = "ppm")]
                {
                    Some(Box::new(crate::codecs::ppm::PPMEncoder::new_with_options(
                        options
                    )))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            Self::QOI =>
            {
                #[cfg(feature = "qoi")]
                {
                    Some(Box::new(crate::codecs::qoi::QoiEncoder::new_with_options(
                        options
                    )))
                }
                #[cfg(not(feature = "qoi"))]
                {
                    None
                }
            }
            Self::JPEG =>
            {
                #[cfg(feature = "jpeg")]
                {
                    Some(Box::new(
                        crate::codecs::jpeg::JpegEncoder::new_with_options(options)
                    ))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    None
                }
            }
            Self::JPEG_XL =>
            {
                #[cfg(feature = "jpeg-xl")]
                {
                    Some(Box::new(
                        crate::codecs::jpeg_xl::JxlEncoder::new_with_options(options)
                    ))
                }
                #[cfg(not(feature = "jpeg-xl"))]
                {
                    None
                }
            }
            Self::HDR =>
            {
                #[cfg(feature = "hdr")]
                {
                    Some(Box::new(crate::codecs::hdr::HdrEncoder::new_with_options(
                        options
                    )))
                }
                #[cfg(not(feature = "hdr"))]
                {
                    None
                }
            }
            Self::PNG =>
            {
                #[cfg(feature = "png")]
                {
                    Some(Box::new(codecs::png::PngEncoder::new_with_options(options)))
                }
                #[cfg(not(feature = "png"))]
                {
                    None
                }
            }
            // all encoders not implemented default to none
            _ => None
        }
    }
    pub fn guess_format(bytes: &[u8]) -> Option<ImageFormat>
    {
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
        ];

        for (magic, decoder) in magic_bytes
        {
            if bytes.starts_with(magic)
            {
                return Some(decoder);
            }
        }
        #[cfg(feature = "bmp")]
        {
            if zune_bmp::probe_bmp(bytes)
            {
                return Some(ImageFormat::BMP);
            }
        }

        None
    }

    pub fn get_encoder_for_extension<P: AsRef<str>>(
        extension: P
    ) -> Option<(ImageFormat, Box<dyn EncoderTrait>)>
    {
        match extension.as_ref()
        {
            "qoi" =>
            {
                #[cfg(feature = "qoi")]
                {
                    Some((ImageFormat::QOI, ImageFormat::QOI.get_encoder().unwrap()))
                }
                #[cfg(not(feature = "qoi"))]
                {
                    None
                }
            }
            "ppm" | "pam" | "pgm" | "pbm" | "pfm" =>
            {
                #[cfg(feature = "ppm")]
                {
                    Some((ImageFormat::PPM, ImageFormat::PPM.get_encoder().unwrap()))
                }
                #[cfg(not(feature = "ppm"))]
                {
                    None
                }
            }
            "jpeg" | "jpg" =>
            {
                #[cfg(feature = "jpeg")]
                {
                    Some((ImageFormat::JPEG, ImageFormat::JPEG.get_encoder().unwrap()))
                }
                #[cfg(not(feature = "jpeg"))]
                {
                    None
                }
            }
            "jxl" =>
            {
                #[cfg(feature = "jpeg-xl")]
                {
                    Some((
                        ImageFormat::JPEG_XL,
                        ImageFormat::JPEG_XL.get_encoder().unwrap()
                    ))
                }
                #[cfg(not(feature = "jpeg-xl"))]
                {
                    None
                }
            }
            "ff" =>
            {
                #[cfg(feature = "farbfeld")]
                {
                    Some((
                        ImageFormat::Farbfeld,
                        ImageFormat::Farbfeld.get_encoder().unwrap()
                    ))
                }
                #[cfg(not(feature = "farbfeld"))]
                {
                    None
                }
            }
            "hdr" =>
            {
                #[cfg(feature = "hdr")]
                {
                    Some((ImageFormat::HDR, ImageFormat::HDR.get_encoder().unwrap()))
                }
                #[cfg(not(feature = "hdr"))]
                {
                    None
                }
            }
            "png" =>
            {
                #[cfg(feature = "png")]
                {
                    Some((ImageFormat::PNG, ImageFormat::PNG.get_encoder().unwrap()))
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
impl Image
{
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
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::image::Image;
    /// // create a luma image
    /// let image = Image::fill::<u8>(128,ColorSpace::Luma,100,100).unwrap();
    /// // save to jpeg
    /// image.save("hello.jpg").unwrap();
    /// ```
    pub fn save<P: AsRef<Path>>(&self, file: P) -> Result<(), ImageErrors>
    {
        return if let Some(ext) = file.as_ref().extension()
        {
            if let Some((format, _)) = ImageFormat::get_encoder_for_extension(ext.to_str().unwrap())
            {
                self.save_to(file, format)
            }
            else
            {
                let msg = format!("No encoder for extension {ext:?}");

                Err(ImageErrors::EncodeErrors(ImgEncodeErrors::Generic(msg)))
            }
        }
        else
        {
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
    /// use zune_image::image::Image;
    /// // create a simple 200x200 grayscale image consisting of pure black
    /// let image = Image::fill::<u8>(0,ColorSpace::Luma,200,200).unwrap();
    /// // save that to jpeg
    /// image.save_to("black.jpg",ImageFormat::JPEG).unwrap();
    /// ```
    pub fn save_to<P: AsRef<Path>>(&self, file: P, format: ImageFormat) -> Result<(), ImageErrors>
    {
        let contents = self.save_to_vec(format)?;
        std::fs::write(file, contents)?;
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
    /// let contents = image.save_to_vec(ImageFormat::QOI).unwrap();
    /// ```
    pub fn save_to_vec(&self, format: ImageFormat) -> Result<Vec<u8>, ImageErrors>
    {
        if let Some(mut encoder) = format.get_encoder()
        {
            // encode
            encoder.encode(self)
        }
        else
        {
            Err(ImageErrors::EncodeErrors(
                crate::errors::ImgEncodeErrors::NoEncoderForFormat(format)
            ))
        }
    }

    /// Open an encoded file for which the library has a configured decoder for it
    ///
    /// # Note
    /// - This reads the whole file into memory before parsing
    /// do not use for large files
    ///
    /// - The decoders supported can be switched on and off depending on how
    ///  you configure your Cargo.toml. It is generally recommended to not enable decoders
    ///  you will not be using as it reduces both the security attack surface and dependency
    ///
    /// # Arguments
    /// - file: The file path from which to read the file from, the file must be a supported format
    /// otherwise it's an error to try and decode
    ///
    /// See also [open_from_mem](Self::open_from_mem) for reading from memory
    pub fn open<P: AsRef<Path>>(file: P) -> Result<Image, ImageErrors>
    {
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
    ) -> Result<Image, ImageErrors>
    {
        let file = std::fs::read(file)?;

        Self::open_from_mem(&file, options)
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
    /// use zune_core::options::DecoderOptions;
    /// use zune_image::image::Image;
    /// // create a simple ppm p5 grayscale format
    /// let image = Image::open_from_mem(b"P5 1 1 255 1",DecoderOptions::default());
    ///```
    pub fn open_from_mem(src: &[u8], options: DecoderOptions) -> Result<Image, ImageErrors>
    {
        let decoder = ImageFormat::guess_format(src);

        if let Some(format) = decoder
        {
            let mut image_decoder = format.get_decoder_with_options(src, options)?;

            image_decoder.decode()
        }
        else
        {
            Err(ImageErrors::ImageDecoderNotImplemented(
                ImageFormat::Unknown
            ))
        }
    }
}
