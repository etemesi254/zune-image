use crate::traits::NumOps;
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;

fn horizontal_blur_region_fast_out<T>(region: &mut PlanarRegionOut<'_, T>, radius: usize)
where
    T: Default + Copy + Clone,
    T: NumOps<T>,
{
    let width = region.width;
    let height = region.height;
    let y_offset = region.y_offset;

    if width <= 1 || radius == 0 {
        return;
    }
    // important on the const generics part of fast_gaussian_inner
    const N: usize = 4;

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        // Slice out the exact input rows this thread is responsible for
        let start_idx = y_offset * width;
        let end_idx = start_idx + (height * width);
        let src_chunk = &src_channel[start_idx..end_idx];

        let chunk_size = width * N;

        let mut src_iter = src_chunk.chunks_exact(chunk_size);
        let mut dest_iter = dest_channel.chunks_exact_mut(chunk_size);

        // Process N=4 rows simultaneously
        for (in_chunk, out_chunk) in src_iter.by_ref().zip(dest_iter.by_ref()) {
            let (s0, rest) = in_chunk.split_at(width);
            let (s1, rest) = rest.split_at(width);
            let (s2, s3) = rest.split_at(width);
            let in_rows = [s0, s1, s2, s3];

            let (r0, rest) = out_chunk.split_at_mut(width);
            let (r1, rest) = rest.split_at_mut(width);
            let (r2, r3) = rest.split_at_mut(width);
            let mut out_rows = [r0, r1, r2, r3];

            fast_gaussian_inner_nx::<_, N>(&in_rows, &mut out_rows, width, radius);
        }

        // Clean up remaining rows
        for (in_row, out_row) in src_iter
            .remainder()
            .chunks_exact(width)
            .zip(dest_iter.into_remainder().chunks_exact_mut(width))
        {
            fast_gaussian_inner_nx(&[in_row], &mut [out_row], width, radius);
        }
    }
}
fn fast_gaussian_inner_nx<T, const N: usize>(
    in_rows: &[&[T]; N], out_rows: &mut [&mut [T]; N], width: usize, radius: usize,
) where
    T: Copy,
    T: NumOps<T>,
{
    const RING_MASK: usize = 1023;

    // A Fast Gaussian weight approximation
    let area = (radius * radius) as i32;
    let initial_sum = area >> 1; // Used to handle rounding/biases

    // 1. Initialize our signed accumulators
    let mut diffs = [0i32; N];
    let mut summs = [initial_sum; N];

    // 2. The Ring Buffer (Size MUST be a power of 2, e.g., 1024)
    // This assumes the maximum supported radius is < 512.
    let mut ring_buffer = [[0i32; N]; 1024];

    // OPTIMIZER HINT: Elide bounds checking for the inner loop
    for i in 0..N {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    // 3. Single Pass Loop
    // We start from a negative index so the sliding window can "fill up"
    // before the center reaches pixel 0.
    let start_x = -(radius as isize) * 2;
    let width_isize = width as isize;
    let radius_isize = radius as isize;

    for x in start_x..width_isize {
        // --- A. WRITE AND TRAILING EDGE SUBTRACTION ---
        if x >= 0 {
            let ux = x as usize;

            // Calculate final pixel value and write to output
            for i in 0..N {
                let blurred_val = summs[i] / area;
                out_rows[i][ux] = T::from_i32(blurred_val);
            }

            // Fetch the old values falling out of the back of the blur windows
            let trail_idx_1 = ((x - radius_isize) as usize) & RING_MASK;
            let trail_idx_2 = ux & RING_MASK;

            let a_stored = ring_buffer[trail_idx_1];
            let d_stored = ring_buffer[trail_idx_2];

            // Update diffs: Add the far trailing pixel, subtract 2x the middle trailing pixel
            for i in 0..N {
                diffs[i] += a_stored[i] - (d_stored[i] * 2);
            }
        } else if x + radius_isize >= 0 {
            // Partial window: we only subtract the middle trailing pixel
            let trail_idx = (x as usize) & RING_MASK;
            let stored = ring_buffer[trail_idx];
            for i in 0..N {
                diffs[i] -= stored[i] * 2;
            }
        }

        // --- B. LEADING EDGE ADDITION ---
        // Fetch the incoming pixel. Clamp to the edge if we read past the image width.
        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;

        let ring_idx = ((x + radius_isize) as usize) & RING_MASK;

        for i in 0..N {
            let pixel_val = in_rows[i][next_x].to_i32();

            // 1. Store incoming pixel in the ring buffer for later subtraction
            ring_buffer[ring_idx][i] = pixel_val;

            // 2. Add incoming pixel to first integral (diffs)
            diffs[i] += pixel_val;

            // 3. Add first integral to second integral (summs)
            summs[i] += diffs[i];
        }
    }
}

fn horizontal_blur_region_fast_out_f32(region: &mut PlanarRegionOut<'_, f32>, radius: usize) {
    let width = region.width;
    let height = region.height;
    let y_offset = region.y_offset;

    if width <= 1 || radius == 0 {
        return;
    }

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let start_idx = y_offset * width;
        let end_idx = start_idx + (height * width);
        let src_chunk = &src_channel[start_idx..end_idx];

        const N: usize = 4;
        let chunk_size = width * N;

        let mut src_iter = src_chunk.chunks_exact(chunk_size);
        let mut dest_iter = dest_channel.chunks_exact_mut(chunk_size);

        for (in_chunk, out_chunk) in src_iter.by_ref().zip(dest_iter.by_ref()) {
            let (s0, rest) = in_chunk.split_at(width);
            let (s1, rest) = rest.split_at(width);
            let (s2, s3) = rest.split_at(width);
            let in_rows = [s0, s1, s2, s3];

            let (r0, rest) = out_chunk.split_at_mut(width);
            let (r1, rest) = rest.split_at_mut(width);
            let (r2, r3) = rest.split_at_mut(width);
            let mut out_rows = [r0, r1, r2, r3];

            fast_gaussian_inner_nx_f32(&in_rows, &mut out_rows, width, radius);
        }

        // Clean up remaining rows
        for (in_row, out_row) in src_iter
            .remainder()
            .chunks_exact(width)
            .zip(dest_iter.into_remainder().chunks_exact_mut(width))
        {
            fast_gaussian_inner_nx_f32(&[in_row], &mut [out_row], width, radius);
        }
    }
}

fn fast_gaussian_inner_nx_f32<const N: usize>(
    in_rows: &[&[f32]; N], out_rows: &mut [&mut [f32]; N], width: usize, radius: usize,
) {
    let area = (radius * radius) as f32;
    let weight = 1.0 / area; // Float multiplication is faster than division

    let mut diffs = [0.0f32; N];
    let mut summs = [0.0f32; N];

    const RING_MASK: usize = 1023;
    let mut ring_buffer = [[0.0f32; N]; 1024];

    for i in 0..N {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    let start_x = -(radius as isize) * 2;
    let width_isize = width as isize;
    let radius_isize = radius as isize;

    for x in start_x..width_isize {
        if x >= 0 {
            let ux = x as usize;

            for i in 0..N {
                out_rows[i][ux] = summs[i] * weight;
            }

            let trail_idx_1 = ((x - radius_isize) as usize) & RING_MASK;
            let trail_idx_2 = ux & RING_MASK;

            let a_stored = ring_buffer[trail_idx_1];
            let d_stored = ring_buffer[trail_idx_2];

            for i in 0..N {
                diffs[i] += a_stored[i] - (d_stored[i] * 2.0);
            }
        } else if x + radius_isize >= 0 {
            let trail_idx = (x as usize) & RING_MASK;
            let stored = ring_buffer[trail_idx];
            for i in 0..N {
                diffs[i] -= stored[i] * 2.0;
            }
        }

        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) & RING_MASK;

        for i in 0..N {
            let pixel_val = in_rows[i][next_x];
            ring_buffer[ring_idx][i] = pixel_val;
            diffs[i] += pixel_val;
            summs[i] += diffs[i];
        }
    }
}
pub fn vertical_blur_region_fast_out<T>(region: &mut PlanarRegionOut<'_, T>, radius: usize)
where
    T: Default + Copy + Clone,
    T: NumOps<T>,
{
    let width = region.width;
    let current_height = region.height;
    let full_height = region.src_channels[0].len() / width;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    debug_assert!(radius < 512, "radius {radius} exceeds ring buffer capacity of 511");

    let area = (radius * radius) as i32;
    let initial_sum = area >> 1;
    const RING_MASK: usize = 1023;
    const BLOCK: usize = 8;

    let radius_isize = radius as isize;
    let full_height_isize = full_height as isize;
    let start_y = (y_offset as isize) - (radius_isize * 2);
    let end_y = (y_offset + current_height) as isize;

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        // --- Blocked columns ---
        let full_blocks = width / BLOCK;

        for block in 0..full_blocks {
            let x_base = block * BLOCK;

            let mut diffs = [0i32; BLOCK];
            let mut summs = [initial_sum; BLOCK];
            let mut ring_buffer = [[0i32; BLOCK]; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + x_base;
                        for col in 0..BLOCK {
                            dest_channel[dest_base + col] =
                                T::from_i32(summs[col] / area);
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..BLOCK {
                        diffs[col] += a[col] - (d[col] * 2);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..BLOCK {
                        diffs[col] -= stored[col] * 2;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + x_base;

                for col in 0..BLOCK {
                    let pixel_val = src_channel[src_base + col].to_i32();
                    ring_buffer[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }

        // --- Tail: remaining columns < BLOCK ---
        let tail_start = full_blocks * BLOCK;
        let tail_len = width - tail_start;

        if tail_len > 0 {
            let mut diffs = [0i32; BLOCK];
            let mut summs = [initial_sum; BLOCK];
            let mut ring_buffer = [[0i32; BLOCK]; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + tail_start;
                        for col in 0..tail_len {
                            dest_channel[dest_base + col] =
                                T::from_i32(summs[col] / area);
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..tail_len {
                        diffs[col] += a[col] - (d[col] * 2);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * 2;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + tail_start;

                for col in 0..tail_len {
                    let pixel_val = src_channel[src_base + col].to_i32();
                    ring_buffer[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }
    }
}


pub fn vertical_blur_region_fast_out_f32(region: &mut PlanarRegionOut<'_, f32>, radius: usize) {
    let width = region.width;
    let current_height = region.height;
    let full_height = region.src_channels[0].len() / width;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    debug_assert!(radius < 512, "radius {radius} exceeds ring buffer capacity of 511");

    let area = (radius * radius) as f32;
    let weight = 1.0 / area;
    const RING_MASK: usize = 1023;
    const BLOCK: usize = 8;

    let radius_isize = radius as isize;
    let full_height_isize = full_height as isize;
    let start_y = (y_offset as isize) - (radius_isize * 2);
    let end_y = (y_offset + current_height) as isize;

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let full_blocks = width / BLOCK;

        for block in 0..full_blocks {
            let x_base = block * BLOCK;

            let mut diffs = [0.0f32; BLOCK];
            let mut summs = [0.0f32; BLOCK];
            let mut ring_buffer = [[0.0f32; BLOCK]; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + x_base;
                        for col in 0..BLOCK {
                            dest_channel[dest_base + col] = summs[col] * weight;
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..BLOCK {
                        diffs[col] += a[col] - (d[col] * 2.0);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..BLOCK {
                        diffs[col] -= stored[col] * 2.0;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + x_base;

                for col in 0..BLOCK {
                    let pixel_val = src_channel[src_base + col];
                    ring_buffer[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }

        // --- Tail ---
        let tail_start = full_blocks * BLOCK;
        let tail_len = width - tail_start;

        if tail_len > 0 {
            let mut diffs = [0.0f32; BLOCK];
            let mut summs = [0.0f32; BLOCK];
            let mut ring_buffer = [[0.0f32; BLOCK]; 1024];

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + tail_start;
                        for col in 0..tail_len {
                            dest_channel[dest_base + col] = summs[col] * weight;
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..tail_len {
                        diffs[col] += a[col] - (d[col] * 2.0);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * 2.0;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + tail_start;

                for col in 0..tail_len {
                    let pixel_val = src_channel[src_base + col];
                    ring_buffer[ring_idx][col] = pixel_val;
                    diffs[col] += pixel_val;
                    summs[col] += diffs[col];
                }
            }
        }
    }
}

pub fn sigma_to_radius_2pass(sigma: f32) -> usize {
    if sigma <= 0.0 {
        return 0;
    }

    // Using n = 2 for the double-accumulator
    let exact_radius = ((6.0 * sigma * sigma + 1.0).sqrt() - 1.0) / 2.0;

    // Round to the nearest integer, as pixel radii must be whole numbers
    exact_radius.round() as usize
}

// Adapter for the image
pub(crate) fn impl_fast_gaussian_blur(sigma: f32, image: &mut Image) -> Result<(), ImageErrors> {
    let depth = image.depth();

    let ignore_alpha = false;
    let radius = sigma_to_radius_2pass(sigma);
    assert!(radius < 512); // TODO: Support more raidus/dispatch to one that can do that radius

    // 1. Allocate our scratch image upfront for Out-Of-Place processing
    let mut scratch_img = image.clone();

    match depth.bit_type() {
        BitType::U8 => {
            // Pass 1: Horizontal (image -> scratch_img)
            image.par_process_regions_out_of_place::<u8, _>(
                &mut scratch_img,
                ignore_alpha,
                |region| {
                    horizontal_blur_region_fast_out(region, radius);
                },
            )?;

            // Pass 2: Vertical (scratch_img -> image)
            scratch_img.par_process_regions_out_of_place::<u8, _>(
                image,
                ignore_alpha,
                |region| {
                    vertical_blur_region_fast_out(region, radius);
                },
            )?;
        }
        BitType::U16 => {
            image.par_process_regions_out_of_place::<u16, _>(
                &mut scratch_img,
                ignore_alpha,
                |region| {
                    horizontal_blur_region_fast_out(region, radius);
                },
            )?;
            scratch_img.par_process_regions_out_of_place::<u16, _>(
                image,
                ignore_alpha,
                |region| {
                    vertical_blur_region_fast_out(region, radius);
                },
            )?;
        }
        BitType::F32 => {
            // Pass 1: Horizontal (image -> scratch_img)
            image.par_process_regions_out_of_place(&mut scratch_img, ignore_alpha, |region| {
                horizontal_blur_region_fast_out_f32(region, radius);
            })?;

            // Pass 2: Vertical (scratch_img -> image)
            scratch_img.par_process_regions_out_of_place(image, ignore_alpha, |region| {
                vertical_blur_region_fast_out_f32(region, radius);
            })?;
        }
        d => {
            return Err(ImageErrors::ImageOperationNotImplemented(
                "Gaussian Blur",
                d,
            ))
        }
    }

    Ok(())
}
