use std::borrow::Cow;
use std::fmt::Write;
use std::sync::{Arc, Mutex};
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::gaussian_blur::gaussian_blur_f32;

/// Structural Similarity Index Measure (SSIM)
///
/// A non-destructive metric operation. Computes the MSSIM between the top
/// two images on the stack and stores the result in `output_score`.
pub struct SsimDetection {
    sigma: f32,
    pub output_scores: Arc<Mutex<Option<(Vec<f32>, f32)>>>,
}

impl SsimDetection {
    #[must_use]
    pub fn new() -> Self {
        SsimDetection {
            sigma: 1.5,
            output_scores: Default::default(),
        }
    }

    #[must_use] 
    pub fn get_output_ptr(&self) -> Arc<Mutex<Option<(Vec<f32>, f32)>>> {
        Arc::clone(&self.output_scores)
    }
}

impl OperationsTrait for SsimDetection {
    fn name(&self) -> &'static str {
        "SSIM Detection"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "SSIM requires multiple images; call via execute_multiple",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    #[allow(clippy::too_many_lines)]
    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.len() < 2 {
            return Err(ImageErrors::GenericStr("SSIM requires at least two images"));
        }

        let len = images.len();
        let img2 = &images[len - 1]; // Top (Test)
        let img1 = &images[len - 2]; // Under top (Reference)

        if img1.dimensions() != img2.dimensions() {
            return Err(ImageErrors::GenericStr(
                "SSIM requires images of identical dimensions",
            ));
        }
        let bit_type_img1 = img1.depth().bit_type();
        let bit_type_img2 = img2.depth().bit_type();

        let (width, height) = img1.dimensions();
        let pixel_count = width * height;

        // if calculating s
        // Constants for SSIM stability (Assuming F32 range 0.0 to 255.0)
        let l = 255.0_f32;
        let c1_const = (0.01 * l).powi(2);
        let c2_const = (0.03 * l).powi(2);
        let sigma = self.sigma; // Copy for the closure

        // We determine the max value of the bit depth to correctly set the SSIM "L" constant
        // L is the dynamic range of the pixel values.

        // We wrap the heavy math in a closure so we don't duplicate it
        // across the thread/no-thread cfg branches.
        let compute_channel_ssim =
            |c1_ref: &Channel, c2_ref: &Channel| -> Result<f32, ImageErrors> {
                // PROMOTE TO F32 SMARTLY: Borrow if F32, Allocate if U8/U16
                let x_cow: Cow<[f32]> = match bit_type_img1 {
                    BitType::U8 => Cow::Owned(
                        c1_ref
                            .reinterpret_as::<u8>()?
                            .iter()
                            .map(|&v| f32::from(v))
                            .collect(),
                    ),
                    BitType::U16 => Cow::Owned(
                        c1_ref
                            .reinterpret_as::<u16>()?
                            .iter()
                            .map(|&v| f32::from(v))
                            .collect(),
                    ),
                    BitType::F32 => Cow::Borrowed(c1_ref.reinterpret_as::<f32>()?),
                    _ => unreachable!(),
                };

                let y_cow: Cow<[f32]> = match bit_type_img2 {
                    BitType::U8 => Cow::Owned(
                        c2_ref
                            .reinterpret_as::<u8>()?
                            .iter()
                            .map(|&v| f32::from(v))
                            .collect(),
                    ),
                    BitType::U16 => Cow::Owned(
                        c2_ref
                            .reinterpret_as::<u16>()?
                            .iter()
                            .map(|&v| f32::from(v))
                            .collect(),
                    ),
                    BitType::F32 => Cow::Borrowed(c2_ref.reinterpret_as::<f32>()?),
                    _ => unreachable!(),
                };

                let mut scratch = vec![0.0; pixel_count];

                // 1. Calculate Means
                let mut mu_x = x_cow.to_vec();
                gaussian_blur_f32(&mut mu_x, &mut scratch, width, height, sigma);

                let mut mu_y = y_cow.to_vec();
                gaussian_blur_f32(&mut mu_y, &mut scratch, width, height, sigma);

                // 2. Calculate Variances & Covariance
                let mut x_sq = x_cow.iter().map(|&v| v * v).collect::<Vec<_>>();
                gaussian_blur_f32(&mut x_sq, &mut scratch, width, height, sigma);

                let mut y_sq = y_cow.iter().map(|&v| v * v).collect::<Vec<_>>();
                gaussian_blur_f32(&mut y_sq, &mut scratch, width, height, sigma);

                let mut xy = x_cow
                    .iter()
                    .zip(y_cow.iter())
                    .map(|(&a, &b)| a * b)
                    .collect::<Vec<_>>();
                gaussian_blur_f32(&mut xy, &mut scratch, width, height, sigma);

                // 3. Compute SSIM
                let mut ssim_sum = 0.0;
                for i in 0..pixel_count {
                    let mu_x_sq = mu_x[i] * mu_x[i];
                    let mu_y_sq = mu_y[i] * mu_y[i];
                    let mu_x_mu_y = mu_x[i] * mu_y[i];

                    let sigma_x_sq = (x_sq[i] - mu_x_sq).max(0.0);
                    let sigma_y_sq = (y_sq[i] - mu_y_sq).max(0.0);
                    let sigma_xy = xy[i] - mu_x_mu_y;

                    let num = (2.0 * mu_x_mu_y + c1_const) * (2.0 * sigma_xy + c2_const);
                    let den = (mu_x_sq + mu_y_sq + c1_const) * (sigma_x_sq + sigma_y_sq + c2_const);

                    ssim_sum += num / den;
                }

                Ok(ssim_sum / pixel_count as f32)
            };

        let channel_ssim_scores: Vec<f32>;

        #[cfg(feature = "threads")]
        {
            channel_ssim_scores = std::thread::scope(|s| {
                let mut handles = Vec::new();

                for (c1_ref, c2_ref) in img1
                    .channels_ref(false)
                    .iter()
                    .zip(img2.channels_ref(false))
                {
                    // We can pass references directly into the scoped thread
                    let handle = s.spawn(|| compute_channel_ssim(c1_ref, c2_ref));
                    handles.push(handle);
                }

                handles
                    .into_iter()
                    .map(|h| h.join().unwrap())
                    .collect::<Result<Vec<f32>, ImageErrors>>()
            })?;
        }

        #[cfg(not(feature = "threads"))]
        {
            let mut scores = Vec::new();
            for (c1_ref, c2_ref) in img1
                .channels_ref(false)
                .iter()
                .zip(img2.channels_ref(false))
            {
                scores.push(compute_channel_ssim(c1_ref, c2_ref)?);
            }
            channel_ssim_scores = scores;
        }

        // Average the scores across all channels to get the final MSSIM
        let final_ssim = channel_ssim_scores.iter().sum::<f32>() / channel_ssim_scores.len() as f32;

        // Write to output safely
        if let Ok(mut scores) = self.output_scores.lock() {
            // Save the channel array and the final average
            *scores = Some((channel_ssim_scores, final_ssim));
        }

        Ok(())
    }

    fn printable_result(&self) -> Option<String> {
        if let Ok(guard) = self.output_scores.lock() {
            if let Some((channels, global)) = &*guard {
                // Calculate decibels: dB = -10 * log10(1 - SSIM)
                // Cap it at 100 dB for perfect matches to avoid infinity
                let db = if *global >= 1.0 { 100.0 } else { -10.0 * (1.0 - global).log10() };

                // Map channels to names (Assuming standard RGB/RGBA order)

                let mut channel_str = String::new();
                for (i, &score) in channels.iter().enumerate() {
                    write!(channel_str, "{i}:{score:.4} ").expect("Can't write");
                }

                return Some(format!(
                    "SSIM {} All:{:.4} ({:.2} dB)",
                    channel_str.trim(),
                    global,
                    db
                ));
            }
        }
        None
    }
}
