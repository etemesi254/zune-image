use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;
use crate::utils::as_mut_array;
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionOut;

const RING_SIZE: usize = 1024;

mod aarch64;
mod x86_64;
// 1. Define a trait to associate a pixel type with its ideal fast accumulator
pub trait BlurAccumulator: Copy + Default {
    type Accum: Copy
        + Default
        + From<i32>
        + std::ops::AddAssign
        + std::ops::SubAssign
        + std::ops::Add<Output = Self::Accum>
        + std::ops::Sub<Output = Self::Accum>
        + std::ops::Mul<Output = Self::Accum>
        + std::ops::Div<Output = Self::Accum>;

    fn to_accum(self) -> Self::Accum;
    fn from_accum(val: Self::Accum) -> Self;

    fn div_by_mod(val: Self::Accum, other: u128) -> Self::Accum;
}

impl BlurAccumulator for u8 {
    type Accum = i32; // u8 is perfectly safe in 32-bit (Full SIMD Speed!)
    #[inline(always)]
    fn to_accum(self) -> Self::Accum {
        i32::from(self)
    }
    #[inline(always)]
    fn from_accum(val: Self::Accum) -> Self {
        val as u8
    }
    #[inline(always)]
    fn div_by_mod(val: Self::Accum, other: u128) -> Self::Accum {
        fastdiv_u32(val as _, other) as _
    }
}

impl BlurAccumulator for u16 {
    type Accum = i64; // u16 MUST use 64-bit to prevent overflow on large radii
    #[inline(always)]
    fn to_accum(self) -> Self::Accum {
        i64::from(self)
    }
    #[inline(always)]
    fn from_accum(val: Self::Accum) -> Self {
        val as u16
    }
    #[inline(always)]
    fn div_by_mod(val: Self::Accum, other: u128) -> Self::Accum {
        fastdiv_u32(val as _, other) as _
    }
}

fn horizontal_blur_region_fast_out_u8(region: &mut PlanarRegionOut<'_, u8>, radius: usize) {
    const N: usize = 4;

    let width = region.width;
    let height = region.height;
    let y_offset = region.y_offset;

    if width <= 1 || radius == 0 {
        return;
    }

    let mut ring_buffer = vec![[0_i32; 4]; 1024];
    let slice = as_mut_array(&mut ring_buffer).unwrap();

    let mut smaller_ring_buffer = vec![[0_i32; 1]; 1024];

    let remainder_ring = as_mut_array(&mut smaller_ring_buffer).unwrap();

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

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
            #[cfg(target_arch = "aarch64")]
            {
                if std::arch::is_aarch64_feature_detected!("neon") {
                    unsafe {
                        aarch64::horizontal_blur_gaussian_inner_u8_neon(
                            &in_rows,
                            slice,
                            &mut out_rows,
                            width,
                            radius,
                        );
                    }
                    continue;
                }
            }
            #[cfg(target_arch = "x86_64")]
            {
                if std::arch::is_x86_feature_detected!("sse2") {
                    unsafe {
                        x86_64::horizontal_blur_gaussian_inner_u8_sse(
                            &in_rows,
                            slice,
                            &mut out_rows,
                            width,
                            radius,
                        )
                    }
                    continue;
                }
            }

            fast_gaussian_inner_nx::<_, N>(&in_rows, slice, &mut out_rows, width, radius);
        }

        // Clean up remaining rows
        for (in_row, out_row) in src_iter
            .remainder()
            .chunks_exact(width)
            .zip(dest_iter.into_remainder().chunks_exact_mut(width))
        {
            fast_gaussian_inner_nx(&[in_row], remainder_ring, &mut [out_row], width, radius);
        }
    }
}
fn horizontal_blur_region_fast_out<T>(region: &mut PlanarRegionOut<'_, T>, radius: usize)
where
    T: Default + Copy + Clone + BlurAccumulator,
    T: NumOps<T>,
{
    const N: usize = 4;

    let width = region.width;
    let height = region.height;
    let y_offset = region.y_offset;

    if width <= 1 || radius == 0 {
        return;
    }

    // Allocate buffer using the generic accumulator type
    let mut ring_buffer = vec![[T::Accum::default(); N]; 1024];
    let mut smaller_ring_buffer = vec![[T::Accum::default(); 1]; 1024];

    let remainder_ring = as_mut_array(&mut smaller_ring_buffer).unwrap();

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let start_idx = y_offset * width;
        let end_idx = start_idx + (height * width);
        let src_chunk = &src_channel[start_idx..end_idx];

        let chunk_size = width * N;

        let mut src_iter = src_chunk.chunks_exact(chunk_size);
        let mut dest_iter = dest_channel.chunks_exact_mut(chunk_size);

        let slice = as_mut_array(&mut ring_buffer).unwrap();

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

            fast_gaussian_inner_nx::<_, N>(&in_rows, slice, &mut out_rows, width, radius);
        }

        // Clean up remaining rows
        for (in_row, out_row) in src_iter
            .remainder()
            .chunks_exact(width)
            .zip(dest_iter.into_remainder().chunks_exact_mut(width))
        {
            fast_gaussian_inner_nx(&[in_row], remainder_ring, &mut [out_row], width, radius);
        }
    }
}

fn fast_gaussian_inner_nx<T, const N: usize>(
    in_rows: &[&[T]; N], ring_buffer: &mut [[T::Accum; N]; RING_SIZE],
    out_rows: &mut [&mut [T]; N], width: usize, radius: usize,
) where
    T: Copy + BlurAccumulator,
    T: NumOps<T>,
{
    let area_i32 = (radius * radius) as i32;
    let initial_sum = T::Accum::from(area_i32 >> 1);
    let two = T::Accum::from(2);

    let mut diffs = [T::Accum::default(); N];
    let mut summs = [initial_sum; N];

    ring_buffer.fill([T::Accum::default(); N]);
    let special_num = compute_mod_u32(area_i32 as u64);

    for i in 0..N {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    let start_x = -radius.cast_signed() * 2;
    let width_isize = width.cast_signed();
    let radius_isize = radius.cast_signed();

    for x in start_x..width_isize {
        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;

        if x >= 0 {
            let ux = x as usize;
            let trail_idx_1 = ((x - radius_isize) as usize) % RING_SIZE;
            let trail_idx_2 = ux % RING_SIZE;

            let a_stored = ring_buffer[trail_idx_1];
            let d_stored = ring_buffer[trail_idx_2];

            // Single pass: output + diff update + ring write + summ update
            for i in 0..N {
                let blurred_val = T::div_by_mod(summs[i], special_num);
                out_rows[i][ux] = T::from_accum(blurred_val);

                let pixel_val = in_rows[i][next_x].to_accum();
                ring_buffer[ring_idx][i] = pixel_val;

                diffs[i] = (a_stored[i] - (d_stored[i] * two) + pixel_val) + diffs[i];
                summs[i] = diffs[i] + summs[i];
            }
        } else if x + radius_isize >= 0 {
            let trail_idx = (x as usize) % RING_SIZE;
            let stored = ring_buffer[trail_idx];

            // Single pass: diff update + ring write + summ update
            for i in 0..N {
                let pixel_val = in_rows[i][next_x].to_accum();
                ring_buffer[ring_idx][i] = pixel_val;

                diffs[i] = (pixel_val - (stored[i] * two)) + diffs[i];
                summs[i] = (diffs[i]) + summs[i];
            }
        } else {
            // Single pass: ring write + summ update, no trail reads
            for i in 0..N {
                let pixel_val = in_rows[i][next_x].to_accum();
                ring_buffer[ring_idx][i] = pixel_val;

                diffs[i] += pixel_val;
                summs[i] += diffs[i];
            }
        }
    }
}

fn horizontal_blur_region_fast_out_f32(region: &mut PlanarRegionOut<'_, f32>, radius: usize) {
    const N: usize = 4;

    let width = region.width;
    let height = region.height;
    let y_offset = region.y_offset;

    if width <= 1 || radius == 0 {
        return;
    }

    let mut ring_buffer = vec![[0.0f32; 4]; 1024];
    let mut smaller_ring_buffer = vec![[0.0f32]; 1024];
    let remainder_ring = as_mut_array(&mut smaller_ring_buffer).unwrap();
    let ring_buffer_correct_type = as_mut_array(&mut ring_buffer).unwrap();

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let start_idx = y_offset * width;
        let end_idx = start_idx + (height * width);
        let src_chunk = &src_channel[start_idx..end_idx];

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

            fast_gaussian_inner_nx_f32(
                &in_rows,
                ring_buffer_correct_type,
                &mut out_rows,
                width,
                radius,
            );
        }

        for (in_row, out_row) in src_iter
            .remainder()
            .chunks_exact(width)
            .zip(dest_iter.into_remainder().chunks_exact_mut(width))
        {
            fast_gaussian_inner_nx_f32(&[in_row], remainder_ring, &mut [out_row], width, radius);
        }
    }
}

fn fast_gaussian_inner_nx_f32<const N: usize>(
    in_rows: &[&[f32]; N], ring_buffer: &mut [[f32; N]; RING_SIZE], out_rows: &mut [&mut [f32]; N],
    width: usize, radius: usize,
) {
    let area = (radius * radius) as f32;
    let weight = 1.0 / area;

    let mut diffs = [0.0f32; N];
    let mut summs = [0.0f32; N];

    ring_buffer.fill([0.0f32; N]);
    for i in 0..N {
        assert!(in_rows[i].len() >= width);
        assert!(out_rows[i].len() >= width);
    }

    let start_x = -radius.cast_signed() * 2;
    let width_isize = width.cast_signed();
    let radius_isize = radius.cast_signed();

    for x in start_x..width_isize {
        if x >= 0 {
            let ux = x as usize;

            for i in 0..N {
                out_rows[i][ux] = summs[i] * weight;
            }

            let trail_idx_1 = ((x - radius_isize) as usize) % RING_SIZE;
            let trail_idx_2 = ux % RING_SIZE;

            let a_stored = ring_buffer[trail_idx_1];
            let d_stored = ring_buffer[trail_idx_2];

            for i in 0..N {
                diffs[i] += a_stored[i] - (d_stored[i] * 2.0);
            }
        } else if x + radius_isize >= 0 {
            let trail_idx = (x as usize) % RING_SIZE;
            let stored = ring_buffer[trail_idx];
            for i in 0..N {
                diffs[i] -= stored[i] * 2.0;
            }
        }

        let next_x = (x + radius_isize).clamp(0, width_isize - 1) as usize;
        let ring_idx = ((x + radius_isize) as usize) % RING_SIZE;

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
    T: Default + Copy + Clone + BlurAccumulator,
    T: NumOps<T>,
{
    const RING_MASK: usize = 1023;
    const BLOCK: usize = 8;

    let width = region.width;
    let current_height = region.height;
    let full_height = region.src_channels[0].len() / width;
    let y_offset = region.y_offset;

    if current_height == 0 || radius == 0 {
        return;
    }

    let area_i32 = (radius * radius) as i32;
    let area = T::Accum::from(area_i32);
    let initial_sum = T::Accum::from(area_i32 >> 1);
    let two = T::Accum::from(2);

    let radius_isize = radius.cast_signed();
    let full_height_isize = full_height.cast_signed();
    let start_y = y_offset.cast_signed() - (radius_isize * 2);
    let end_y = (y_offset + current_height).cast_signed();

    let mut diffs = [T::Accum::default(); BLOCK];
    let mut summs = [initial_sum; BLOCK];
    let mut ring_buffer = vec![[T::Accum::default(); BLOCK]; 1024];

    for c in 0..region.src_channels.len() {
        let src_channel = region.src_channels[c];
        let dest_channel = &mut region.dest_channels[c];

        let full_blocks = width / BLOCK;

        for block in 0..full_blocks {
            let x_base = block * BLOCK;
            diffs.fill(T::Accum::default());
            summs.fill(initial_sum);
            ring_buffer.fill([T::Accum::default(); BLOCK]);

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset.cast_signed() {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + x_base;
                        for col in 0..BLOCK {
                            dest_channel[dest_base + col] = T::from_accum(summs[col] / area);
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..BLOCK {
                        diffs[col] += a[col] - (d[col] * two);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..BLOCK {
                        diffs[col] -= stored[col] * two;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + x_base;

                for col in 0..BLOCK {
                    let pixel_val = src_channel[src_base + col].to_accum();
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
            ring_buffer.fill([T::Accum::default(); BLOCK]);
            diffs.fill(T::Accum::default());
            summs.fill(initial_sum);

            for y in start_y..end_y {
                if y >= 0 {
                    let uy = y as usize;

                    if y >= y_offset as isize {
                        let local_y = uy - y_offset;
                        let dest_base = local_y * width + tail_start;
                        for col in 0..tail_len {
                            dest_channel[dest_base + col] = T::from_accum(summs[col] / area);
                        }
                    }

                    let trail_idx_1 = ((y - radius_isize) as usize) & RING_MASK;
                    let trail_idx_2 = uy & RING_MASK;
                    let a = ring_buffer[trail_idx_1];
                    let d = ring_buffer[trail_idx_2];
                    for col in 0..tail_len {
                        diffs[col] += a[col] - (d[col] * two);
                    }
                } else if y + radius_isize >= 0 {
                    let trail_idx = (y as usize) & RING_MASK;
                    let stored = ring_buffer[trail_idx];
                    for col in 0..tail_len {
                        diffs[col] -= stored[col] * two;
                    }
                }

                let next_y = (y + radius_isize).clamp(0, full_height_isize - 1) as usize;
                let ring_idx = ((y + radius_isize) as usize) & RING_MASK;
                let src_base = next_y * width + tail_start;

                for col in 0..tail_len {
                    let pixel_val = src_channel[src_base + col].to_accum();
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

    let area = (radius * radius) as f32;
    let weight = 1.0 / area;
    const RING_MASK: usize = 1023;
    const BLOCK: usize = 8;

    let radius_isize = radius.cast_signed();
    let full_height_isize = full_height.cast_signed();
    let start_y = y_offset.cast_signed() - (radius_isize * 2);
    let end_y = (y_offset + current_height).cast_signed();

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

                    if y >= y_offset.cast_signed() {
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

                    if y >= y_offset.cast_signed() {
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
    let exact_radius = (6.0 * sigma * sigma + 1.0).sqrt();
    exact_radius.round() as usize
}

// Adapter for the image
pub(crate) fn impl_fast_gaussian_blur(sigma: f32, image: &mut Image) -> Result<(), ImageErrors> {
    let depth = image.depth();

    let ignore_alpha = false;
    let radius = sigma_to_radius_2pass(sigma);

    // Swap the panicking assert for a safe library error
    if radius >= 512 {
        return Err(ImageErrors::GenericString(format!(
            "Gaussian Blur radius({radius}) >= 512"
        )));
    }
    if radius <= 1 {
        return Err(ImageErrors::GenericString(format!(
            "Gaussian Blur radius is too small at {radius}"
        )));
    }

    // 1. Allocate our scratch image upfront for Out-Of-Place processing
    let mut scratch_img = image.clone();

    match depth.bit_type() {
        BitType::U8 => {
            // Pass 1: Horizontal (image -> scratch_img)
            image.par_process_regions_out_of_place::<u8, _>(
                &mut scratch_img,
                ignore_alpha,
                |region| {
                    horizontal_blur_region_fast_out_u8(region, radius);
                },
            )?;

            // Pass 2: Vertical (scratch_img -> image)
            scratch_img.par_process_regions_out_of_place::<u8, _>(
                image,
                ignore_alpha,
                |region| {
                    #[cfg(target_arch = "aarch64")]
                    {
                        if std::arch::is_aarch64_feature_detected!("neon") {
                            unsafe {
                                aarch64::vertical_blur_region_u8_neon(region, radius);
                            }
                            return;
                        }
                    }
                    #[cfg(target_arch = "x86_64")]
                    {
                        if std::arch::is_x86_feature_detected!("avx2") {
                            unsafe {
                                x86_64::vertical_blur_region_u8_avx2(region, radius);
                            }
                            return;
                        }
                    }
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
