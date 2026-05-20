/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use crate::gaussian_blur::GaussianBlur;
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::OperationsTrait;

/// Sharpens an image using the Unsharp Mask algorithm.
///
/// Despite its name, the Unsharp Mask is the industry standard for *sharpening* images.
/// It works by creating a blurred (unsharp) copy of the image, finding the differences
/// between the blurred copy and the original, and then adding a percentage of those
/// differences back to the original image to enhance local contrast and edges.
///
/// # Algorithm
///
/// For each pixel, the filter evaluates the difference between the original image ($I$)
/// and the Gaussian blurred image ($B$). If the absolute difference is greater than the
/// `threshold`, the pixel is sharpened:
/// $$Output = I + (I - B) \cdot \frac{Percentage}{100}$$
///
/// # Parameters
///
/// * `sigma` - Controls the radius/spread of the Gaussian blur. A larger sigma thickens the sharpened edges.
/// * `threshold` - The minimum brightness difference required before a pixel is sharpened.
///   Raising this prevents the filter from amplifying flat noise (like film grain or JPEG artifacts).
/// * `percentage` - The strength of the sharpening effect. `100` means 100% of the difference is added back.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::sharpen::Sharpen;
/// use zune_image::errors::ImageErrors;
///
/// let mut img = Image::fill(128_u8, ColorSpace::RGB, 100, 100);
///
/// // Sharpen with a sigma of 1.0, a threshold of 10, and a strength of 150%
/// let sharpen = Sharpen::new(1.0, 10, 150);
/// sharpen.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct Sharpen {
    sigma: f32,
    threshold: u16,
    percentage: u8,
}

impl Sharpen {
    /// Create a new unsharp mask
    ///
    /// # Arguments
    /// - sigma: This value is passed to the gaussian filter,consult [it's documentation](crate::gaussian_blur::GaussianBlur)
    /// on how to use it
    ///
    /// - threshold: If the result of the blur and the initial image is greater than this,  add the difference, otherwise
    /// skip
    ///  - percentage: `threshold*percentage`
    ///
    #[must_use]
    pub fn new(sigma: f32, threshold: u16, percentage: u8) -> Sharpen {
        Sharpen {
            sigma,
            threshold,
            percentage,
        }
    }
}

impl OperationsTrait for Sharpen {
    fn name(&self) -> &'static str {
        "Unsharpen"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();

        // 1. Create the blurred copy utilizing our existing optimized operation
        trace!("Generating unsharp mask via GaussianBlur");
        let mut blur_image = image.clone();
        let blur_op = GaussianBlur::new(self.sigma);
        blur_op.execute_impl(&mut blur_image)?;

        // 2. Blend the blurred image and the original image in parallel
        // We set ignore_alpha to true, as sharpening should not affect transparency
        trace!("Blending unsharp mask with original image");
        let ignore_alpha = true;

        match depth.bit_type() {
            BitType::U8 => {
                // blur_image acts as the immutable "src", image acts as the mutable "dest"
                blur_image.par_process_regions_out_of_place::<u8, _>(
                    image,
                    ignore_alpha,
                    |region| {
                        unsharpen_region_u8(region, self.threshold as u8, self.percentage);
                    },
                )?;
            }
            BitType::U16 => {
                blur_image.par_process_regions_out_of_place::<u16, _>(
                    image,
                    ignore_alpha,
                    |region| {
                        unsharpen_region_u16(region, self.threshold, u16::from(self.percentage));
                    },
                )?;
            }
            BitType::F32 => {
                blur_image.par_process_regions_out_of_place::<f32, _>(
                    image,
                    ignore_alpha,
                    |region| {
                        let f32_thresh = f32::from(
                            u8::try_from(self.threshold.clamp(0, 255)).unwrap_or(u8::MAX),
                        );
                        unsharpen_region_f32(region, f32_thresh, f32::from(self.percentage));
                    },
                )?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn unsharpen_region_u8(region: &mut PlanarRegionOut<'_, u8>, threshold: u8, percentage: u8) {
    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    // Calculate 1D array bounds for this chunk
    let start_idx = region.y_offset * region.width;
    let end_idx = start_idx + (region.height * region.width);

    for (blur_full, orig_chunk) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        if blur_full.len() >= end_idx {
            // Slice the global blurred image down to match the local original chunk
            let blur_chunk = &blur_full[start_idx..end_idx];

            for (b_pix, in_pix) in blur_chunk.iter().zip(orig_chunk.iter_mut()) {
                let orig = i32::from(*in_pix);
                let blurred = i32::from(*b_pix);
                let diff = orig - blurred;

                if diff.abs() > thresh {
                    let scaled_diff = (diff * pct) / 100;
                    *in_pix = (orig + scaled_diff).clamp(0, 255) as u8;
                }
            }
        }
    }
}

fn unsharpen_region_u16(region: &mut PlanarRegionOut<'_, u16>, threshold: u16, percentage: u16) {
    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    let start_idx = region.y_offset * region.width;
    let end_idx = start_idx + (region.height * region.width);

    for (blur_full, orig_chunk) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        if blur_full.len() >= end_idx {
            let blur_chunk = &blur_full[start_idx..end_idx];

            for (b_pix, in_pix) in blur_chunk.iter().zip(orig_chunk.iter_mut()) {
                let orig = i32::from(*in_pix);
                let blurred = i32::from(*b_pix);
                let diff = orig - blurred;

                if diff.abs() > thresh {
                    let scaled_diff = (diff * pct) / 100;
                    *in_pix = (orig + scaled_diff).clamp(0, 65535) as u16;
                }
            }
        }
    }
}

fn unsharpen_region_f32(region: &mut PlanarRegionOut<'_, f32>, threshold: f32, percentage: f32) {
    let start_idx = region.y_offset * region.width;
    let end_idx = start_idx + (region.height * region.width);

    for (blur_full, orig_chunk) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        if blur_full.len() >= end_idx {
            let blur_chunk = &blur_full[start_idx..end_idx];

            for (b_pix, in_pix) in blur_chunk.iter().zip(orig_chunk.iter_mut()) {
                let orig = *in_pix;
                let blurred = *b_pix;
                let diff = orig - blurred;

                if diff.abs() > threshold {
                    let scaled_diff = (diff * percentage) / 100.0;
                    *in_pix = orig + scaled_diff;
                }
            }
        }
    }
}
