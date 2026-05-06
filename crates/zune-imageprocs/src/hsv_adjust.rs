//! Adjust the hue, saturation and variance of an image
//!
//! This filter allows one to play around with the HSV of an image and change them accordingly
//!
//! The filter preserves the initial colorspace of the image
//!
//! ## Algorithm
//! The canonical way to do it is to convert the colorspace to HSL/HSV and manipulate in that colorspace
//! and convert back to original colorspace
//!
//! But we can do better, we really only need the HSV for manipulation so we can do HSV color transforms
//! on RGB data using matrix multiplications
//!
//! How to do that is found [in this amazing article](https://beesbuzz.biz/code/16-hsv-color-transforms)
//! but the main gist is that instead of converting back and forth, use a simple matrix that allows such calculations
//! this routine adapts that for use
//!
use std::f32::consts::PI;

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;

/// Adjusts the Hue, Saturation, and Value (Brightness) of an image.
///
/// This filter manipulates the color properties of an image while preserving its
/// original colorspace.
///
/// # Algorithm
///
/// Instead of performing an expensive per-pixel conversion from RGB to HSV and back,
/// this filter computes a 3x3 transformation matrix based on the requested HSV shifts.
/// It then applies this matrix directly to the RGB channels, acting as a highly
/// optimized color tint/modulation operation.
///
/// *Reference: [HSV Color Transforms](https://beesbuzz.biz/code/16-hsv-color-transforms)*
///
/// # Example
///
/// Rotate the hue by 90 degrees and boost saturation by 20%:
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::hsv_adjust::HsvAdjust;
///
/// // Create a solid red image
/// let mut img = Image::fill(255_u8, ColorSpace::RGB, 100, 100);
///
/// // Apply the adjustment
/// let adjust = HsvAdjust::new(90.0, 1.2, 1.0);
/// adjust.execute(&mut img)?;
/// # Ok::<(), ImageErrors>(())
/// ```
pub struct HsvAdjust {
    hue:        f32,
    saturation: f32,
    lightness:  f32
}

impl HsvAdjust {
    /// Create a new hsv adjust filter
    ///
    /// # Arguments
    /// - hue: The hue rotation argument. This is usually a value between 0 and 360 degrees
    ///
    /// -saturation: The saturation scaling factor, a value of 0 produces a grayscale image, 1 has no effect, other values lie withing that spectrum
    ///  > 1 produces vibrant cartoonish color
    ///
    /// - lightness: The lightness scaling factor, a value greater than 0, values less than or equal to zero
    /// produce a black image, higher values increase the brightness of the image, 1.0 doesn't do anything
    /// to image lightness
    ///
    #[must_use]
    pub fn new(hue: f32, saturation: f32, lightness: f32) -> HsvAdjust {
        HsvAdjust {
            hue,
            saturation,
            lightness
        }
    }
}
impl OperationsTrait for HsvAdjust {
    fn name(&self) -> &'static str {
        "modulate"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let orig_color = image.colorspace();
        // convert to RGBA, this preserves alpha when it exists
        // were we to do rgb, we have to worry about preserving alpha
        // we also do this so that we know where R,G and B components are, e.g if color was ARGB
        // we'd have gotten the components wrong
        image.convert_color(ColorSpace::RGBA)?;
        // then we can manipulate the values
        let depth = image.depth();

        for frames in image.frames_mut() {
            let channels = frames.channels_vec();

            let (r, rest) = channels.split_at_mut(1);
            let (g, b) = rest.split_at_mut(1);

            match depth.bit_type() {
                BitType::U8 => {
                    modulate_hsl::<u8>(
                        r[0].reinterpret_as_mut()?,
                        g[0].reinterpret_as_mut()?,
                        b[0].reinterpret_as_mut()?,
                        self.hue,
                        self.saturation,
                        self.lightness
                    );
                }
                BitType::U16 => {
                    modulate_hsl::<u16>(
                        r[0].reinterpret_as_mut()?,
                        g[0].reinterpret_as_mut()?,
                        b[0].reinterpret_as_mut()?,
                        self.hue,
                        self.saturation,
                        self.lightness
                    );
                }
                BitType::F32 => {
                    modulate_hsl::<f32>(
                        r[0].reinterpret_as_mut()?,
                        g[0].reinterpret_as_mut()?,
                        b[0].reinterpret_as_mut()?,
                        self.hue,
                        self.saturation,
                        self.lightness
                    );
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }
        // convert to original color
        image.convert_color(orig_color)?;

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U8, BitType::U16]
    }
}

#[allow(clippy::many_single_char_names)]
#[allow(clippy::many_single_char_names)]
fn modulate_hsl<T>(r: &mut [T], g: &mut [T], b: &mut [T], h: f32, s: f32, v: f32)
where
    f32: From<T>,
    T: NumOps<T> + Copy
{
    let vsu = v * s * (h * PI / 180.0).cos();
    let vsw = v * s * (h * PI / 180.0).sin();

    // 1. Hoist the matrix coefficients out of the loop!
    let rr = 0.299 * v + 0.701 * vsu + 0.168 * vsw;
    let rg = 0.587 * v - 0.587 * vsu + 0.330 * vsw;
    let rb = 0.114 * v - 0.114 * vsu - 0.497 * vsw;

    let gr = 0.299 * v - 0.299 * vsu - 0.328 * vsw;
    let gg = 0.587 * v + 0.413 * vsu + 0.035 * vsw;
    let gb = 0.114 * v - 0.114 * vsu + 0.292 * vsw;

    let br = 0.299 * v - 0.300 * vsu + 1.250 * vsw;
    let bg = 0.587 * v - 0.588 * vsu - 1.050 * vsw;
    let bb = 0.114 * v + 0.886 * vsu - 0.203 * vsw;

    let min = T::MIN_VAL.to_f32();
    let max = T::MAX_VAL.to_f32();

    // 2. Elide bounds checks to unlock SIMD
    assert_eq!(r.len(), g.len());
    assert_eq!(g.len(), b.len());

    for ((r_i, g_i), b_i) in r.iter_mut().zip(g.iter_mut()).zip(b.iter_mut()) {
        let in_r = f32::from(*r_i);
        let in_g = f32::from(*g_i);
        let in_b = f32::from(*b_i);

        let new_r = (rr * in_r) + (rg * in_g) + (rb * in_b);
        let new_g = (gr * in_r) + (gg * in_g) + (gb * in_b);
        let new_b = (br * in_r) + (bg * in_g) + (bb * in_b);

        *r_i = T::from_f32(new_r.clamp(min, max));
        *g_i = T::from_f32(new_g.clamp(min, max));
        *b_i = T::from_f32(new_b.clamp(min, max));
    }
}
