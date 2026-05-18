use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::traits::NumOps;

/// Averages multiple channels together to reduce noise.
/// `base` acts as both the first input and the mutable output buffer.
pub fn average_channels<T>(base: &mut [T], others: &[&[T]])
where
    T: Copy + NumOps<T>,
{
    let total_images = (others.len() + 1) as f32;

    for i in 0..base.len() {
        // Accumulate in f32 to prevent integer overflow
        let mut sum = base[i].to_f32();

        for other in others {
            sum += other[i].to_f32();
        }

        // Write the averaged value back into the base image
        base[i] = T::from_f32(sum / total_images);
    }
}
pub struct AverageSequence;

impl Default for AverageSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl AverageSequence {
    #[must_use]
    pub fn new() -> Self {
        AverageSequence
    }
}

impl OperationsTrait for AverageSequence {
    fn name(&self) -> &'static str {
        "Average Sequence"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Average Sequence requires the full stack; call via execute_multiple",
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
                "Average Sequence requires at least 2 images.",
            ));
        }

        {
            let base_image = &images[0];
            let b_dims = base_image.dimensions();
            let b_depth = base_image.depth();
            let b_colorspace = base_image.colorspace();
            let other_images = &images[1..];

            for img in other_images.as_ref() {
                if img.dimensions() != b_dims
                    || img.depth() != b_depth
                    || img.colorspace() != b_colorspace
                {
                    return Err(ImageErrors::GenericStr(
                        "All images in the sequence must have identical dimensions, depths, and colorspaces."
                    ));
                }
            }
        }
        // 1. Drain the stack. We take ownership of all images.
        let sequence: Vec<Image> = std::mem::take(images);

        // 2. Separate the first image to use as our output canvas
        let mut iter = sequence.into_iter();
        let mut base_image = iter.next().unwrap();
        let other_images = iter;

        // 3. Validate invariants
        let b_depth = base_image.depth();
        let b_colorspace = base_image.colorspace();


        let bit_type = b_depth.bit_type();

        // 4. Iterate over frames and channels
        for frame_idx in 0..base_image.frames_ref().len() {
            let num_channels = base_image.frames_ref()[frame_idx]
                .channels_ref(b_colorspace, false)
                .len();

            for c_idx in 0..num_channels {
                // Match the bit type and collect the slices
                match bit_type {
                    BitType::U8 => {
                        let mut other_slices = Vec::with_capacity(other_images.len());
                        for img in other_images.as_ref() {
                            let chan = &img.frames_ref()[frame_idx]
                                .channels_ref(b_colorspace, false)[c_idx];
                            other_slices.push(chan.reinterpret_as::<u8>()?);
                        }
                        let base_chan = &mut base_image.frames_mut()[frame_idx]
                            .channels_mut(b_colorspace, false)[c_idx];
                        average_channels::<u8>(
                            base_chan.reinterpret_as_mut::<u8>()?,
                            &other_slices,
                        );
                    }
                    BitType::U16 => {
                        let mut other_slices = Vec::with_capacity(other_images.len());
                        for img in other_images.as_ref() {
                            let chan = &img.frames_ref()[frame_idx]
                                .channels_ref(b_colorspace, false)[c_idx];
                            other_slices.push(chan.reinterpret_as::<u16>()?);
                        }
                        let base_chan = &mut base_image.frames_mut()[frame_idx]
                            .channels_mut(b_colorspace, false)[c_idx];
                        average_channels::<u16>(
                            base_chan.reinterpret_as_mut::<u16>()?,
                            &other_slices,
                        );
                    }
                    BitType::F32 => {
                        let mut other_slices = Vec::with_capacity(other_images.len());
                        for img in other_images.as_ref() {
                            let chan = &img.frames_ref()[frame_idx]
                                .channels_ref(b_colorspace, false)[c_idx];
                            other_slices.push(chan.reinterpret_as::<f32>()?);
                        }
                        let base_chan = &mut base_image.frames_mut()[frame_idx]
                            .channels_mut(b_colorspace, false)[c_idx];
                        average_channels::<f32>(
                            base_chan.reinterpret_as_mut::<f32>()?,
                            &other_slices,
                        );
                    }
                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
                }
            }
        }

        // 5. Push the flattened, averaged image back onto the pipeline stack
        images.push(base_image);

        Ok(())
    }
}
