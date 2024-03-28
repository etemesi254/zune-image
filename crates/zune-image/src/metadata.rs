/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Image metadata
//!
//! This module provides the ability to store image metadata and transfer it
//! from one image to another

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::{ColorCharacteristics, ColorSpace};

use crate::codecs::ImageFormat;

mod exif;

/// Contains information about whether the image
/// is pre multiplied with it's alpha
/// or it's not
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AlphaState {
    PreMultiplied,
    NonPreMultiplied
}

/// Image metadata
///
/// Each image type has this information present
/// The decoder usually sets this up while the encoder
/// can get these details from the user/image struct
#[derive(Clone, Debug)]
pub struct ImageMetadata {
    // REMEMBER: If you add a field here add it's serialization
    // to mod file
    pub(crate) color_trc:     Option<ColorCharacteristics>,
    pub(crate) default_gamma: Option<f32>,
    pub(crate) width:         usize,
    pub(crate) height:        usize,
    pub(crate) colorspace:    ColorSpace,
    pub(crate) depth:         BitDepth,
    pub(crate) format:        Option<ImageFormat>,
    pub(crate) alpha:         AlphaState,
    #[cfg(feature = "metadata")]
    pub(crate) exif:          Option<Vec<::exif::Field>>,
    pub(crate) icc_chunk:     Option<Vec<u8>>
}

impl Default for ImageMetadata {
    fn default() -> Self {
        ImageMetadata {
            color_trc: None,
            default_gamma: None,
            width: 0,
            height: 0,
            colorspace: ColorSpace::Unknown,
            depth: BitDepth::default(),
            format: None,
            alpha: AlphaState::NonPreMultiplied,
            #[cfg(feature = "metadata")]
            exif: None,

            icc_chunk: None
        }
    }
}

impl ImageMetadata {
    /// Return the exif metadata of an image or none if it
    /// doesn't exist
    ///
    /// This requires the metadata feature otherwise
    /// it will always return `None`
    #[cfg(feature = "metadata")]
    pub const fn exif(&self) -> Option<&Vec<::exif::Field>> {
        #[cfg(feature = "metadata")]
        {
            return self.exif.as_ref();
        }
        #[cfg(not(feature = "metadata"))]
        {
            return None;
        }
    }

    /// Return a mutable reference to the exif metadata of an image or none if it
    /// doesn't exist
    ///
    /// This requires the metadata feature otherwise
    /// it will always return `None`
    #[cfg(feature = "metadata")]

    pub fn exif_mut(&mut self) -> Option<&mut Vec<::exif::Field>> {
        #[cfg(feature = "metadata")]
        {
            return self.exif.as_mut();
        }
        #[cfg(not(feature = "metadata"))]
        {
            return None;
        }
    }
    /// Get image dimensions as a tuple of width and height
    ///  
    /// # Example
    ///
    /// ```rust
    ///use zune_image::metadata::ImageMetadata;
    /// let meta = ImageMetadata::default();
    /// // default dimensions are usually zero
    /// assert_eq!(meta.dimensions(),(0,0));
    /// ```
    pub const fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
    /// Set image dimensions
    ///
    /// # Example
    /// ```
    /// use zune_image::metadata::ImageMetadata;
    /// let mut meta = ImageMetadata::default();
    /// // set image dimensions
    /// meta.set_dimensions(23,24);
    /// // get image dimensions
    /// assert_eq!(meta.dimensions(),(23,24));
    /// ```
    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
    }
    /// Get an image's colorspace
    ///
    /// The default colorspace is usually [`ColorSpace::Unknown`]
    /// which represents an uninitialized image
    pub const fn colorspace(&self) -> ColorSpace {
        self.colorspace
    }
    /// Set the image's colorspace
    pub fn set_colorspace(&mut self, colorspace: ColorSpace) {
        self.colorspace = colorspace;
    }
    /// Get color transfer characteristics
    ///
    /// Color transfer characteristics tell us more about how
    /// the colorspace values are represented
    /// whether they are linear or gamma encoded
    pub const fn color_trc(&self) -> Option<ColorCharacteristics> {
        self.color_trc
    }
    /// Set color transfer characteristics for this image
    pub fn set_color_trc(&mut self, trc: ColorCharacteristics) {
        self.color_trc = Some(trc);
    }
    /// Get the image bit depth
    ///
    /// Default value is [`BitDepth::Unknown`]
    /// which indicates that the bit-depth is currently unknown for a
    /// particular image
    pub const fn depth(&self) -> BitDepth {
        self.depth
    }
    /// Set the image bit depth
    pub fn set_depth(&mut self, depth: BitDepth) {
        self.depth = depth;
    }
    /// Set the default gamma for this image
    ///
    /// This is gamma that will be used to convert this image
    /// from gamma colorspace to linear colorspace and back.
    ///
    /// Do not set this in between operations, and do not set
    /// this if you do not know what you are doing.
    ///
    /// The library will set this automatically for supported decoders
    /// (those which specify gamma during transfer)
    ///
    /// # Arguments
    /// - gamma : The new gamma value
    pub fn set_default_gamma(&mut self, gamma: f32) {
        self.default_gamma = Some(gamma);
    }

    /// Get the image for which this metadata was fetched from
    ///
    /// May be None if the caller didn't set a format
    pub const fn image_format(&self) -> Option<ImageFormat> {
        self.format
    }

    pub const fn alpha(&self) -> AlphaState {
        self.alpha
    }
    pub fn is_premultiplied_alpha(&self) -> bool {
        self.alpha.eq(&AlphaState::PreMultiplied)
    }

    pub fn set_alpha(&mut self, alpha_state: AlphaState) {
        self.alpha = alpha_state;
    }

    pub fn set_icc_chunk(&mut self, icc_chunk: Vec<u8>) {
        self.icc_chunk = Some(icc_chunk);
    }
    /// Return the icc chunk of the image
    pub fn icc_chunk(&self) -> Option<&Vec<u8>> {
        self.icc_chunk.as_ref()
    }
}
