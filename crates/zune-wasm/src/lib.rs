/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use wasm_bindgen::prelude::*;
use zune_core::bytestream::ZCursor;
use zune_core::log::{debug, info};
use zune_core::options::DecoderOptions;

use crate::custom_operations::composite::WasmOverlayOp;
use crate::enums::{
    WasmColorProfiles, WasmColorspace, WasmCompositeMethod, WasmFlipDirection, WasmImageFormats, WasmResizeMethod, WasmSpatialOperations, WasmThresholdMethod,
};
use crate::utils::set_panic_hook;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::pipelines::Pipeline;
use zune_imageprocs::auto_orient::AutoOrient;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::blur::Blur;
use zune_imageprocs::box_blur::BoxBlur;
use zune_imageprocs::brighten::Brighten;
use zune_imageprocs::color_matrix::ColorMatrix;
use zune_imageprocs::color_transform::ColorTransform;
use zune_imageprocs::contrast::Contrast;
use zune_imageprocs::convolve::Convolve;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::{Flip, FlipDirection};
use zune_imageprocs::fx::Fx;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::hsv_adjust::HsvAdjust;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::median::MedianBlur;
use zune_imageprocs::resize::{Resize, ResizeDimensions};
use zune_imageprocs::rotate::Rotate;
use zune_imageprocs::sharpen::Sharpen;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::threshold::{Threshold, ThresholdMethod};
use zune_imageprocs::transpose::Transpose;

mod custom_operations;
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

// ... your other imports

#[wasm_bindgen]
pub struct WasmImage {
    pipeline: Pipeline,
}
#[wasm_bindgen]
impl WasmImage {
    /// Create a new image from in-memory bytes
    #[wasm_bindgen(constructor)]
    pub fn from_bytes(bytes: &[u8]) -> Result<WasmImage, JsError> {
        let image = Image::read(ZCursor::from(bytes), DecoderOptions::new_fast())
            .map_err(<ImageErrors as Into<JsError>>::into)?;

        let mut pipeline = Pipeline::new();
        pipeline.chain_image(image);

        Ok(WasmImage { pipeline })
    }
}

#[wasm_bindgen]
impl WasmImage {
    /// Auto orient the image based on EXIF tag
    pub fn auto_orient(mut self) -> WasmImage {
        self.pipeline.chain_operations(Box::new(AutoOrient));
        self
    }

    /// Apply a bilateral filter to reduce noise while preserving edges
    pub fn bilateral_filter(mut self, d: i32, sigma_color: f32, sigma_space: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(BilateralFilter::new(d, sigma_color, sigma_space)));
        self
    }

    /// Apply a box blur to the image
    pub fn box_blur(mut self, radius: usize) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(BoxBlur::new(radius)));
        self
    }

    /// Adjust the brightness of the image
    pub fn brighten(mut self, by: f32) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Brighten::new(by)));
        self
    }

    /// Apply a color matrix operation.
    /// Takes a flat array of 20 floats and converts it to the 4x5 matrix.
    pub fn color_matrix(mut self, matrix: &[f32]) -> Result<WasmImage, JsError> {
        let op = ColorMatrix::try_from_slice(matrix)
            .ok_or_else(|| JsError::new("Length of matrix must be exactly 20"))?;
        self.pipeline.chain_operations(Box::new(op));
        Ok(self)
    }

    /// Flip the image
    pub fn flip(mut self, direction: WasmFlipDirection) -> WasmImage {
        // Map your WASM enum to the internal FlipDirection
        let dir = match direction {
            WasmFlipDirection::Horizontal => FlipDirection::Horizontal,
            WasmFlipDirection::Vertical => FlipDirection::Vertical,
        };
        self.pipeline.chain_operations(Box::new(Flip::new(dir)));
        self
    }

    /// Transform the image's colors to match a target ICC color profile
    pub fn color_transform(mut self, target_profile: WasmColorProfiles) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(ColorTransform::new(target_profile.into())));
        self
    }

    /// Apply a 2D convolution matrix to the image
    pub fn convolve(mut self, weights: &[f32], scale: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Convolve::new(weights.to_vec(), scale)));
        self
    }

    /// Adjust the contrast of the image
    pub fn contrast(mut self, contrast: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Contrast::new(contrast)));
        self
    }

    /// Crop the image to a specified rectangular region
    pub fn crop(mut self, width: usize, height: usize, x: usize, y: usize) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Crop::new(width, height, x, y)));
        self
    }

    /// Adjust the exposure and black level of the image
    pub fn exposure(mut self, exposure: f32, black: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Exposure::new(exposure, black)));
        self
    }

    /// Apply a custom mathematical expression to every pixel
    pub fn fx(mut self, expression: String) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Fx::new(expression)));
        self
    }

    /// Apply gamma correction
    pub fn gamma(mut self, value: f32) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Gamma::new(value)));
        self
    }

    /// Apply a fast Gaussian blur
    pub fn gaussian_blur(mut self, sigma: f32) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Blur::new(sigma)));
        self
    }

    /// Adjust Hue, Saturation, and Value
    pub fn hsv_adjust(mut self, hue: f32, saturation: f32, lightness: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(HsvAdjust::new(hue, saturation, lightness)));
        self
    }

    /// Invert the colors of the image
    pub fn invert(mut self) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Invert));
        self
    }

    /// Apply a median filter to reduce noise
    pub fn median_blur(mut self, radius: usize) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(MedianBlur::new(radius)));
        self
    }

    /// Resize the image to exact dimensions
    pub fn resize(mut self, width: usize, height: usize, method: WasmResizeMethod) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Resize::new(
            ResizeDimensions::Exact(width, height),
            method.into(),
        )));
        self
    }

    /// Rotate the image by an arbitrary angle in degrees
    pub fn rotate(mut self, angle: f32, bg_color: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Rotate::new_with_bg_color(angle, bg_color)));
        self
    }

    /// Sharpen the image using an Unsharp Mask
    pub fn sharpen(mut self, sigma: f32, threshold: u16, percentage: u8) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(Sharpen::new(sigma, threshold, percentage)));
        self
    }

    /// Linearly stretches the contrast of the image
    pub fn stretch_contrast(mut self, lower: f32, upper: f32) -> WasmImage {
        self.pipeline
            .chain_operations(Box::new(StretchContrast::new(lower, upper)));
        self
    }

    /// Apply a fixed-level threshold to the image
    pub fn threshold(mut self, threshold: f32, method: WasmThresholdMethod) -> WasmImage {
        let m = match method {
            WasmThresholdMethod::Binary => ThresholdMethod::Binary,
            WasmThresholdMethod::BinaryInv => ThresholdMethod::BinaryInv,
            WasmThresholdMethod::ThreshTrunc => ThresholdMethod::ThreshTrunc,
            WasmThresholdMethod::ThreshToZero => ThresholdMethod::ThreshToZero,
        };
        self.pipeline
            .chain_operations(Box::new(Threshold::new(threshold, m)));
        self
    }

    /// Transpose the image (swap rows and columns)
    pub fn transpose(mut self) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Transpose::new()));
        self
    }

    /// Apply a Sobel edge detection filter
    pub fn sobel(mut self) -> WasmImage {
        self.pipeline.chain_operations(Box::new(Sobel::new()));
        self
    }

    /// Execute the pipeline and return the encoded bytes
    pub fn to_buffer(mut self, format: WasmImageFormats) -> Result<Vec<u8>, JsError> {
        self.pipeline
            .advance_to_end()
            .map_err(<ImageErrors as Into<JsError>>::into)?;

        let images = self.pipeline.images();
        let img = images
            .first()
            .ok_or_else(|| JsError::new("No image found in pipeline"))?;

        let mut dest = Vec::with_capacity(1024 * 1024); // Start with 1MB capacity to prevent early reallocations
        img.encode(format.to_format(), &mut dest)
            .map_err(<ImageErrors as Into<JsError>>::into)?;

        Ok(dest)
    }
}

impl WasmImage {
    /// Internal helper to guarantee an image is loaded into the pipeline
    /// without running any of the queued processing operations.
    fn ensure_image(&mut self) -> Result<(), JsError> {
        if self.pipeline.images().is_empty() {
            // No image present. We attempt to advance the pipeline one step.
            // If the pipeline is in the `Decode` state and has decoders queued,
            // this will execute them and populate the `image` vector.
            self.pipeline
                .advance()
                .map_err(<ImageErrors as Into<JsError>>::into)?;

            // Check again to see if advancing actually produced an image
            if self.pipeline.images().is_empty() {
                return Err(JsError::new(
                    "Cannot read metadata: Pipeline has no image and no decoders queued.",
                ));
            }
        }
        Ok(())
    }
}

#[wasm_bindgen]
impl WasmImage {
    /// Return the original width of the image
    pub fn width(&mut self) -> Result<usize, JsError> {
        self.ensure_image()?;

        let img = self.pipeline.images().first().unwrap();
        let (width, _) = img.dimensions();
        Ok(width)
    }

    /// Return the original height of the image
    pub fn height(&mut self) -> Result<usize, JsError> {
        self.ensure_image()?;

        let img = self.pipeline.images().first().unwrap();
        let (_, height) = img.dimensions();
        Ok(height)
    }

    /// Return the image's original colorspace
    pub fn colorspace(&mut self) -> Result<WasmColorspace, JsError> {
        self.ensure_image()?;

        let img = self.pipeline.images().first().unwrap();
        Ok(WasmColorspace::from_colorspace(img.colorspace()))
    }

    /// Returns `true` if the original image has an alpha channel, `false` otherwise
    pub fn has_alpha(&mut self) -> Result<bool, JsError> {
        self.ensure_image()?;

        let img = self.pipeline.images().first().unwrap();
        Ok(img.colorspace().has_alpha())
    }
}

#[wasm_bindgen]
impl WasmImage {
    /// Composite another image over this one
    ///
    /// @param overlay_bytes - The encoded bytes of the image to place on top
    /// @param method - The blend mode or Porter-Duff operator to use
    /// @param x - The x-coordinate to place the overlay
    /// @param y - The y-coordinate to place the overlay
    pub fn composite(
        mut self, overlay_bytes: &[u8], method: WasmCompositeMethod, x: usize, y: usize,
    ) -> Result<WasmImage, JsError> {
        // Eagerly decode the overlay image so it is ready
        let overlay = Image::read(ZCursor::from(overlay_bytes), DecoderOptions::new_fast())
            .map_err(<ImageErrors as Into<JsError>>::into)?;

        // Queue the isolated composite operation
        let op = WasmOverlayOp::new(overlay, method.into(), x, y);
        self.pipeline.chain_operations(Box::new(op));

        Ok(self)
    }
}
