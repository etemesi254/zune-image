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
use std::time::Instant;

use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::ColorCharacteristics;
use zune_core::log::trace;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;

use crate::image_transfer::{ConversionType, ImageTransfer, TransferFunction};
use crate::premul_alpha::PremultiplyAlpha;
use crate::resize::seperable_kernel::{resample_separable_u8, PrecomputedKernels};
use crate::traits::NumOps;
use crate::utils::execute_on;

mod bilinear;
mod seperable_kernel;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResizeMethod {
    Lanczos3,   // Lanczos with a=3 (highest quality, slowest)
    Lanczos2,   // Lanczos with a=2
    Bicubic,    // Bicubic interpolation (Mitchell-Netravali, B=1/3, C=1/3)
    CatmullRom, // Catmull-Rom spline (B=0, C=0.5)
    Mitchell,   // Mitchell filter (B=1/3, C=1/3) - same as Bicubic but explicit
    BSpline,    // B-Spline (B=1, C=0)
    Hermite,    // Hermite filter (B=0, C=0)
    Sinc,       // Sinc with window radius 3
    Bilinear,   // Bilinear (for completeness, 2x2 kernel)
}
/// Resize dimensions
#[derive(Copy, Clone, Debug)]
pub enum ResizeDimensions {
    /// e.g., "50%" or "50%x75%"
    Percentage(usize, usize),
    /// e.g., "800x600!" (Ignore aspect ratio entirely)
    IgnoreAspectRatio(usize, usize),
    /// e.g., "800x600" (Fit within bounds, keep aspect ratio)
    FitWithin(usize, usize),
    /// e.g., "800x600^" (Fill bounds, keep aspect ratio)
    Fill(usize, usize),
    /// e.g., "800" (Width only)
    WidthOnly(usize),
    /// e.g., "x600" (Height only)
    HeightOnly(usize),
    /// e.g., "800x600>"
    ShrinkToFit(usize, usize),
    /// e.g., "800x600<"
    EnlargeToFit(usize, usize),
    /// e.g., "40000@"
    Area(usize),
}
/// Resize an image to a new width and height
/// using the resize method specified
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

    #[allow(clippy::too_many_lines)]
    #[allow(unused_variables)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        // For any resize always convert gamma to linear.
        // But check if that is the current image format
        let is_image_linear = image.metadata().color_trc() == Some(ColorCharacteristics::Linear);

        let transfer_function = image
            .metadata()
            .color_trc()
            .unwrap_or(ColorCharacteristics::sRGB);

        if !is_image_linear {
            let start = Instant::now();
            trace!("Converting image to linear along resize method");
            let transfers = ImageTransfer::new(
                TransferFunction::from(transfer_function),
                ConversionType::GammaToLinear,
            );
            transfers.execute_impl(image)?;
            let duration = start.elapsed();
            trace!("Image conversion to linear successfully completed in {duration:.2?}");
        }
        // if alpha is present premultiply
        let is_premultiplied = image.metadata().is_premultiplied_alpha();
        let has_alpha = image.colorspace().has_alpha();
        let original_depth = image.depth();
        if !is_premultiplied && has_alpha {
            // pre multiply looses color if its in u8
            image.convert_depth(BitDepth::Float32)?;
            trace!("Premultiplying alpha along resize method");
            let start = Instant::now();
            PremultiplyAlpha::new(AlphaState::PreMultiplied).execute_impl(image)?;
            let duration = start.elapsed();
            trace!("Premultiply successfully completed in {duration:.2?}");
        }
        let (old_w, old_h) = image.dimensions();
        let depth = image.depth().bit_type();

        let (new_w, new_h) = calc_absolute_dimensions(self.dimensions, image);

        trace!("Resize dims -> width:{new_w} height:{new_h}");

        let new_length = new_w * new_h * image.depth().size_of();

        let precomputed_kernels = if self.method == ResizeMethod::Bilinear {
            None
        } else {
            Some(PrecomputedKernels::new(
                old_w,
                old_h,
                new_w,
                new_h,
                self.method,
            ))
        };

        let resize_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut new_channel = Channel::new_with_bit_type(new_length, depth);
            match depth {
                BitType::U8 => {
                    if self.method == ResizeMethod::Bilinear {
                        // will short circuit to the right one
                        resize::<u8>(
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?,
                            self.method,
                            old_w,
                            old_h,
                            new_w,
                            new_h,
                            precomputed_kernels.as_ref(),
                        );
                    } else {
                        resample_separable_u8(
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?,
                            old_w,
                            old_h,
                            new_w,
                            new_h,
                            precomputed_kernels.as_ref().unwrap(),
                        );
                    }
                }
                BitType::U16 => resize::<u16>(
                    channel.reinterpret_as()?,
                    new_channel.reinterpret_as_mut()?,
                    self.method,
                    old_w,
                    old_h,
                    new_w,
                    new_h,
                    precomputed_kernels.as_ref(),
                ),

                BitType::F32 => {
                    resize::<f32>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        self.method,
                        old_w,
                        old_h,
                        new_w,
                        new_h,
                        precomputed_kernels.as_ref(),
                    );
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented("resize", d)),
            }
            *channel = new_channel;
            Ok(())
        };
        execute_on(resize_fn, image, false)?;
        image.set_dimensions(new_w, new_h);

        // convert back from premultiplied if we did not get the
        // image as premultiplied
        if !is_premultiplied && has_alpha {
            trace!("Un-premultiplying alpha along resize method");
            let start = Instant::now();
            PremultiplyAlpha::new(AlphaState::NonPreMultiplied).execute_impl(image)?;
            // return the depth originally there
            image.convert_depth(original_depth)?;
            let duration = start.elapsed();
            trace!("Un-premultiply successfully completed in {duration:.2?}");
        }
        // convert back again to gamma
        if !is_image_linear {
            let start = Instant::now();
            trace!("Converting image back to gamma along resize method");
            let transfers = ImageTransfer::new(
                TransferFunction::from(transfer_function),
                ConversionType::LinearToGamma,
            );
            transfers.execute_impl(image)?;
            let duration = start.elapsed();
            trace!("Image conversion to gamma successfully completed in {duration:.2?}");
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
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
/// Resize an image **channel** to new dimensions
///
/// # Arguments
/// - in_image: A contiguous slice of a single channel of an image
/// - out_image: Where we will store the new resized pixels
/// - method: The resizing method to use
/// - in_width: `in_image`'s width
/// - in_height:  `in_image`'s height.
/// - out_width: The expected width
/// - out_height: The expected height.
/// # Panics
/// - `in_width*in_height` do not match `in_image.len()`.
/// - `out_width*out_height` do not match `out_image.len()`.
#[allow(clippy::too_many_arguments)]
fn resize<T>(
    in_image: &[T], out_image: &mut [T], method: ResizeMethod, in_width: usize, in_height: usize,
    out_width: usize, out_height: usize, precomputed_kernels: Option<&PrecomputedKernels>,
) where
    T: Copy + NumOps<T> + Default,
    f32: std::convert::From<T>,
{
    match method {
        ResizeMethod::Bilinear => {
            bilinear::bilinear_impl(
                in_image, out_image, in_width, in_height, out_width, out_height,
            );
        }

        _ => match precomputed_kernels {
            Some(precomputed_kernels) => {
                seperable_kernel::resample_separable(
                    in_image,
                    out_image,
                    in_width,
                    in_height,
                    out_width,
                    out_height,
                    precomputed_kernels,
                );
            }
            None => {
                unreachable!("Precomputed kernels not loaded");
            }
        },
    }
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
        ResizeDimensions::IgnoreAspectRatio(w, h) => (w, h),

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
