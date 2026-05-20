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

    let global_height = region.src_channels[0].len() / width;

    let ptr: fn(&[T]) -> T = match operations {
        SpatialOperations::Contrast => find_contrast::<T>,
        SpatialOperations::Maximum => find_max::<T>,
        SpatialOperations::Gradient => find_gradient::<T>,
        SpatialOperations::Minimum => find_min::<T>,
        SpatialOperations::Mean => find_mean::<T>,
    };

    let diameter = 2 * radius + 1;
    let neighbourhood_len = diameter * diameter;
    let mut buf = vec![T::default(); neighbourhood_len];

    for (src, dest) in region
        .src_channels
        .iter()
        .zip(region.dest_channels.iter_mut())
    {
        for local_y in 0..region.height {
            let global_y = region.y_offset + local_y;

            // 1. Initialize the full neighbourhood at the start of the row (x = 0)
            init_neighbourhood(src, width, global_height, 0, global_y, radius, &mut buf);
            dest[local_y * width] = ptr(&buf);

            // 2. Slide the window for the rest of the row
            for x in 1..width {
                slide_neighbourhood(src, width, global_height, x, global_y, radius, &mut buf);
                dest[local_y * width + x] = ptr(&buf);
            }
        }
    }
}

/// Collect the `(2*radius+1)²` neighbourhood from scratch at `(cx, cy)` into `buf`.
#[inline(always)]
fn init_neighbourhood<T: Copy>(
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

/// Slide the `(2*radius+1)²` neighbourhood one pixel to the right.
/// Discards the leftmost column and fetches the new rightmost column.
#[inline(always)]
fn slide_neighbourhood<T: Copy>(
    src: &[T], width: usize, global_height: usize, cx: usize, cy: usize, radius: usize,
    buf: &mut [T],
) {
    let diameter = 2 * radius + 1;

    // Calculate the x-coordinate of the *new* rightmost column, clamping to the edge.
    let sx = (cx + radius).min(width - 1);

    for ky in 0..diameter {
        let sy = (cy + ky).saturating_sub(radius).min(global_height - 1);
        let row_offset = sy * width;
        let row_start = ky * diameter;

        // Shift the current row left by 1 pixel (memmove under the hood)
        buf.copy_within((row_start + 1)..(row_start + diameter), row_start);

        // Insert the newly exposed pixel at the end of the row
        buf[row_start + diameter - 1] = src[row_offset + sx];
    }
}