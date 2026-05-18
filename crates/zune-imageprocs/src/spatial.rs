use crate::spatial_ops::{
    find_contrast, find_gradient, find_max, find_mean, find_min, SpatialOperations,
};
use crate::traits::NumOps;
use std::ops::{Add, Div, Sub};
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;
use zune_image::traits::OperationsTrait;

pub struct SpatialOps {
    radius: usize,
    operation: SpatialOperations,
}

impl SpatialOps {
    #[must_use]
    pub fn new(radius: usize, operation: SpatialOperations) -> SpatialOps {
        SpatialOps { radius, operation }
    }
}

impl OperationsTrait for SpatialOps {
    fn name(&self) -> &'static str {
        "StatisticsOps Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();

        // Pre-allocate the destination buffer
        let mut dest_image = image.clone();
        let ignore_alpha = true;

        match depth.bit_type() {
            BitType::U8 => {
                image.par_process_regions_out_of_place::<u8, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        spatial_region(region, self.radius, self.operation);
                    },
                )?;
            }
            BitType::U16 => {
                image.par_process_regions_out_of_place::<u16, _>(
                    &mut dest_image,
                    ignore_alpha,
                    |region| {
                        spatial_region(region, self.radius, self.operation);
                    },
                )?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        *image = dest_image;
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
pub fn spatial_region<T>(
    region: &mut PlanarRegionOut<'_, T>, radius: usize, operations: SpatialOperations,
) where
    T: PartialOrd
        + Default
        + Copy
        + NumOps<T>
        + Sub<Output = T>
        + Add<Output = T>
        + Div<Output = T>
        + Send
        + Sync,
    u64: std::convert::From<T>,
{
    let width = region.width;
    if region.src_channels.is_empty() || width == 0 {
        return;
    }

    // Safely derive the absolute height of the image from the source slice
    let global_height = region.src_channels[0].len() / width;

    // Resolve the function pointer once per chunk
    let ptr: fn(&[T]) -> T = match operations {
        SpatialOperations::Contrast => find_contrast::<T>,
        SpatialOperations::Maximum => find_max::<T>,
        SpatialOperations::Gradient => find_gradient::<T>,
        SpatialOperations::Minimum => find_min::<T>,
        SpatialOperations::Mean => find_mean::<T>,
    };

    let neighbourhood_len = (2 * radius + 1) * (2 * radius + 1);

    // Thread-local buffer allocation.
    let mut buf = vec![T::default(); neighbourhood_len];

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            for x in 0..width {
                // Pass global_y and global_height so clamping math works correctly
                collect_neighbourhood(src, width, global_height, x, global_y, radius, &mut buf);
                dest[local_y * width + x] = ptr(&buf);
            }
        }
    }
}

/// Collect the `(2*radius+1)²` neighbourhood around `(cx, cy)` into `buf`,
/// clamping out-of-bounds coordinates to replicate edge pixels.
#[inline(always)]
fn collect_neighbourhood<T: Copy>(
    src: &[T], width: usize, global_height: usize, cx: usize, cy: usize, radius: usize,
    buf: &mut [T],
) {
    let diameter = 2 * radius + 1;
    let mut i = 0;

    for ky in 0..diameter {
        let sy = (cy + ky).saturating_sub(radius).min(global_height - 1);
        let row_offset = sy * width;

        for kx in 0..diameter {
            let sx = (cx + kx).saturating_sub(radius).min(width - 1);
            buf[i] = src[row_offset + sx];
            i += 1;
        }
    }
}
