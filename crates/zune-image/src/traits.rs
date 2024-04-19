/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Various encapsulations of common image operations
//!
//! This contains traits that allow homogenous implementation of various items in different stages
//! of the image processing timeline and the use of this via encapsulation without worrying of implementation
//!
//! The main traits are divided into the following
//! - decoding: `DecoderTrait`: Implementing this for a format means the library can decode such formats
//! - image processing `OperationsTrait`: Implementing this means one can modify the image or extract information from it
//! - encoding: `EncoderTrait`: Implementing this means the image can be saved to a certain format
//!
#![allow(unused_variables)]
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::bytestream::ZByteWriterTrait;
use zune_core::colorspace::{ColorSpace, ALL_COLORSPACES};
use zune_core::log::{trace, warn};
use zune_core::options::EncoderOptions;

use crate::codecs::ImageFormat;
use crate::core_filters::colorspace::ColorspaceConv;
use crate::core_filters::depth::Depth;
use crate::errors::{ImageErrors, ImageOperationsErrors};
use crate::image::Image;
use crate::metadata::AlphaState::NonPreMultiplied;
use crate::metadata::{AlphaState, ImageMetadata};
use crate::pipelines::EncodeResult;

/// Encapsulates an image decoder.
///
/// All supported image decoders must implement this class
pub trait DecoderTrait {
    /// Decode a buffer already in memory
    ///
    /// The buffer to be decoded is the one passed
    /// to the decoder when initializing the decoder
    ///
    /// # Returns
    /// - Image -> Pixels decoded from the image as interleaved pixels.
    ///
    /// # Errors
    ///  - Any image decoding errors will be propagated to the caller.
    ///
    /// # Example
    /// ```no_run
    /// use zune_core::bytestream::ZCursor;
    /// #[cfg(feature = "jpeg")]
    /// {
    ///     use zune_image::traits::DecoderTrait;
    ///     use zune_jpeg::JpegDecoder;
    ///     let mut decoder = JpegDecoder::new(ZCursor::new([0xFF,0xD8]));
    ///
    ///     decoder.decode().unwrap();
    /// }
    /// #[cfg(not(feature="jpeg"))]
    /// ()
    /// ```
    fn decode(&mut self) -> Result<Image, crate::errors::ImageErrors>;

    /// Get width and height of the image
    ///
    /// # Returns
    /// - Some(width,height)
    /// - None -> If image hasn't been decoded and we can't extract
    ///  the width and height.
    fn dimensions(&self) -> Option<(usize, usize)>;

    /// Get the colorspace that the decoded pixels
    /// are stored in.
    fn out_colorspace(&self) -> ColorSpace;

    /// Get the name of the decoder
    fn name(&self) -> &'static str;

    /// Return true whether or not this codec is fully supported
    /// and well tested to handle various formats.
    ///
    /// Currently set to true but a codec that is experimental should override it
    /// to be false
    fn is_experimental(&self) -> bool {
        false
    }
    /// Read image metadata returning the values as
    /// a struct
    fn read_headers(&mut self) -> Result<Option<ImageMetadata>, crate::errors::ImageErrors> {
        Ok(None)
    }
}

/// This encapsulates an image operation.
///
/// All operations that can be stored in a workflow
/// need to encapsulate this struct.
pub trait OperationsTrait {
    /// Get the name of this operation
    fn name(&self) -> &'static str;

    /// Execute a simple operation on the image
    /// manipulating the image struct
    ///
    /// An object should implement this function, but
    /// a caller should call [`execute`], which does some error checking
    /// before calling this method
    ///
    /// [`execute`]: Self::execute
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>;

    /// Return the supported colorspaces this operation supports
    ///
    /// Some operations cannot work on all colorspaces, e.g rgb to grayscale will
    /// only work on RGB colorspace, not YUV or YCbCr colorspace, hence such an operation
    /// must only declare support for such colorspace
    ///
    /// During execution, the image colorspace will be matched to this colorspace and
    /// if it doesn't support it, an error will be raised during execution
    fn supported_colorspaces(&self) -> &'static [ColorSpace] {
        &ALL_COLORSPACES
    }
    /// Get supported bit types for this operation
    ///
    /// Not all operations are supported for all bit types and
    /// o each support requires careful analysis to ensure it's doing
    /// the right things
    fn supported_types(&self) -> &'static [BitType];

    /// Execute an operation
    ///
    /// This does come common error checking operations, e.g
    /// it checks that image dimensions match array length and that this operation
    /// supports the image colorspace, before calling [`execute_impl`]
    ///
    /// # Arguments
    /// - image: A mutable reference to an image which
    /// this operation will manipulate
    ///
    ///
    /// # Errors
    /// Any operations error will be propagated to the caller
    ///
    ///
    /// [`execute_impl`]: Self::execute_impl
    fn execute(&self, image: &mut Image) -> Result<(), ImageErrors> {
        // Confirm colorspace
        let colorspace = image.colorspace();

        let supported = self
            .supported_colorspaces()
            .iter()
            .any(|x| *x == colorspace);

        if !supported {
            return Err(ImageErrors::UnsupportedColorspace(
                colorspace,
                self.name(),
                self.supported_colorspaces()
            ));
        }
        // if image.metadata.alpha != self.alpha_state()
        // {
        //     PremultiplyAlpha::new(self.alpha_state());
        // }
        // check we support the bit depth
        let bit_type = image.metadata.depth().bit_type();

        let supported = self.supported_types().iter().any(|x| *x == bit_type);

        if !supported {
            return Err(ImageErrors::OperationsError(
                ImageOperationsErrors::UnsupportedType(self.name(), bit_type)
            ));
        }

        confirm_invariants(image)?;

        self.execute_impl(image)
            .map_err(<ImageErrors as Into<ImageErrors>>::into)?;

        confirm_invariants(image)?;

        Ok(())
    }
    /// Alpha state for which the image operation works in
    ///
    /// Most image expect a premultiplied alpha state to work correctly
    /// this allows one to override the alpha state the image will
    /// be converted into before carrying out an operation
    fn alpha_state(&self) -> AlphaState {
        AlphaState::PreMultiplied
    }

    /// Clone the image and execute the operation on it, returning
    /// a new image instead of modifying the existing one
    ///
    /// This is provided as a convenience function for when one
    /// doesn't want to modify the existing image
    fn clone_and_execute(&self, image: &Image) -> Result<Image, ImageErrors> {
        let mut c_img = image.clone();
        self.execute(&mut c_img)?;
        Ok(c_img)
    }
}

/// Confirm that image invariants have been respected across image
/// operations
fn confirm_invariants(image: &Image) -> Result<(), ImageErrors> {
    // Ensure dimensions are correct

    for frame in image.frames_ref() {
        if frame.channels.len() != image.colorspace().num_components() {
            {
                return Err(ImageErrors::GenericString(format!(
                    "Components mismatch, expected {} channels since image format is {:?}, but found {}",
                    image.colorspace().num_components(),
                    image.colorspace(),
                    frame.channels.len()
                )));
            }
        }
    }

    let (width, height) = image.dimensions();
    // check the number of channels match the length

    let expected_length = image.depth().size_of() * width * height;

    for channel in image.channels_ref(true) {
        if channel.len() != expected_length {
            return Err(ImageErrors::DimensionsMisMatch(
                expected_length,
                channel.len()
            ));
        }
    }

    Ok(())
}

/// The trait dealing with image encoding and saving
pub trait EncoderTrait {
    /// Get the name of the encoder
    fn name(&self) -> &'static str;

    /// Encode and write to a file
    ///
    /// The file is stored internally by the decoder, e.g
    /// by asking for it during initialization
    ///
    /// # Arguments
    /// - image: An image which we are trying to encode.
    ///
    /// # Returns
    /// - `Ok(usize)`: The number of bytes written into `sink`
    /// in the format [ImageFormat]
    ///
    /// - Err : An unrecoverable error occurred
    ///
    fn encode_inner<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors>;

    /// Return all colorspaces supported by this encoder.
    ///
    /// An encoder should reject any other colorspace and should not try to write
    /// an unknown colorspace
    fn supported_colorspaces(&self) -> &'static [ColorSpace];

    /// Encode the actual image into the specified format
    ///
    /// This also covers conversion where possible,
    /// allowing things like bit-depth conversion where possible
    ///
    /// After doing some book keeping, this will eventually call `encode_inner` which
    /// actually carries out the encoding
    ///
    /// # Arguments
    /// - image: The image to encode
    ///
    /// # Returns
    ///
    /// - `Ok(usize)`: The number of bytes written into `sink`
    /// in the format [ImageFormat]
    ///
    /// - Err : An unrecoverable error occurred
    ///
    /// # Note
    /// The library may clone `image` depending on configurations of the encoder
    /// e.g to do colorspace conversions or bit-depth conversions, hence it
    /// is recommended to have the image in a format that can be encoded
    /// directly to prevent such
    fn encode<T: ZByteWriterTrait>(
        &mut self, image: &Image, sink: T
    ) -> Result<usize, ImageErrors> {
        // confirm things hold themselves
        confirm_invariants(image)?;

        // check colorspace is correct.
        let colorspace = image.colorspace();
        let supported_colorspaces = self.supported_colorspaces();

        // deal convert bit depths
        let depth = image.depth();

        if image.is_animated() && !self.supports_animated_images() {
            warn!("The current image is animated but the encoder ({:?}) doesn't support animated images, this will only encode the first frame",self.name());
        }
        if !supported_colorspaces.contains(&colorspace)
            || !self.supported_bit_depth().contains(&depth)
            || image.metadata.alpha != NonPreMultiplied
        {
            let mut image_clone = image.clone();

            if !supported_colorspaces.contains(&colorspace) {
                // get default colorspace
                let default_colorspace = self.default_colorspace(colorspace);
                let image_format = self.format();

                trace!("Image is in {colorspace:?} colorspace,converting it to {default_colorspace:?} which is the default configured colorspace of {image_format:?}");
                // try converting  it to a supported colorspace
                let converter = ColorspaceConv::new(default_colorspace);

                converter.execute(&mut image_clone)?
            }
            let image_depth = image.depth();

            if !self.supported_bit_depth().contains(&depth) {
                trace!(
                    "Image depth is in {:?}, but {} encoder supports {:?}",
                    image.depth(),
                    self.name(),
                    self.supported_bit_depth()
                );
                trace!(
                    "Converting image to a depth of {:?}",
                    self.default_depth(image_depth)
                );

                let depth = Depth::new(self.default_depth(image_depth));

                depth.execute(&mut image_clone)?;
            }

            // confirm again we didn't mess up
            confirm_invariants(&image_clone)?;

            self.encode_inner(&image_clone, sink)
        } else {
            self.encode_inner(image, sink)
        }
    }
    /// Return the image format for which this
    /// encoder will encode the format in
    ///
    /// # Example
    ///  Get jpeg encoder format
    ///-  Requires jpeg feature to work
    /// ```
    /// use zune_image::codecs::ImageFormat;
    /// use zune_image::codecs::jpeg::JpegEncoder;
    /// use zune_image::traits::EncoderTrait;
    ///
    /// let encoder = JpegEncoder::new();
    /// assert_eq!(encoder.format(),ImageFormat::JPEG);
    /// ```
    fn format(&self) -> ImageFormat;

    /// Call `encode` and then store the image
    /// and format in `EncodeResult`
    fn encode_to_result(&mut self, image: &Image) -> Result<EncodeResult, ImageErrors> {
        let mut sink = vec![];
        let data = self.encode(image, &mut sink)?;

        Ok(EncodeResult {
            data:   vec![],
            format: self.format()
        })
    }
    /// Get supported bit-depths for this image
    ///
    /// This should return all supported bit depth
    /// for the encoder
    fn supported_bit_depth(&self) -> &'static [BitDepth];

    /// Returns the common/expected bit depth for this image
    ///
    /// This is used in conjunction with [`supported_bit_depth`]
    /// for cases where we want to convert the image to a bit depth
    /// since the image is not in one of the supported image formats
    ///
    /// [`supported_bit_depth`]:EncoderTrait::supported_bit_depth
    fn default_depth(&self, depth: BitDepth) -> BitDepth;

    /// Returns the default colorspace to use when the image
    /// contains a different colorspace
    ///
    /// Default is RGB
    ///
    /// # Arguments
    /// - colorspace: The colorspace the image is currently in
    fn default_colorspace(&self, _: ColorSpace) -> ColorSpace {
        ColorSpace::RGB
    }

    /// Set encoder options for this encoder
    ///
    /// This allows one to configure specific settings for an encoder where supported
    fn set_options(&mut self, _: EncoderOptions) {}

    /// Return true if the encoder can encode multiple image frames as one animated Image.
    ///
    /// This returns true if the format and encoder can encode animated images, false otherwise
    ///
    /// If false, the encoder will only encode one frame from the image otherwise it will
    /// encode all frames as a single animated image
    fn supports_animated_images(&self) -> bool {
        false
    }
}

/// Trait that encapsulates supported
/// integers which work with the image crates
pub trait ZuneInts<T> {
    fn depth() -> BitDepth;

    ///Maximum value for this type
    ///
    /// For integers its the maximum value they can hold
    /// For float values it's 1.0
    fn max_value() -> T;
}

impl ZuneInts<u8> for u8 {
    #[inline(always)]
    fn depth() -> BitDepth {
        BitDepth::Eight
    }
    #[inline(always)]
    fn max_value() -> u8 {
        255
    }
}

impl ZuneInts<u16> for u16 {
    #[inline(always)]
    fn depth() -> BitDepth {
        BitDepth::Sixteen
    }
    #[inline(always)]
    fn max_value() -> u16 {
        u16::MAX
    }
}

impl ZuneInts<f32> for f32 {
    #[inline(always)]
    fn depth() -> BitDepth {
        BitDepth::Float32
    }
    #[inline(always)]
    fn max_value() -> f32 {
        1.0
    }
}

/// Trait that encapsulates image decoders that
/// can write data as raw native endian into
/// a buffer of u8
pub trait DecodeInto {
    /// Decode raw image bytes into a buffer that can
    /// hold u8 bytes
    ///
    /// The rationale is that u8 bytes can alias any type
    /// and higher bytes offer ways to construct types from
    /// u8's hence they can be used as a base type
    fn decode_into(&mut self, buffer: &mut [u8]) -> Result<(), ImageErrors>;

    /// Minimum buffer length which is needed to decode this image
    ///
    /// This may call `decode_headers` for the image routine to fetch the
    /// expected output size.
    fn output_buffer_size(&mut self) -> Result<usize, ImageErrors>;
}
/// Convert something into an image by consuming it
pub trait IntoImage {
    /// Consumes this and returns an image
    fn into_image(self) -> Result<Image, ImageErrors>;
}

impl IntoImage for Image {
    fn into_image(self) -> Result<Image, ImageErrors> {
        Ok(self)
    }
}
