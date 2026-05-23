/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Resize an image to a new width and height
//!
//!
use std::cmp::PartialEq;

use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::ColorCharacteristics;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::frame::Frame;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::premul_alpha::PremultiplyAlpha;
use crate::resize::seperable_kernel::{
    resample_region_generic, resample_region_u8, PrecomputedKernels,
};
use crate::transfer_curve::{ConversionType, TransferCurve, TransferFunction};

pub(crate) mod seperable_kernel;

/// Resampling algorithms available for image resizing.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResizeMethod {
    /// Lanczos filter with a window of 3. Provides the highest quality and sharpest
    /// results for both upscaling and downscaling, but is the slowest.
    Lanczos3,
    /// Lanczos filter with a window of 2. A slightly faster, slightly softer alternative to Lanczos3.
    Lanczos2,
    /// Bicubic interpolation (Mitchell-Netravali). A good balance of speed and quality.
    Bicubic,
    /// Catmull-Rom spline. Produces sharp edges without the ringing artifacts sometimes seen in Lanczos.
    CatmullRom,
    /// Mitchell filter. An alias for Bicubic interpolation.
    Mitchell,
    /// B-Spline interpolation. Produces very smooth/soft results.
    BSpline,
    /// Hermite filter. Fast, but relatively soft.
    Hermite,
    /// Sinc filter with a window radius of 3.
    Sinc,
    /// Bilinear interpolation. Very fast, but produces blurry results when upscaling
    /// and aliasing artifacts when downscaling.
    Bilinear,
    //MagicKernelSharp
}

/// Geometry constraints for calculating the final resized dimensions.
#[derive(Copy, Clone, Debug)]
pub enum ResizeDimensions {
    /// Resizes the image by a percentage of its original width and height (e.g., `50`, `75`).
    Percentage(usize, usize),
    /// Forces the image to exactly these dimensions, ignoring the original aspect ratio.
    Exact(usize, usize),
    /// Scales the image to fit entirely within the specified bounding box while maintaining aspect ratio.
    FitWithin(usize, usize),
    /// Scales the image so that it completely covers the bounding box while maintaining aspect ratio (some parts of the image may overflow the box).
    Fill(usize, usize),
    /// Resizes the image to the target width; the height is calculated automatically to maintain aspect ratio.
    WidthOnly(usize),
    /// Resizes the image to the target height; the width is calculated automatically to maintain aspect ratio.
    HeightOnly(usize),
    /// Fits the image within the bounding box, but *only* if the image is larger than the box.
    ShrinkToFit(usize, usize),
    /// Fits the image within the bounding box, but *only* if the image is smaller than the box.
    EnlargeToFit(usize, usize),
    /// Scales the image so its total pixel count matches the specified area, maintaining aspect ratio.
    Area(usize),
}
/// Resizes an image to new dimensions using a specified resampling algorithm.
///
/// # Color Accuracy
///
/// To prevent visual artifacts (such as dark fringing or color shifting), this operation
/// automatically performs mathematical resampling in a **linear color space**.
///
/// 1. If the image is gamma-encoded (e.g., sRGB), it is temporarily linearized.
/// 2. If the image has an alpha channel, it is temporarily converted to **premultiplied alpha**.
/// 3. The image is resampled.
/// 4. The image is converted back to its original gamma and alpha state.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::resize::{Resize, ResizeDimensions, ResizeMethod};
/// use zune_image::errors::ImageErrors;
///
/// let mut img = Image::fill(255_u8, ColorSpace::RGB, 1000, 1000);
///
/// // Resize the image to fit within a 500x500 box using high-quality Lanczos3
/// let resize = Resize::new(ResizeDimensions::FitWithin(500, 500), ResizeMethod::Lanczos3);
/// resize.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Copy, Clone)]
pub struct Resize {
    dimensions: ResizeDimensions,
    method: ResizeMethod,
}

impl Resize {
    /// Create a new resize operation
    ///
    /// # Argument
    /// - dims: Resize dimensions, either fixed or a percentage
    /// - method: The resize method to use
    #[must_use]
    pub fn new(dims: ResizeDimensions, method: ResizeMethod) -> Resize {
        Resize {
            dimensions: dims,
            method,
        }
    }
}

impl OperationsTrait for Resize {
    fn name(&self) -> &'static str {
        "Resize"
    }

    #[allow(clippy::too_many_lines, unused_variables)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let is_image_linear = image.metadata().color_trc() == Some(ColorCharacteristics::Linear);
        let transfer_function = image
            .metadata()
            .color_trc()
            .unwrap_or(ColorCharacteristics::sRGB);

        if !is_image_linear {
            trace!("Converting image to linear along resize method");
            TransferCurve::new(
                TransferFunction::from(transfer_function),
                ConversionType::GammaToLinear,
            )
            .execute_impl(image)?;
        }

        let is_premultiplied = image.metadata().is_premultiplied_alpha();
        let colorspace = image.colorspace();
        let has_alpha = colorspace.has_alpha();
        let original_depth = image.depth();

        if !is_premultiplied && has_alpha {
            image.convert_depth(BitDepth::Float32)?;
            trace!("Premultiplying alpha along resize method");
            PremultiplyAlpha::new(AlphaState::PreMultiplied).execute_impl(image)?;
        }

        let (old_w, old_h) = image.dimensions();
        let (new_w, new_h) = calc_absolute_dimensions(self.dimensions, image);
        let depth = image.depth().bit_type();

        trace!("Resize dims -> width:{new_w} height:{new_h}");

        let precomputed_kernels = PrecomputedKernels::new(old_w, old_h, new_w, new_h, self.method);

        for frame in image.frames_mut() {
            let src_channels: Vec<&Channel> =
                frame.channels_ref(colorspace, false).iter().collect();
            let num_channels = src_channels.len();

            let new_channels = vec![Channel::new_with_bit_type(new_w * new_h, depth); num_channels];
            let mut dest_frame = Frame::new(new_channels);

            // 2. Dispatch to the frame primitive using the new dimensions
            match depth {
                BitType::U8 => {
                    let src_slices: Vec<&[u8]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();
                    Image::par_process_frame_regions::<u8, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            resample_region_u8(region, &src_slices, old_w, &precomputed_kernels);
                        },
                    )?;
                }
                BitType::U16 => {
                    let src_slices: Vec<&[u16]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();
                    
                    Image::par_process_frame_regions::<u16, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            resample_region_generic::<u16>(
                                region,
                                &src_slices,
                                old_w,
                                &precomputed_kernels,
                            );
                        },
                    )?;
                }
                BitType::F32 => {
                    let src_slices: Vec<&[f32]> = src_channels
                        .iter()
                        .map(|c| c.reinterpret_as().unwrap())
                        .collect();
                    Image::par_process_frame_regions::<f32, _>(
                        &mut dest_frame,
                        new_w,
                        new_h,
                        colorspace,
                        false,
                        |region| {
                            resample_region_generic::<f32>(
                                region,
                                &src_slices,
                                old_w,
                                &precomputed_kernels,
                            );
                        },
                    )?;
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented("resize", d)),
            }

            // 3. Swap the processed, resized channels back into the original frame
            for (old_chan, new_chan) in frame
                .channels_mut(colorspace, false)
                .iter_mut()
                .zip(dest_frame.channels_mut(colorspace, false))
            {
                std::mem::swap(old_chan, new_chan);
            }
        }

        image.set_dimensions(new_w, new_h);

        if !is_premultiplied && has_alpha {
            trace!("Un-premultiplying alpha along resize method");
            PremultiplyAlpha::new(AlphaState::NonPreMultiplied).execute_impl(image)?;
            image.convert_depth(original_depth)?;
        }

        if !is_image_linear {
            trace!("Converting image back to gamma along resize method");
            TransferCurve::new(
                TransferFunction::from(transfer_function),
                ConversionType::LinearToGamma,
            )
            .execute_impl(image)?;
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }
}

/// Return the image resize dimensions that would not cause a distortion
/// taking into consideration the smaller dimension
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn ratio_dimensions_smaller(
    old_w: usize, old_h: usize, new_w: usize, new_h: usize,
) -> (usize, usize) {
    let ratio_w = old_w as f64 / new_w as f64;
    let ratio_h = old_h as f64 / new_h as f64;
    let percent = if ratio_h < ratio_w { ratio_w } else { ratio_h };

    let t = (old_w as f64 / percent) as usize;
    let u = (old_h as f64 / percent) as usize;
    (t, u)
}

/// Return the image resize dimensions that would not cause a distortion
/// taking into consideration the larger dimension
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn ratio_dimensions_larger(
    old_w: usize, old_h: usize, new_w: usize, new_h: usize,
) -> (usize, usize) {
    let ratio_w = old_w as f64 / new_w as f64;
    let ratio_h = old_h as f64 / new_h as f64;
    let percent = if ratio_h < ratio_w { ratio_w } else { ratio_h };

    let t = (old_w as f64 / percent) as usize;
    let u = (old_h as f64 / percent) as usize;
    (t, u)
}

fn calc_absolute_dimensions(resize_dims: ResizeDimensions, image: &Image) -> (usize, usize) {
    let (orig_w, orig_h) = image.dimensions();

    // Prevent division by zero panics on empty/invalid images
    if orig_w == 0 || orig_h == 0 {
        return (orig_w, orig_h);
    }

    let orig_w_f = orig_w as f64;
    let orig_h_f = orig_h as f64;

    let (new_w, new_h) = match resize_dims {
        // Force exact dimensions (ImageMagick `!`)
        ResizeDimensions::Exact(w, h) => (w, h),

        // Percentage math
        ResizeDimensions::Percentage(percent_w, percent_h) => (
            orig_w.saturating_mul(percent_w) / 100,
            orig_h.saturating_mul(percent_h) / 100,
        ),

        // Provide width, calculate height to keep aspect ratio
        ResizeDimensions::WidthOnly(target_w) => {
            let ratio = target_w as f64 / orig_w_f;
            (target_w, (orig_h_f * ratio).round() as usize)
        }

        // Provide height, calculate width to keep aspect ratio
        ResizeDimensions::HeightOnly(target_h) => {
            let ratio = target_h as f64 / orig_h_f;
            ((orig_w_f * ratio).round() as usize, target_h)
        }

        // Fit entirely inside the target box (ImageMagick default)
        ResizeDimensions::FitWithin(target_w, target_h) => {
            let ratio = f64::min(target_w as f64 / orig_w_f, target_h as f64 / orig_h_f);
            (
                (orig_w_f * ratio).round() as usize,
                (orig_h_f * ratio).round() as usize,
            )
        }

        // Scale to completely cover the target box (ImageMagick `^`)
        ResizeDimensions::Fill(target_w, target_h) => {
            let ratio = f64::max(target_w as f64 / orig_w_f, target_h as f64 / orig_h_f);
            (
                (orig_w_f * ratio).round() as usize,
                (orig_h_f * ratio).round() as usize,
            )
        }

        // Fit within box, but only if the image is larger (ImageMagick `>`)
        ResizeDimensions::ShrinkToFit(target_w, target_h) => {
            if orig_w > target_w || orig_h > target_h {
                let ratio = f64::min(target_w as f64 / orig_w_f, target_h as f64 / orig_h_f);
                (
                    (orig_w_f * ratio).round() as usize,
                    (orig_h_f * ratio).round() as usize,
                )
            } else {
                (orig_w, orig_h)
            }
        }

        // Fit within box, but only if the image is smaller (ImageMagick `<`)
        ResizeDimensions::EnlargeToFit(target_w, target_h) => {
            if orig_w < target_w || orig_h < target_h {
                let ratio = f64::min(target_w as f64 / orig_w_f, target_h as f64 / orig_h_f);
                (
                    (orig_w_f * ratio).round() as usize,
                    (orig_h_f * ratio).round() as usize,
                )
            } else {
                (orig_w, orig_h)
            }
        }

        // Target a specific total pixel count (ImageMagick `@`)
        ResizeDimensions::Area(target_area) => {
            let orig_area = (orig_w * orig_h) as f64;
            let ratio = f64::sqrt(target_area as f64 / orig_area);
            (
                (orig_w_f * ratio).round() as usize,
                (orig_h_f * ratio).round() as usize,
            )
        }
    };

    // Failsafe: Ensure we never return a width or height of 0
    // (unless the original image was somehow 0, caught early above).
    (new_w.max(1), new_h.max(1))
}
// #[cfg(feature = "benchmarks")]
// #[cfg(test)]
// mod benchmarks {
//     extern crate test;
//
//     use crate::resize::{resize, ResizeMethod};
//
//     #[bench]
//     fn bench_resize_linear(b: &mut test::Bencher) {
//         let width = 4000;
//         let height = 2000;
//
//         let new_width = 1200;
//         let new_height = 1000;
//
//         let dimensions = width * height;
//
//         let new_dimensions = new_width * new_height;
//
//         let in_vec = vec![255_u16; dimensions];
//         let mut out_vec = vec![255_u16; new_dimensions];
//
//         b.iter(|| {
//             resize(
//                 &in_vec,
//                 &mut out_vec,
//                 ResizeMethod::Bilinear,
//                 width,
//                 height,
//                 new_width,
//                 new_height
//             );
//         });
//     }
//     #[bench]
//     fn bench_resize_cubic(b: &mut test::Bencher) {
//         let width = 4000;
//         let height = 2000;
//
//         let new_width = 1500;
//         let new_height = 1000;
//
//         let dimensions = width * height;
//
//         let new_dimensions = new_width * new_height;
//
//         let in_vec = vec![255_u16; dimensions];
//         let mut out_vec = vec![255_u16; new_dimensions];
//
//         b.iter(|| {
//             resize(
//                 &in_vec,
//                 &mut out_vec,
//                 ResizeMethod::Bicubic,
//                 width,
//                 height,
//                 new_width,
//                 new_height
//             );
//         });
//     }
//     #[bench]
//     fn bench_resize_lancazos(b: &mut test::Bencher) {
//         let width = 4000;
//         let height = 2000;
//
//         let new_width = 1500;
//         let new_height = 1000;
//
//         let dimensions = width * height;
//
//         let new_dimensions = new_width * new_height;
//
//         let in_vec = vec![255_u16; dimensions];
//         let mut out_vec = vec![255_u16; new_dimensions];
//
//         b.iter(|| {
//             resize(
//                 &in_vec,
//                 &mut out_vec,
//                 ResizeMethod::Lanczos3,
//                 width,
//                 height,
//                 new_width,
//                 new_height
//             );
//         });
//     }
// }
// #[cfg(test)]
// mod tests {
//     use crate::resize::{resize, ResizeMethod};
//
//     #[test]
//     fn bench_resize_cubic() {
//         let width = 4000;
//         let height = 2000;
//
//         let new_width = 1500;
//         let new_height = 1500;
//
//         let dimensions = width * height;
//
//         let new_dimensions = new_width * new_height;
//
//         let in_vec = vec![255_u16; dimensions];
//         let mut out_vec = vec![255_u16; new_dimensions];
//
//
//         resize(
//             &in_vec,
//             &mut out_vec,
//             ResizeMethod::Bicubic,
//             width,
//             height,
//             new_width,
//             new_height
//         );
//     }
//     #[test]
//     fn test_resize_lancazos() {
//         let width = 400;
//         let height = 200;
//
//         let new_width = 1500;
//         let new_height = 1500;
//
//         let dimensions = width * height;
//
//         let new_dimensions = new_width * new_height;
//
//         let in_vec = vec![255_u16; dimensions];
//         let mut out_vec = vec![255_u16; new_dimensions];
//
//         resize(
//             &in_vec,
//             &mut out_vec,
//             ResizeMethod::Lanczos3,
//             width,
//             height,
//             new_width,
//             new_height
//         );
//     }
// }
