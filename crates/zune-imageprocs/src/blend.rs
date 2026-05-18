//! Blend filter
//!
//! This can be used to combine two or more images based on an alpha value
//! which is used to determine the `opacity` of pixels during blending
//!
//! The formula for blending is
//!
//! ```text
//! dest = (src_alpha) * src + (1 - src_alpha) * dest
//! ```
//! `src_alpha` is expected to be between 0.0 and 1.0
//!
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;

/// Create a blend image filter which
/// can blend two images based on a configurable alpha
///
/// Alpha must be between 0.0 and 1.0 for the images
/// and it's clamped to that range.
///
/// # Alpha channel
/// - Alpha channel is ignored
///
/// # Stack Mechanics
/// This operation pops the last image off the stack to use as the `source`,
/// and blends it onto the new top of the stack (`destination`).
///
/// # Examples
///
/// Blend two images with an alpha of 0.5 which divides the source and destination pixel by half and adds them
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::blend::Blend;
///
/// // create a gradient from luma using addition
/// let im1 = Image::from_fn::<u8,_>(100, 100, ColorSpace::Luma, |x, y, pix| {
///     pix[0] = ((x + y) % 256) as u8;
/// });
/// // create a reverse gradient
/// let im2 = Image::from_fn::<u8,_>(100, 100, ColorSpace::Luma, |x, y, pix| {
///     pix[0] = (x.wrapping_sub(y) % 256) as u8;
/// });
///
/// // Load them into a stack
/// let mut stack = vec![im1, im2];
///
/// // blend them with 0.5, which picks equal from forward and reverse gradient
/// Blend::new(0.5).execute_multiple(&mut stack).unwrap();
///
/// // The stack now contains 1 blended image
/// assert_eq!(stack.len(), 1);
/// ```
///
pub struct Blend {
    alpha: f32,
}

impl Blend {
    /// Create a new blend filter
    ///
    /// # Arguments
    /// - src_alpha: Range is 0.0-1.0. If above 1.0 source will become the destination, if less than 0.0 dest will be unmodified.
    #[must_use]
    pub fn new(src_alpha: f32) -> Blend {
        Blend { alpha: src_alpha }
    }
}

impl OperationsTrait for Blend {
    fn name(&self) -> &'static str {
        "Blend"
    }

    // Not in use, use execute_multiple
    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Blend requires multiple images; it must be called via execute_multiple",
        ))
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.len() < 2 {
            return Err(ImageErrors::GenericStr(
                "Blend requires at least two images in the pipeline",
            ));
        }

        if self.alpha != 0.0 && !self.alpha.is_normal() {
            return Err(ImageErrors::GenericStr("Alpha is not normal"));
        }

        // Pop the overlay image off the stack
        let src_image = images.pop().unwrap();

        // The background image is now the top of the stack
        let dst_image = images.last_mut().unwrap();

        // Confirm invariants between the two images
        if dst_image.dimensions() != src_image.dimensions() {
            return Err(ImageErrors::GenericStr(
                "Image dimensions are incompatible for blend",
            ));
        }
        if dst_image.depth() != src_image.depth() {
            return Err(ImageErrors::GenericStr(
                "Image depths do not match for blend",
            ));
        }
        if dst_image.colorspace() != src_image.colorspace() {
            return Err(ImageErrors::GenericStr(
                "Image colorspace does not match for blend",
            ));
        }

        let b_type = dst_image.depth().bit_type();

        match b_type {
            BitType::U8 => {
                src_image.par_process_regions_out_of_place::<u8, _>(
                    dst_image,
                    true, 
                    |region| blend_region::<u8>(region, self.alpha),
                )?;
            }
            BitType::U16 => {
                src_image.par_process_regions_out_of_place::<u16, _>(
                    dst_image,
                    true,
                    |region| blend_region::<u16>(region, self.alpha),
                )?;
            }
            BitType::F32 => {
                src_image.par_process_regions_out_of_place::<f32, _>(
                    dst_image,
                    true,
                    |region| blend_region::<f32>(region, self.alpha),
                )?;
            }
            d => {
                return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
            }
        }
        Ok(())
    }
}

fn blend_region<T>(region: &mut PlanarRegionOut<'_, T>, alpha: f32)
where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    // Calculate the spatial bounds for this specific region chunk
    let start_idx = region.y_offset * region.width;
    let len = region.height * region.width;
    let end_idx = start_idx + len;

    // Iterate through the corresponding channels of both images
    for (src_full, dest_chunk) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        // Safety boundary check
        if src_full.len() >= end_idx {
            // Slice the global source image down to the exact local chunk bounds
            let src_chunk = &src_full[start_idx..end_idx];

            // Execute your existing zero-allocation SIMD-friendly function!
            blend_single_channel::<T>(src_chunk, dest_chunk, alpha);
        }
    }
}
pub fn blend_single_channel<T>(src: &[T], dest: &mut [T], src_alpha: f32)
where
    f32: std::convert::From<T>,
    T: Copy + NumOps<T>,
{
    if src_alpha <= 0.0 {
        return;
    }
    if src_alpha >= 1.0 {
        // copy source to destination
        dest.copy_from_slice(src);
    }

    let dest_alpha = 1.0 - src_alpha;

    for (src, dest) in src.iter().zip(dest.iter_mut()) {
        // formula is (src_alpha) * src  + (dest_alpha) * dest
        *dest = T::from_f32((src_alpha * f32::from(*src)) + (dest_alpha * f32::from(*dest)));
    }
}
