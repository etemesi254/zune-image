//! A bilateral filter
//!
//! A bilateral filter is a non-linear, edge-preserving,
//! and noise-reducing smoothing filter for images.
//!
//! It is a type of non-linear filter that reduces noise while preserving edges.
//! The filter works by averaging the pixels in a neighborhood around a given pixel,
//! but the weights of the pixels are determined not only by their spatial distance from the given pixel,
//! but also by their intensity difference from the given pixel
//!
//!  A description can be found [here](https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/MANDUCHI1/Bilateral_Filtering.html)
//!
use crate::simd_fn;
use crate::traits::NumOps;
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::{OperationColorValues, OperationsTrait};
/// A bilateral filter operation for edge-preserving smoothing.
///
/// Unlike standard Gaussian blurs that average all pixels in a neighborhood, a bilateral
/// filter weighs surrounding pixels based on two factors:
/// 1. **Spatial distance:** Pixels closer to the center have a higher weight.
/// 2. **Color/Intensity distance:** Pixels with values closer to the center pixel's value have a higher weight.
///
/// # Alpha Channel
///
/// The alpha channel is ignored during this operation.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::bilateral_filter::BilateralFilter;
///
/// let filter = BilateralFilter::new(10, 25.0, 25.0);
/// let mut image = Image::fill(10_u8, ColorSpace::RGB, 10, 10);
/// filter.execute(&mut image)?;
/// # Ok::<(), ImageErrors>(())
/// ```
pub struct BilateralFilter {
    d: i32,
    sigma_color: f32,
    sigma_space: f32,
}

impl BilateralFilter {
    /// Create a new bilateral filter
    ///
    /// # Arguments
    /// - `d`:Diameter of each pixel neighborhood that is used during filtering. If it is non-positive, it is computed from sigma_space.
    ///
    /// - `sigma_color`:Filter sigma in the color space.
    ///   A larger value of the parameter means that farther colors within the pixel neighborhood (see sigmaSpace)
    ///   will be mixed together, resulting in larger areas of semi-equal color.
    ///- `sigma_space`: Filter sigma in the coordinate space.
    ///  A larger value of the parameter means that farther pixels will influence each other as
    ///  long as their colors are close enough (see sigma_color ).
    ///  When d>0, it specifies the neighborhood size regardless of sigma_space. Otherwise, d is proportional to sigma_space.
    #[must_use]
    pub fn new(d: i32, sigma_color: f32, sigma_space: f32) -> BilateralFilter {
        BilateralFilter {
            d,
            sigma_color,
            sigma_space,
        }
    }
}

impl OperationsTrait for BilateralFilter {
    fn name(&self) -> &'static str {
        "Bilateral Filter"
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        if self.d < 1 {
            return Ok(());
        }

        // 1. Pre-allocate the destination buffer by cloning the source image
        let mut dest_image = image.clone();

        // 2. Initialize coefficients once
        let coeffs = init_bilateral(
            self.d,
            self.sigma_color,
            self.sigma_space,
            usize::from(depth.max_value()) + 1,
        );

        // 3. Dispatch the parallel out-of-place primitive based on bit depth
        match depth.bit_type() {
            BitType::U8 => {
                image.par_process_regions_out_of_place::<u8, _>(
                    &mut dest_image,
                    true, // ignore_alpha as per documentation
                    |region| bilateral_filter_region_u8(region, &coeffs),
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    true,
                    |region| bilateral_filter_region::<u16>(region, &coeffs),
                )?;
            }
            d => {
                return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
            }
        }

        // 4. Overwrite the original image with the processed destination
        *image = dest_image;

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}

simd_fn!(
    fn bilateral_filter_region_u8(region: &mut PlanarRegionOut<'_, u8>, coeffs: &BilateralCoeffs) {
        bilateral_filter_region::<u8>(region, coeffs);
    }
);

#[inline(always)]
fn bilateral_filter_region<T>(region: &mut PlanarRegionOut<'_, T>, coeffs: &BilateralCoeffs)
where
    T: Copy + NumOps<T> + Default + Send + Sync,
    i32: std::convert::From<T>,
{
    let radius = coeffs.radius as i32;
    let width = region.width;

    if region.src_channels.is_empty() {
        return;
    }

    // Since src_channels contains the FULL image slices, we can derive the global height
    let global_height = region.src_channels[0].len() / width;

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        // Iterate only over the lines this specific region is responsible for
        for local_y in 0..region.height {
            // Map to the absolute image coordinates for neighborhood lookups
            let global_y = region.y_offset + local_y;

            for x in 0..width {
                let val0 = i32::from(src[global_y * width + x]);

                let mut sum = 0.0_f64;
                let mut wsum = 0.0_f64;
                let mut k = 0usize;

                let mut dy = -radius;
                while dy <= radius {
                    let mut dx = -radius;
                    while dx <= radius {
                        let r = ((dy * dy + dx * dx) as f64).sqrt();
                        if r <= radius as f64 {
                            // Clamp against the GLOBAL height, not the region height
                            let sy =
                                (global_y as i32 + dy).clamp(0, global_height as i32 - 1) as usize;
                            let sx = (x as i32 + dx).clamp(0, width as i32 - 1) as usize;

                            // Read safely from the full source buffer
                            let val = i32::from(src[sy * width + sx]);
                            let abs_diff = (val - val0).unsigned_abs() as usize;

                            let w = coeffs.space_weight[k] * coeffs.color_weight[abs_diff];
                            sum += f64::from(val) * w;
                            wsum += w;
                            k += 1;
                        }
                        dx += 1;
                    }
                    dy += 1;
                }

                // Write to the chunked destination buffer using LOCAL coordinates
                dest[local_y * width + x] = T::from_f64((sum / wsum).round());
            }
        }
    }
}

struct BilateralCoeffs {
    color_weight: Vec<f64>,
    space_weight: Vec<f64>,
    radius: usize,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn init_bilateral(
    d: i32, sigma_color: f32, mut sigma_space: f32, color_range: usize,
) -> BilateralCoeffs {
    let gauss_color_coeff = f64::from(-0.5 / (sigma_color * sigma_color));
    let gauss_space_coeff = f64::from(-0.5 / (sigma_space * sigma_space));
    let cn = 1;

    // if sigma_color <= 0.0 {
    //     sigma_color = 1.0;
    // }
    if sigma_space <= 0.0 {
        sigma_space = 1.0;
    }

    let radius: i32 = if d <= 0 { (sigma_space * 1.5).round() as _ } else { d / 2 };

    let mut color_weight = vec![0.0_f64; cn * color_range];
    let mut space_weight = vec![0.0_f64; (d * d).unsigned_abs() as usize];

    // initialize color-related bilateral filter coeffs
    for (i, item) in color_weight.iter_mut().enumerate().take(color_range) {
        let c = i as f64;
        *item = (c * c * gauss_color_coeff).exp();
    }
    let mut makx = 0;
    // initialize space-related bilateral coeffs
    for i in -radius..=radius {
        for j in -radius..=radius {
            let r = f64::from((i * i) + (j * j)).sqrt();
            if r > f64::from(radius) {
                continue;
            }
            space_weight[makx] = (r * r * gauss_space_coeff).exp();
            makx += 1;
        }
    }
    return BilateralCoeffs {
        color_weight,
        space_weight,
        radius: usize::try_from(radius).unwrap_or_default(),
    };
}

#[cfg(test)]
mod tests {
    use crate::bilateral_filter::BilateralFilter;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    /// Tests to see that the filter can run on supported bit depths
    #[test]
    fn test_bilateral_simple() {
        use nanorand::Rng;
        use zune_core::colorspace::ColorSpace;

        let w = 100;
        let h = 100;
        let color = ColorSpace::Luma;

        // fill with random items
        let mut input = vec![0_u8; w * h * color.num_components()];
        nanorand::WyRand::new_seed(301).fill(&mut input);

        let pixels = Image::from_u8(&input, w, h, color);
        let filter = BilateralFilter::new(20, 75.0, 75.0);
        for d in filter.supported_types() {
            let mut c = pixels.clone();
            c.convert_depth(d.to_depth()).unwrap();
            filter.execute(&mut c).unwrap();
        }
    }

    /// Constant image must be unchanged by the bilateral filter —
    /// every neighbour has zero color distance so all weights are equal,
    /// and the weighted average equals the original value.
    #[test]
    fn test_bilateral_constant_image() {
        use zune_core::colorspace::ColorSpace;

        let w = 32;
        let h = 32;
        let fill_value = 128_u8;
        let input = vec![fill_value; w * h];
        let mut image = Image::from_u8(&input, w, h, ColorSpace::Luma);
        BilateralFilter::new(9, 50.0, 50.0)
            .execute(&mut image)
            .unwrap();

        let channel = image.flatten_to_u8();
        for &px in &channel[0] {
            assert_eq!(
                px, fill_value,
                "constant image must survive bilateral filtering unchanged"
            );
        }
    }

    /// A 1×1 image should pass through unchanged — the only pixel
    /// is both the centre and its sole neighbour.
    #[test]
    fn test_bilateral_single_pixel() {
        use zune_core::colorspace::ColorSpace;

        let mut image = Image::from_u8(&[200_u8], 1, 1, ColorSpace::Luma);
        BilateralFilter::new(5, 25.0, 25.0)
            .execute(&mut image)
            .unwrap();

        let out = image.flatten_to_u8();
        assert_eq!(out[0][0], 200, "single pixel must be returned unchanged");
    }

    /// A sharp vertical edge (left half black, right half white) must
    /// not be blurred away — the edge pixels should remain clearly
    /// separated after filtering.
    #[test]
    fn test_bilateral_preserves_edge() {
        use zune_core::colorspace::ColorSpace;

        let w = 40;
        let h = 20;
        let input: Vec<u8> = (0..w * h)
            .map(|i| if (i % w) < w / 2 { 0 } else { 255 })
            .collect();

        let mut image = Image::from_u8(&input, w, h, ColorSpace::Luma);
        BilateralFilter::new(9, 30.0, 30.0)
            .execute(&mut image)
            .unwrap();

        let out = image.flatten_to_u8();
        // Pixels well inside the dark half must stay dark
        for row in 0..h {
            let dark_px = out[0][row * w + 5];
            assert!(dark_px < 64, "dark region smeared too much: {dark_px}");
            // Pixels well inside the bright half must stay bright
            let bright_px = out[0][row * w + w - 5];
            assert!(
                bright_px > 192,
                "bright region smeared too much: {bright_px}"
            );
        }
    }

    /// Applying the filter twice must not blow up or produce wildly
    /// different results from a single pass — the operation is stable.
    #[test]
    fn test_bilateral_idempotent_approx() {
        use nanorand::Rng;
        use zune_core::colorspace::ColorSpace;

        let w = 20;
        let h = 20;
        let mut input = vec![0_u8; w * h];
        nanorand::WyRand::new_seed(7).fill(&mut input);

        let mut once = Image::from_u8(&input, w, h, ColorSpace::Luma);
        BilateralFilter::new(5, 40.0, 40.0)
            .execute(&mut once)
            .unwrap();
        let after_one = once.flatten_to_u8()[0].clone();

        let mut twice = Image::from_u8(&input, w, h, ColorSpace::Luma);
        BilateralFilter::new(5, 40.0, 40.0)
            .execute(&mut twice)
            .unwrap();
        BilateralFilter::new(5, 40.0, 40.0)
            .execute(&mut twice)
            .unwrap();
        let after_two = twice.flatten_to_u8()[0].clone();

        // The second pass should change values by at most ~30 on a noisy image;
        // larger deltas would indicate numerical instability.
        for (a, b) in after_one.iter().zip(after_two.iter()) {
            let diff = (*a as i16 - *b as i16).abs();
            assert!(
                diff <= 50,
                "second pass caused unexpectedly large change: {diff}"
            );
        }
    }

    /// d < 1 must be a no-op — the image must be returned byte-for-byte identical.
    #[test]
    fn test_bilateral_zero_diameter_noop() {
        use zune_core::colorspace::ColorSpace;

        let input = vec![123_u8; 10 * 10];
        let mut image = Image::from_u8(&input, 10, 10, ColorSpace::Luma);
        BilateralFilter::new(0, 25.0, 25.0)
            .execute(&mut image)
            .unwrap();

        let out = image.flatten_to_u8();
        for &px in &out[0] {
            assert_eq!(px, 123, "d=0 must leave the image unchanged");
        }
    }
}
