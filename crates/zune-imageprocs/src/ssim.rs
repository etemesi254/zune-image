use rayon::prelude::*;
use std::fmt::Write;
use std::sync::{Arc, Mutex};

use zune_core::bit_depth::{BitDepth, BitType};
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};
use crate::gaussian_blur::GaussianBlur;

/// Structural Similarity Index Measure (SSIM)
///
/// A non-destructive metric operation. Computes the MSSIM between the top
/// two images on the stack and stores the result in `output_score`.
#[allow(clippy::type_complexity)]
pub struct SsimDetection {
    sigma: f32,
    pub output_scores: Arc<Mutex<Option<(Vec<f32>, f32)>>>,
}

impl Default for SsimDetection {
    fn default() -> Self {
        Self::new()
    }
}

impl SsimDetection {
    #[must_use]
    pub fn new() -> Self {
        SsimDetection {
            sigma: 1.5,
            output_scores: Arc::default(),
        }
    }

    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn get_output_ptr(&self) -> Arc<Mutex<Option<(Vec<f32>, f32)>>> {
        Arc::clone(&self.output_scores)
    }
}

impl OperationsTrait for SsimDetection {
    fn name(&self) -> &'static str {
        "SSIM Detection"
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
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

        // 1. Prepare base images (Cloned & Normalized to F32 0.0..1.0 range)
        let mut x = img1.clone();
        let mut y = img2.clone();
        x.convert_depth(BitDepth::Float32)?;
        y.convert_depth(BitDepth::Float32)?;

        let ignore_alpha = false;

        // 2. Generate Intermediate Math Images (X^2, Y^2, XY) using region primitives
        let mut x_sq = x.clone();
        x_sq.par_process_regions::<f32, _>(ignore_alpha, |region| {
            for ch in region.channels.iter_mut() {
                for px in ch.iter_mut() {
                    *px = *px * *px;
                }
            }
        })?;

        let mut y_sq = y.clone();
        y_sq.par_process_regions::<f32, _>(ignore_alpha, |region| {
            for ch in region.channels.iter_mut() {
                for px in ch.iter_mut() {
                    *px = *px * *px;
                }
            }
        })?;

        let mut xy = x.clone();
        y.par_process_regions_out_of_place::<f32, _>(&mut xy, ignore_alpha, |region| {
            let start_idx = region.y_offset * region.width;
            let end_idx = start_idx + (region.height * region.width);

            for (src_full, dest_chunk) in region
                .src_channels
                .iter()
                .zip(region.dest_channels.iter_mut())
            {
                if src_full.len() >= end_idx {
                    let src_chunk = &src_full[start_idx..end_idx];
                    for (s, d) in src_chunk.iter().zip(dest_chunk.iter_mut()) {
                        *d *= *s; // xy starts as x, we multiply by y
                    }
                }
            }
        })?;

        // 3. Apply Gaussian Blurs to all 5 images
        let blur = GaussianBlur::new(self.sigma);
        blur.execute_impl(&mut x)?; // Now mu_x
        blur.execute_impl(&mut y)?; // Now mu_y
        blur.execute_impl(&mut x_sq)?; // Now mu_x_sq
        blur.execute_impl(&mut y_sq)?; // Now mu_y_sq
        blur.execute_impl(&mut xy)?; // Now mu_xy

        // 4. Compute Final SSIM per channel
        // F32 image bounds to [0.0, 1.0], so L = 1.0
        let l = 1.0_f32;
        let c1 = (0.01 * l).powi(2);
        let c2 = (0.03 * l).powi(2);

        let num_channels = x.channels_ref(ignore_alpha).len();
        let mut channel_scores = vec![0.0f32; num_channels];

        channel_scores
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, score)| {
                let mu_x_ch = x.channels_ref(ignore_alpha)[i]
                    .reinterpret_as::<f32>()
                    .unwrap();
                let mu_y_ch = y.channels_ref(ignore_alpha)[i]
                    .reinterpret_as::<f32>()
                    .unwrap();
                let mu_x_sq_ch = x_sq.channels_ref(ignore_alpha)[i]
                    .reinterpret_as::<f32>()
                    .unwrap();
                let mu_y_sq_ch = y_sq.channels_ref(ignore_alpha)[i]
                    .reinterpret_as::<f32>()
                    .unwrap();
                let mu_xy_ch = xy.channels_ref(ignore_alpha)[i]
                    .reinterpret_as::<f32>()
                    .unwrap();

                let mut ssim_sum = 0.0;
                let len = mu_x_ch.len();

                for p in 0..len {
                    let mu_x_sq = mu_x_ch[p] * mu_x_ch[p];
                    let mu_y_sq = mu_y_ch[p] * mu_y_ch[p];
                    let mu_x_mu_y = mu_x_ch[p] * mu_y_ch[p];

                    // Variances (clamped to 0.0 to prevent floating-point precision drift negatives)
                    let sigma_x_sq = (mu_x_sq_ch[p] - mu_x_sq).max(0.0);
                    let sigma_y_sq = (mu_y_sq_ch[p] - mu_y_sq).max(0.0);
                    let sigma_xy = mu_xy_ch[p] - mu_x_mu_y;

                    let num = (2.0 * mu_x_mu_y + c1) * (2.0 * sigma_xy + c2);
                    let den = (mu_x_sq + mu_y_sq + c1) * (sigma_x_sq + sigma_y_sq + c2);

                    ssim_sum += num / den;
                }

                *score = ssim_sum / (len as f32);
            });

        // 5. Final Outputs
        let final_ssim = channel_scores.iter().sum::<f32>() / channel_scores.len() as f32;

        if let Ok(mut scores) = self.output_scores.lock() {
            *scores = Some((channel_scores, final_ssim));
        }

        Ok(())
    }

    fn printable_result(&self) -> Option<String> {
        if let Ok(guard) = self.output_scores.lock() {
            if let Some((channels, global)) = &*guard {
                // Calculate decibels: dB = -10 * log10(1 - SSIM)
                let db = if *global >= 1.0 { 100.0 } else { -10.0 * (1.0 - global).log10() };

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
