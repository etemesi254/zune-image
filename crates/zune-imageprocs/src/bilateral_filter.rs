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
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::pad::{pad, PadMethod};
use crate::spatial::spatial;
use crate::traits::NumOps;

/// The bilateral filter struct
///
/// # Alpha channel
/// - Alpha  channel is ignored
///
/// # Example
///
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::bilateral_filter::BilateralFilter;
/// // random values
/// let filter= BilateralFilter::new(10,25.0,25.0);
///
/// let mut image =Image::fill(10_u8,ColorSpace::RGB,10,10);
/// filter.execute(&mut image)?;
/// # Ok::<(),ImageErrors>(())
/// ```
pub struct BilateralFilter {
    d:           i32,
    sigma_color: f32,
    sigma_space: f32
}

impl BilateralFilter {
    /// Create a new bilateral filter
    ///
    /// # Arguments
    /// - `d`:Diameter of each pixel neighborhood that is used during filtering. If it is non-positive, it is computed from sigma_space.
    ///
    /// - `sigma_color`:Filter sigma in the color space.
    ///  A larger value of the parameter means that farther colors within the pixel neighborhood (see sigmaSpace)
    ///  will be mixed together, resulting in larger areas of semi-equal color.
    ///- `sigma_space`: Filter sigma in the coordinate space.
    ///  A larger value of the parameter means that farther pixels will influence each other as
    ///   long as their colors are close enough (see sigma_color ).
    ///   When d>0, it specifies the neighborhood size regardless of sigma_space. Otherwise, d is proportional to sigma_space.
    #[must_use]
    pub fn new(d: i32, sigma_color: f32, sigma_space: f32) -> BilateralFilter {
        BilateralFilter {
            d,
            sigma_color,
            sigma_space
        }
    }
}

impl OperationsTrait for BilateralFilter {
    fn name(&self) -> &'static str {
        "Bilateral Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        let (w, h) = image.dimensions();
        if self.d < 1 {
            return Ok(());
        }

        // initialize bilateral coefficients outside of the main loop
        let coeffs = init_bilateral(
            self.d,
            self.sigma_color,
            self.sigma_space,
            usize::from(depth.max_value()) + 1
        );

        #[cfg(feature = "threads")]
        {
            std::thread::scope(|s| {
                let mut t_results = vec![];
                for channel in image.channels_mut(true) {
                    let result = s.spawn(|| {
                        let mut new_channel =
                            Channel::new_with_bit_type(channel.len(), depth.bit_type());
                        match depth.bit_type() {
                            BitType::U8 => bilateral_filter_int::<u8>(
                                channel.reinterpret_as()?,
                                new_channel.reinterpret_as_mut()?,
                                w,
                                h,
                                &coeffs
                            ),
                            BitType::U16 => bilateral_filter_int::<u16>(
                                channel.reinterpret_as()?,
                                new_channel.reinterpret_as_mut()?,
                                w,
                                h,
                                &coeffs
                            ),

                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d
                                ));
                            }
                        }
                        *channel = new_channel;
                        Ok(())
                    });
                    t_results.push(result);
                }

                t_results
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }

        #[cfg(not(feature = "threads"))]
        {
            for channel in image.channels_mut(true) {
                let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());
                match depth.bit_type() {
                    BitType::U8 => {
                        bilateral_filter_int::<u8>(
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?,
                            w,
                            h,
                            &coeffs
                        );
                    }
                    BitType::U16 => {
                        bilateral_filter_int::<u16>(
                            channel.reinterpret_as()?,
                            new_channel.reinterpret_as_mut()?,
                            w,
                            h,
                            &coeffs
                        );
                    }

                    d => {
                        return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
                    }
                }
                // overwrite with the filtered channel
                *channel = new_channel;
            }
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}

struct BilateralCoeffs {
    color_weight: Vec<f64>,
    space_weight: Vec<f64>,
    radius:       usize,
    makx:         usize
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn init_bilateral(
    d: i32, sigma_color: f32, mut sigma_space: f32, color_range: usize
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
        makx
    };
}

fn bilateral_filter_int<T>(
    src: &[T], dest: &mut [T], width: usize, height: usize, coeffs: &BilateralCoeffs
) where
    T: Copy + NumOps<T> + Default,
    i32: std::convert::From<T>
{
    let radius = coeffs.radius;

    //pad here
    let padded_input = pad(src, width, height, radius, radius, PadMethod::Replicate);

    // use an inner lambda to implement the bilateral loop as it allows us to borrow
    // surrounding variables

    // Carry out the bilateral filter on a single pixel
    // the mid of the area is considered to be the main pixel, the others
    // are it's surrounding.
    //
    // This impl matches opencv bilateral_filter's inner loop, with less pointer chasing as
    // the spatial function sends the right thing to us
    let bilateral_func = |area: &[T]| -> T {
        let mid = (area.len() + 1) / 2;

        let mut sum = 0.0;
        let mut wsum = 0.0;
        let val0 = i32::from(area[mid]);

        for (val, space_w) in area
            .iter()
            .zip(coeffs.space_weight.iter())
            .take(coeffs.makx)
        {
            let val = i32::from(*val);
            let abs_val = (val - val0).unsigned_abs() as usize;

            let w = space_w * coeffs.color_weight[abs_val];
            sum += f64::from(val) * w;
            wsum += w;
        }
        return T::from_f64((sum / wsum).round());
    };

    spatial(&padded_input, dest, radius, width, height, bilateral_func);
}

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
    nanorand::WyRand::new().fill(&mut input);

    let pixels = Image::from_u8(&input, w, h, color);
    let filter = BilateralFilter::new(20, 75.0, 75.0);
    for d in filter.supported_types() {
        let mut c = pixels.clone();
        c.convert_depth(d.to_depth()).unwrap();
        filter.execute(&mut c).unwrap();
    }
}
