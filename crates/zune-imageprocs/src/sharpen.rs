/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::gaussian_blur::{gaussian_blur_f32, gaussian_blur_u16, gaussian_blur_u8};
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
        let (width, height) = image.dimensions();

        let depth = image.depth();

        #[cfg(not(feature = "threads"))]
        {
            trace!("Running unsharpen in single threaded mode");

            match depth.bit_type() {
                BitType::U16 => {
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in image.channels_mut(true) {
                        unsharpen_u16(
                            channel.reinterpret_as_mut::<u16>()?,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            self.threshold,
                            self.percentage as u16,
                            width,
                            height,
                            1,
                        );
                    }
                }

                BitType::U8 => {
                    let mut blur_buffer = vec![0; width * height];
                    let mut blur_scratch = vec![0; width * height];

                    for channel in image.channels_mut(true) {
                        unsharpen_u8(
                            channel.reinterpret_as_mut::<u8>()?,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            self.threshold as u8,
                            self.percentage,
                            width,
                            height,
                            1,
                        );
                    }
                }
                BitType::F32 => {
                    let mut blur_buffer = vec![0.0; width * height];
                    let mut blur_scratch = vec![0.0; width * height];
                    for channel in image.channels_mut(true) {
                        unsharpen_f32(
                            channel.reinterpret_as_mut::<f32>()?,
                            &mut blur_buffer,
                            &mut blur_scratch,
                            self.sigma,
                            u8::try_from(self.threshold.clamp(0, 255)).unwrap_or(u8::MAX) as u16,
                            self.percentage as u16,
                            width,
                            height,
                            1,
                        );
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
        }
        #[cfg(feature = "threads")]
        {
            // Calculate how many threads the blur should use internally
            let num_channels = image.channels_ref(true).len();
            let total_cores = std::thread::available_parallelism().map_or(1, |n| n.get());
            let blur_threads = (total_cores / num_channels).max(1);

            trace!("Running unsharpen in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.channels_mut(true) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => {
                            let mut blur_buffer = vec![0; width * height];
                            let mut blur_scratch = vec![0; width * height];

                            unsharpen_u16(
                                channel.reinterpret_as_mut::<u16>()?,
                                &mut blur_buffer,
                                &mut blur_scratch,
                                self.sigma,
                                self.threshold,
                                u16::from(self.percentage),
                                width,
                                height,
                                blur_threads,
                            );
                            Ok(())
                        }

                        BitType::U8 => {
                            let mut blur_buffer = vec![0; width * height];
                            let mut blur_scratch = vec![0; width * height];

                            unsharpen_u8(
                                channel.reinterpret_as_mut::<u8>()?,
                                &mut blur_buffer,
                                &mut blur_scratch,
                                self.sigma,
                                u8::try_from(self.threshold.clamp(0, 255)).unwrap_or(u8::MAX),
                                self.percentage,
                                width,
                                height,
                                blur_threads,
                            );
                            Ok(())
                        }
                        BitType::F32 => {
                            let mut blur_buffer = vec![0.0; width * height];
                            let mut blur_scratch = vec![0.0; width * height];

                            unsharpen_f32(
                                channel.reinterpret_as_mut::<f32>()?,
                                &mut blur_buffer,
                                &mut blur_scratch,
                                self.sigma,
                                u8::try_from(self.threshold.clamp(0, 255)).unwrap_or(u8::MAX)
                                    as u16,
                                self.percentage as u16,
                                width,
                                height,
                                blur_threads,
                            );
                            Ok(())
                        }
                        d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16,BitType::F32]
    }
}

#[allow(clippy::too_many_arguments)]
fn unsharpen_u8(
    channel: &mut [u8], blur_buffer: &mut [u8], blur_scratch_buffer: &mut [u8], sigma: f32,
    threshold: u8, percentage: u8, width: usize, height: usize, blur_threads: usize,
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u8(blur_buffer, blur_scratch_buffer, width, height, sigma, blur_threads);

    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let orig = i32::from(*in_pix);
        let blurred = i32::from(*blur_pix);

        // Signed difference allows us to lighten OR darken the pixel
        let diff = orig - blurred;

        // Check against threshold using absolute magnitude
        if diff.abs() > thresh {
            // Apply the percentage intensity
            let scaled_diff = (diff * pct) / 100;

            // Add back to original and clamp to valid u8 range
            let new_val = orig + scaled_diff;
            *in_pix = new_val.clamp(0, 255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn unsharpen_u16(
    channel: &mut [u16], blur_buffer: &mut [u16], blur_scratch_buffer: &mut [u16], sigma: f32,
    threshold: u16, percentage: u16, width: usize, height: usize, blur_threads: usize,
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u16(
        blur_buffer,
        blur_scratch_buffer,
        width,
        height,
        sigma,
        blur_threads,
    );

    let pct = i32::from(percentage);
    let thresh = i32::from(threshold);

    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let orig = i32::from(*in_pix);
        let blurred = i32::from(*blur_pix);

        // Signed difference allows us to lighten OR darken the pixel
        let diff = orig - blurred;

        if diff.abs() > thresh {
            // Apply the percentage intensity
            let scaled_diff = (diff * pct) / 100;

            // Add back to original and clamp to valid u16 range
            let new_val = orig + scaled_diff;
            *in_pix = new_val.clamp(0, 65535) as u16;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn unsharpen_f32(
    channel: &mut [f32], blur_buffer: &mut [f32], blur_scratch_buffer: &mut [f32], sigma: f32,
    threshold: u16, percentage: u16, width: usize, height: usize, blur_threads: usize,
) {
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_f32(
        blur_buffer,
        blur_scratch_buffer,
        width,
        height,
        sigma,
        blur_threads,
    );

    let pct = f32::from(percentage);
    let thresh = f32::from(threshold);

    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter()) {
        let orig = f32::from(*in_pix);
        let blurred = f32::from(*blur_pix);

        // Signed difference allows us to lighten OR darken the pixel
        let diff = orig - blurred;

        if diff.abs() > thresh {
            // Apply the percentage intensity
            let scaled_diff = (diff * pct) / 100.00;

            // Add back to original and clamp to valid u16 range
            let new_val = orig + scaled_diff;
            *in_pix = new_val.clamp(0.0, 1.0);
        }
    }
}
