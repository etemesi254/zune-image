/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ops::{Deref, DerefMut};

use wasm_bindgen::prelude::*;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::log::{debug, error, info};
// use zune_core::colorspace::ColorSpace;
use zune_image::codecs::ImageFormat;
use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::core_filters::depth::Depth;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::auto_orient::AutoOrient;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::blend::Blend;
use zune_imageprocs::box_blur::BoxBlur;
use zune_imageprocs::brighten::Brighten;
use zune_imageprocs::color_matrix::ColorMatrix;
use zune_imageprocs::contrast::Contrast;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::Flip;
use zune_imageprocs::flop::Flop;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::hsv_adjust::HsvAdjust;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::median::Median;
use zune_imageprocs::premul_alpha::PremultiplyAlpha;
use zune_imageprocs::spatial::SpatialOps;
use zune_imageprocs::spatial_ops::SpatialOperations;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::threshold::{Threshold, ThresholdMethod};

use crate::enums::{WasmColorspace, WasmImageFormats, WasmSpatialOperations};
use crate::utils::set_panic_hook;

mod enums;
mod utils;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet() {
    //alert("Hello, zune-wasm!");
}

#[wasm_bindgen(start)]
pub fn setup() {
    wasm_logger::init(wasm_logger::Config::default());
    set_panic_hook();
    print_initial_stats();
}

fn print_initial_stats() {
    info!("Zune-wasm is live");
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        debug!("Running with SIMD 128 bit support");
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        debug!("No SIMD 128 bit support :( ");
    }
}

//
// #[wasm_bindgen]
// pub struct WasmImageMetadata
// {
//     width:      usize,
//     height:     usize,
//     depth:      BitDepth,
//     colorspace: ColorSpace
// }
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmImage {
    image: Image
}

impl Deref for WasmImage {
    type Target = Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl DerefMut for WasmImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.image
    }
}

#[wasm_bindgen]
impl WasmImage {
    /// Return the width of the image
    ///
    /// @returns: The image width as a `usize` in rust, the equivalent in wasm
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> usize {
        let (width, _) = self.image.dimensions();
        width
    }

    /// Return the height of the image
    ///
    /// @returns: The image width as a `usize` in rust, the equivalent in wasm
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> usize {
        let (_, height) = self.image.dimensions();
        height
    }

    /// Linearly stretches the contrast in an image in place,
    /// sending lower to image minimum and upper to image maximum.
    ///
    /// Values in the mid-range are scaled in respect to the lower and upper values using the formula
    /// `(max_value-pix) / (upper-lower)` where max value is the
    /// largest supported number for the said bit-depth (255 for u8, 1 for f32 images etc)
    ///
    /// @param lower - Lower minimum value for which pixels below this are clamped to the value
    /// @param upper - Upper maximum value for which pixels above are clamped to the value
    pub fn stretch_contrast(&mut self, lower: f32, upper: f32) -> Result<(), JsError> {
        let ops = StretchContrast::new(lower, upper);
        self.execute_ops(&ops)
    }

    /// Internal operation executor that allows me to do some tracking
    fn execute_ops(&mut self, ops: &dyn OperationsTrait) -> Result<(), JsError> {
        let start = web_time::Instant::now();
        match ops.execute(&mut self.image) {
            Ok(()) => {
                let end = web_time::Instant::now();
                info!(
                    "Successfully executed {} in {:?} ms",
                    ops.name(),
                    (end - start).as_millis()
                );
                Ok(())
            }
            Err(e) => {
                error!("Executing {} failed because of {:?}", ops.name(), e);
                Err(e.into())
            }
        }
    }

    /// Apply a brighten operation to the image
    ///
    /// This is a simple `(pix+constant)` operation where `pix` is a single image pixel and `constant`
    /// is the value specified here
    ///
    /// @praram value -  Value to increase the channel values with, must be between -1 and 1, where 1 stands for maximum brightness
    // /// and -1 for darkness
    pub fn brighten(&mut self, value: f32) -> Result<(), JsError> {
        let ops = Brighten::new(value);
        self.execute_ops(&ops)
    }
    /// Apply a contrast operation to the image
    ///
    /// Algorithm used is from [here](https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-5-contrast-adjustment/)
    ///
    /// @param contrast - The contrast adjustment factor.
    pub fn contrast(&mut self, contrast: f32) -> Result<(), JsError> {
        let ops = Contrast::new(contrast);
        self.execute_ops(&ops)
    }
    /// Crop an image creating a sub-image from the initial image
    ///
    /// Origin is defined from the top left corner of the image.
    ///
    /// @param width -  The new image width
    /// @param height - The new image height
    /// @param x -  How far from the x origin the image should start from
    /// @param y -  How far from the y origin the image should start from
    ///
    pub fn crop(&mut self, width: usize, height: usize, x: usize, y: usize) -> Result<(), JsError> {
        self.execute_ops(&Crop::new(width, height, x, y))
    }

    /// Adjust an image's gamma value
    ///
    /// @param value - Gamma adjust parameter, typical range is from 0.8-3.0
    pub fn gamma(&mut self, gamma: f32) -> Result<(), JsError> {
        let ops = Gamma::new(gamma);
        self.execute_ops(&ops)
    }

    /// Invert an image's pixels.
    ///
    /// The typical operation is `max-pixel`, where `max` is the maximum value
    /// supported by that depth, and `pixel` is the current pixel of an image
    pub fn invert(&mut self) -> Result<(), JsError> {
        let ops = Invert::new();
        self.execute_ops(&ops)
    }

    /// Binarize an image.
    ///
    /// THe operation is `pix = max(pix,thresh) > thresh? max_v : 0` where `pix`
    /// is the image pixel, `thresh` is the threshold value and `max_v` is the
    /// largest value the bitdepth supports
    ///
    /// @param threshold - The threshold value, values less than this are replaced with zero,
    /// values greater than this are replaced with max supported integer for that bit depth
    pub fn threshold(&mut self, threshold: f32) -> Result<(), JsError> {
        let ops = Threshold::new(threshold, ThresholdMethod::Binary);
        self.execute_ops(&ops)
    }

    /// Convert an image to  grayscale
    ///
    /// A convenience function for {@link convert_color}
    pub fn grayscale(&mut self) -> Result<(), JsError> {
        self.execute_ops(&ColorspaceConv::new(ColorSpace::Luma))
    }
    /// Convert a color from one colorspace into another.
    ///
    /// Some colorspace do not have a direct conversion, e.g CMYK -> HSV , so for
    /// such we use intermediate conversion, e.g CMYK->RGB->HSV
    ///
    /// @param colorspace - Colorspace to convert the image to
    pub fn convert_color(&mut self, colorspace: WasmColorspace) -> Result<(), JsError> {
        self.execute_ops(&ColorspaceConv::new(colorspace.to_colorspace()))
    }

    /// Carry out a mean filter on the image
    ///
    /// A mean filter replaces a pixel with the average of it's neighbors
    ///
    /// Execution speed depends on array radius and image size
    ///
    /// @param radius : radius of the filter
    pub fn mean_filter(&mut self, radius: usize) -> Result<(), JsError> {
        let ops = SpatialOps::new(radius, SpatialOperations::Mean);
        self.execute_ops(&ops)
    }

    /// Return the image's colorspace
    ///
    /// @returns The current image colorspace
    pub fn colorspace(&mut self) -> WasmColorspace {
        WasmColorspace::from_colorspace(self.image.colorspace())
    }
    /// Auto orient the image based on exif tag
    ///
    /// This is a no-op if the image doesn't have an orientation
    /// exif tag present.
    pub fn auto_orient(&mut self) -> Result<(), JsError> {
        self.execute_ops(&AutoOrient)
    }
    /// Spatial operations implemented for images
    ///
    /// Spatial operations return one value from a pixel's surrounding based on a function
    ///
    /// E.g the `min` operation will return the minimum of the pixel in a given radius.
    ///
    /// @param radius - Image radius to consider, the larger the radius the longer the operation
    /// @param operations - The operation being ran on the image, currently only a set of pre-configured operations can
    /// be ran
    pub fn spatial(
        &mut self, radius: usize, operations: WasmSpatialOperations
    ) -> Result<(), JsError> {
        self.execute_ops(&SpatialOps::new(radius, operations.into()))
    }

    /// Create a new image from a file
    ///
    /// @param file The file path that contains the image details,
    /// if the image is a compression format supported, it will open it automatically
    ///
    /// @returns An image representation if everything goes well, otherwise panics
    #[wasm_bindgen(constructor)]
    pub fn from_file(file: String) -> Result<WasmImage, JsError> {
        let c = Image::open(file).map_err(<ImageErrors as Into<JsError>>::into)?;

        Ok(WasmImage { image: c })
    }

    /// A bilateral filter is a non-linear, edge-preserving,
    /// and noise-reducing smoothing filter for images.
    ///
    /// It is a type of non-linear filter that reduces noise while preserving edges.
    /// The filter works by averaging the pixels in a neighborhood around a given pixel,
    /// but the weights of the pixels are determined not only by their spatial distance from the given pixel,
    /// but also by their intensity difference from the given pixel
    ///
    ///  A description can be found [here](https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/MANDUCHI1/Bilateral_Filtering.html)
    ///
    ///
    /// @param d - Diameter of each pixel neighborhood that is used during filtering. If it is non-positive, it is computed from sigma_space.
    ///
    /// @param sigma_color  - Filter sigma in the color space.
    ///  A larger value of the parameter means that farther colors within the pixel neighborhood (see sigmaSpace)
    ///  will be mixed together, resulting in larger areas of semi-equal color.
    ///
    /// @param sigma_space - Filter sigma in the coordinate space.
    ///  A larger value of the parameter means that farther pixels will influence each other as
    ///   long as their colors are close enough (see sigma_color ).
    ///   When d>0, it specifies the neighborhood size regardless of sigma_space. Otherwise, d is proportional to sigma_space.
    pub fn bilateral_filter(
        &mut self, d: i32, sigma_color: f32, sigma_space: f32
    ) -> Result<(), JsError> {
        self.execute_ops(&BilateralFilter::new(d, sigma_color, sigma_space))
    }

    /// Combine two or more images based on an alpha value
    /// which is used to determine the `opacity` of pixels during blending
    ///
    ///
    /// The formula for blending is
    ///
    /// ```text
    /// dest = (src_alpha) * src  + (1-src_alpha) * dest
    /// ```
    /// `src_alpha` is expected to be between 0.0 and 1.0
    ///
    /// Images must have same width, height, colorspace and depth
    ///
    /// @param other: The secondary image
    ///
    /// @param src_alpha The source alpha parameter, range is between 0-1, and it's clamped there
    pub fn blend(&mut self, other: &WasmImage, src_alpha: f32) -> Result<(), JsError> {
        self.execute_ops(&Blend::new(&other.image, src_alpha))
    }
    /// Perform a mean/box blur of the image
    ///
    /// This returns the average pixels of a radius `radius` around a pixel
    /// The execution is independent of size
    ///
    /// @param radius - The number of neighbours that would be included per blur pixel,
    /// the larger the number the more pronounced the blur
    pub fn box_blur(&mut self, radius: usize) -> Result<(), JsError> {
        self.execute_ops(&BoxBlur::new(radius))
    }
    /// Adjust exposure of an image
    ///
    /// Formula for exposure is `pix = clamp((pix - black) * exposure)`
    ///
    /// @param exposure - Value to adjust pixels with, range is usually betweeen 0 and +infinity
    /// value of `1` doesn't have an effect, `2` increases pixel intensity by 2
    ///
    /// @param black_point - Offset to adjust the pixels with before carrying out exposure, default value
    /// would be `0.0` for it to have no range
    pub fn exposure(&mut self, exposure: f32, black_point: f32) -> Result<(), JsError> {
        self.execute_ops(&Exposure::new(exposure, black_point))
    }

    /// Flip an image horizontally
    ///
    ///
    /// ```text
    ///old image     new image
    ///┌─────────┐   ┌──────────┐
    ///│a b c d e│   │e d b c a │
    ///│f g h i j│   │j i h g f │
    ///└─────────┘   └──────────┘
    ///```
    ///
    pub fn flip_horizontal(&mut self) -> Result<(), JsError> {
        self.execute_ops(&Flop::new())
    }

    /// Flip an image by reflecting pixels around the x-axis.
    ///
    /// ```text
    ///
    ///  old image     new image
    /// ┌─────────┐   ┌──────────┐
    /// │a b c d e│   │j i h g f │
    /// │f g h i j│   │e d c b a │
    /// └─────────┘   └──────────┘
    /// ```
    pub fn flip_vertical(&mut self) -> Result<(), JsError> {
        self.execute_ops(&Flip::new())
    }
    /// Blur the image using a gaussian kernel with sigma `sigma`
    ///
    /// @param sigma - A value of how much to blur the image by, larger values means
    /// more pronounced blurs
    pub fn gaussian_blur(&mut self, sigma: f32) -> Result<(), JsError> {
        self.execute_ops(&GaussianBlur::new(sigma))
    }

    /// Adjust either the hue, saturation and lightness/value of the image
    ///
    /// @param hue - The hue rotation argument. This is usually a value between 0 and 360 degrees
    ///
    /// @param saturation - The saturation scaling factor, a value of 0 produces a grayscale image, 1 has no effect, other values lie withing that
    /// spectrum > 1 produces vibrant cartoonish color
    ///
    /// @param  lightness - The lightness scaling factor, a value greater than 0, values less than or equal to zero
    /// produce a black image, higher values increase the brightness of the image, 1.0 doesn't do anything
    /// to image lightness
    pub fn hsv_adjust(&mut self, hue: f32, saturation: f32, lightness: f32) -> Result<(), JsError> {
        self.execute_ops(&HsvAdjust::new(hue, saturation, lightness))
    }

    /// Applies a median filter of given dimensions to an image. Each output pixel is the median
    /// of the pixels in a `(2 * radius + 1) * (2 * radius + 1)` kernel of pixels in the input image.
    ///
    /// @param radius: The radius to consider for the image
    pub fn median(&mut self, radius: usize) -> Result<(), JsError> {
        self.execute_ops(&Median::new(radius))
    }
    /// Pre multiply the image alpha
    ///
    ///  Note: This operation is lossy, one cannot undo effects of it
    /// (e.g where we are multiplying by zero), use with caution
    pub fn premultiply(&mut self) -> Result<(), JsError> {
        self.execute_ops(&PremultiplyAlpha::new(AlphaState::PreMultiplied))
    }
    /// Undo alpha pre-multiplication
    pub fn unpremultiply(&mut self) -> Result<(), JsError> {
        self.execute_ops(&PremultiplyAlpha::new(AlphaState::NonPreMultiplied))
    }

    /// Apply a color matrix operation on an RGBA image
    ///
    /// The matrix is a 4 by 5 matrix
    ///
    /// This multiplies color bands by  different factors and adds them together
    /// to create one output pixel
    ///
    /// Equivalent to the operation
    /// ```text
    /// red   = m[0][0]*r + m[0][1]*g + m[0][2]*b + m[0][3]*a + m[0][4]
    /// green = m[1][0]*r + m[1][1]*g + m[1][2]*b + m[1][3]*a + m[1][4]
    /// blue  = m[2][0]*r + m[2][1]*g + m[2][2]*b + m[2][3]*a + m[2][4]
    /// alpha = m[3][0]*r + m[3][1]*g + m[3][2]*b + m[3][3]*a + m[3][4]
    ///```
    ///
    /// This is most similar to Android's [ColorMatrix](https://developer.android.com/reference/android/graphics/ColorMatrix) operation
    ///  with the difference being that matrix values are always between 0 and 1 and the library will do appropriate scaling
    ///
    /// This is similar to imagemagick's [color-matrix](https://imagemagick.org/script/command-line-options.php?#color-matrix) operator
    /// with some examples provided in the website at [Color matrix operator](https://imagemagick.org/Usage/color_mods/#color-matrix)
    ///
    ///
    /// A playground to build color matrices can be found [here](https://fecolormatrix.com/) (external link, not affiliated)
    ///
    /// @param matrix -  An array of length 20 representing a matrix, 4 rows each representing color channels in the order R,G,B,A
    ///  and each row has 5 columns with the effect of `[r,g,b,a,offset]` , the matrices are arranged from left to right, top to bottom
    ///
    pub fn color_matrix(&mut self, matrix: &[f32]) -> Result<(), JsError> {
        match ColorMatrix::try_from_slice(matrix) {
            None => Err(JsError::new("Length of matrix is not 20")),
            Some(r) => self.execute_ops(&r)
        }
    }

    /// Returns `true` if image has an alpha, `false` otherwise
    #[wasm_bindgen(getter)]
    pub fn has_alpha(&self) -> bool {
        self.image.colorspace().has_alpha()
    }

    /// Save an image to a specified supported format returning the encoded bytes for that format
    ///
    /// @param format - The image format, not all formats have encoders, but most have.
    pub fn save_to(&self, format: WasmImageFormats) -> Result<Vec<u8>, JsError> {
        let mut dest = Vec::with_capacity(1000); // arbitrary number of how many

        self.image
            .encode(format.to_format(), &mut dest)
            .map_err(<ImageErrors as Into<JsError>>::into)?;

        Ok(dest)
    }

    // /// Create a new image from in memory bytes of a compressed image
    // ///
    // /// @param bytes The bytes containing encoded pixels in a specific format.
    // /// The library will infer the image format from the bytes themselves
    // ///
    // /// @returns An image representation if everything goes well, otherwise panics
    // pub fn from_bytes(bytes: &[u8]) -> Result<WasmImage, JsError> {
    //     let c = Image::read(ZCursor::from(bytes), DecoderOptions::new_fast())
    //         .map_err(<ImageErrors as Into<JsError>>::into)?;
    //
    //     Ok(WasmImage { image: c })
    // }
}

/// Decode an image returning the pixels if the image is decodable
/// or none otherwise
#[wasm_bindgen]
pub fn decode(bytes: &[u8]) -> Option<WasmImage> {
    if let Some((format, content)) = ImageFormat::guess_format(ZCursor::new(bytes)) {
        if let Ok(mut decoder) = format.decoder(content) {
            let mut image = decoder.decode().unwrap();

            // WASM works with 8 bit images, so convert this to an 8 biy image
            Depth::new(BitDepth::Eight).execute(&mut image).unwrap();

            return Some(WasmImage { image });
        } else {
            error!(
                "Could not decode {:?}",
                format.decoder(ZCursor::new(bytes)).err().unwrap()
            )
        }
    }
    None
}

/// Guess the image format returning an enum if we know the format
///
/// or None otherwise
#[wasm_bindgen]
pub fn guess_format(bytes: &[u8]) -> Option<WasmImageFormats> {
    if let Some((format, _)) = ImageFormat::guess_format(ZCursor::new(bytes)) {
        return Some(WasmImageFormats::from_formats(format));
    }
    None
}
