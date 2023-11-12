#![allow(dead_code, unused_variables)]

use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

/// Create a blend image filter which
/// can blend two images based on a configurable alpha
///
/// Alpha must be between 0.0 and 1.0 for the images
/// and it's clamped to that range
pub struct Blend<'src> {
    image: &'src Image,
    alpha: f32
}

impl<'src> Blend<'src> {
    /// Create a new blend filter
    ///
    /// # Arguments
    /// - src_alpha:  If above 1.0 source will become the destination, if less than 0.0 dest will be unmodified
    /// -
    pub fn new(image: &'src Image, src_alpha: f32) -> Blend<'src> {
        Blend {
            image,
            alpha: src_alpha
        }
    }
}

impl<'src> OperationsTrait for Blend<'src> {
    fn name(&self) -> &'static str {
        "Blend"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        // confirm invariants
        if !self.alpha.is_normal() {
            return Err(ImageErrors::GenericStr("Alpha is not normal"));
        }
        if image.dimensions() != self.image.dimensions() {
            return Err(ImageErrors::GenericStr(
                "Image dimensions are incompatible for blend"
            ));
        }
        if image.depth() != self.image.depth() {
            return Err(ImageErrors::GenericStr(
                "Image depths do not match for blend"
            ));
        }

        if image.colorspace() != self.image.colorspace() {
            return Err(ImageErrors::GenericStr(
                "Image colorspace does not match for blend"
            ));
        }

        let b_type = image.depth().bit_type();
        for (src_chan, d_chan) in self
            .image
            .channels_ref(true)
            .iter()
            .zip(image.channels_mut(true))
        {
            match b_type {
                BitType::U8 => blend_single_channel::<u8>(
                    src_chan.reinterpret_as()?,
                    d_chan.reinterpret_as_mut()?,
                    self.alpha
                ),
                BitType::U16 => blend_single_channel::<u16>(
                    src_chan.reinterpret_as()?,
                    d_chan.reinterpret_as_mut()?,
                    self.alpha
                ),
                BitType::F32 => blend_single_channel::<f32>(
                    src_chan.reinterpret_as()?,
                    d_chan.reinterpret_as_mut()?,
                    self.alpha
                ),

                d => {
                    return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
                }
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

pub fn blend_single_channel<T>(src: &[T], dest: &mut [T], src_alpha: f32)
where
    f32: std::convert::From<T>,
    T: Copy + NumOps<T>
{
    if src_alpha <= 0.0 {
        return;
    }
    if src_alpha >= 1.0 {
        // copy source to destination
        dest.copy_from_slice(src);
    }

    let dest_alpha = f32::from(T::max_val()) - src_alpha;

    for (src, dest) in src.iter().zip(dest.iter_mut()) {
        // formula is (src_alpha) * src  + (dest_alpha) * dest
        *dest = T::from_f32((src_alpha * f32::from(*src)) + (dest_alpha * f32::from(*dest)));
    }
}
