use crate::resize::ResizeMethod;
use crate::traits::NumOps;
use zune_image::planar_regions::PlanarRegionMut;

fn get_kernel_fn_and_radius(method: ResizeMethod) -> (fn(f32) -> f32, i32) {
    match method {
        ResizeMethod::Lanczos3 => (lanczos_kernel::<3>, 3),
        ResizeMethod::Lanczos2 => (lanczos_kernel::<2>, 2),
        ResizeMethod::Bicubic | ResizeMethod::Mitchell => {
            (|x| bicubic_kernel(x, 1.0 / 3.0, 1.0 / 3.0), 2)
        }
        ResizeMethod::CatmullRom => (|x| bicubic_kernel(x, 0.0, 0.5), 2),
        ResizeMethod::BSpline => (|x| bicubic_kernel(x, 1.0, 0.0), 2),
        ResizeMethod::Hermite => (|x| bicubic_kernel(x, 0.0, 0.0), 2),
        ResizeMethod::Sinc => (sinc_kernel::<3>, 3),
        ResizeMethod::Bilinear => (bilinear_kernel, 1),
    }
}

pub(crate) struct PrecomputedKernels {
    pub horizontal: Option<Vec<ConvKernel>>,
    pub vertical: Option<Vec<ConvKernel>>,
}

impl PrecomputedKernels {
    pub fn new(
        in_width: usize, in_height: usize, out_width: usize, out_height: usize,
        method: ResizeMethod,
    ) -> Self {
        PrecomputedKernels::new_separated_kernels(
            in_width, in_height, out_width, out_height, method, method,
        )
    }
    pub fn new_separated_kernels(
        in_width: usize, in_height: usize, out_width: usize, out_height: usize,
        horiz_method: ResizeMethod, vert_method: ResizeMethod,
    ) -> Self {
        let horizontal = if in_width == out_width {
            None
        } else {
            let (kernel_fn, radius) = get_kernel_fn_and_radius(horiz_method);

            let x_ratio = in_width as f32 / out_width as f32;
            Some(precompute_kernels(
                in_width, out_width, x_ratio, radius, kernel_fn,
            ))
        };

        let vertical = if in_height == out_height {
            None
        } else {
            let (kernel_fn, radius) = get_kernel_fn_and_radius(vert_method);
            let y_ratio = in_height as f32 / out_height as f32;
            Some(precompute_kernels(
                in_height, out_height, y_ratio, radius, kernel_fn,
            ))
        };

        PrecomputedKernels {
            horizontal,
            vertical,
        }
    }
}
/// A single convolution kernel for one output pixel.
/// Weights are heap-allocated so the kernel can be as wide as needed
/// (e.g. Lanczos3 at 10× downscale needs ~60 taps, not 6).
#[derive(Clone)]
pub(crate) struct ConvKernel {
    pub weights: Vec<f32>,
    pub weights_i32: Vec<i32>,
    pub start_idx: u32,
    pub end_idx: u32,
}

#[allow(clippy::needless_range_loop, clippy::cast_possible_wrap)]
fn precompute_kernels(
    in_size: usize,
    out_size: usize,
    ratio: f32, // in_size / out_size  (>1 = downscale)
    a: i32,     // base kernel radius in kernel-space taps
    kernel_fn: fn(f32) -> f32,
) -> Vec<ConvKernel> {
    // When downscaling the kernel must widen to cover the source pixels that
    // map to a single output pixel. `scale` is the stretch factor; for
    // upscaling it stays 1.0 so the kernel is not artificially narrowed.
    let scale = ratio.max(1.0_f32);

    // Effective radius in input-pixel units. Ceiling so we never under-sample.
    let scaled_a = (a as f32 * scale).ceil() as i32;

    let mut kernels = Vec::with_capacity(out_size);
    let in_size_i32 = in_size as i32;

    for out_pos in 0..out_size {
        // Centre of the output pixel mapped into input space.
        let src_pos = (out_pos as f32 + 0.5) * ratio - 0.5;
        let center = src_pos.floor() as i32;

        // Clamp the tap range to the valid input region.
        let start = (-scaled_a + 1).max(-center);
        let end = scaled_a.min(in_size_i32 - center - 1);

        let start_idx = (center + start) as usize;
        let end_idx = (center + end) as usize;
        let count = (end - start + 1) as usize;

        let mut weights = vec![0.0_f32; count];
        let mut weight_sum = 0.0_f32;

        for (i, delta) in (start..=end).enumerate() {
            // Normalise the distance into kernel-function space: divide by
            // `scale` so the kernel is evaluated at the same relative
            // position regardless of how many input pixels it covers.
            let distance = ((center + delta) as f32 - src_pos) / scale;
            let w = kernel_fn(distance);
            weights[i] = w;
            weight_sum += w;
        }

        // Normalise floating-point weights.
        if weight_sum.abs() > 1e-10 {
            let inv = 1.0 / weight_sum;
            for w in &mut weights {
                *w *= inv;
            }
        }

        // Convert to fixed-point Q16 (×65536) for the u8 fast path.
        let mut weights_i32 = vec![0_i32; count];
        let mut fixed_sum = 0_i32;
        for i in 0..count {
            let w = (weights[i] * 65536.0).round() as i32;
            weights_i32[i] = w;
            fixed_sum += w;
        }

        // Correct any rounding drift so the fixed-point weights sum exactly
        // to 65536, preventing brightness shifts on flat images.
        if count > 0 {
            let diff = 65536 - fixed_sum;
            // Add the residual to the tap with the largest absolute weight.
            let max_idx = weights_i32
                .iter()
                .enumerate()
                .max_by_key(|(_, &v)| v)
                .map_or(0, |(i, _)| i);
            weights_i32[max_idx] += diff;
        }
        kernels.push(ConvKernel {
            weights,
            weights_i32,
            start_idx: start_idx as u32,
            end_idx: end_idx as u32,
        });
    }

    kernels
}
pub fn resample_region_generic<T>(
    region: &mut PlanarRegionMut<'_, T>, src_channels: &[&[T]], in_width: usize,
    kernels: &PrecomputedKernels,
) where
    T: Copy + NumOps<T> + Send + Sync,
    f32: std::convert::From<T>,
{
    let out_width = region.width;
    let need_horizontal = kernels.horizontal.is_some();
    let need_vertical = kernels.vertical.is_some();

    // Loop over each channel independently within this chunk
    for (src_ch, dest_ch) in src_channels.iter().zip(region.channels.iter_mut()) {
        // 1. Vertical Only
        if !need_horizontal && need_vertical {
            let v_kernels = kernels.vertical.as_ref().unwrap();
            for local_y in 0..region.height {
                let out_y = region.y_offset + local_y;
                let kernel = &v_kernels[out_y];
                let out_row_offset = local_y * out_width;

                for x in 0..out_width {
                    let mut sum = 0.0_f32;
                    for in_y in kernel.start_idx..=kernel.end_idx {
                        let tap_idx = (in_y - kernel.start_idx) as usize;
                        let pixel = f32::from(src_ch[in_y as usize * in_width + x]);
                        sum += pixel * kernel.weights[tap_idx];
                    }
                    dest_ch[out_row_offset + x] = T::from_f32(sum);
                }
            }
            continue;
        }

        // 2. Horizontal Only
        if need_horizontal && !need_vertical {
            let h_kernels = kernels.horizontal.as_ref().unwrap();
            for local_y in 0..region.height {
                let out_y = region.y_offset + local_y;
                let in_row = &src_ch[out_y * in_width..out_y * in_width + in_width];
                let out_row = &mut dest_ch[local_y * out_width..local_y * out_width + out_width];

                for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
                    let start = kernel.start_idx as usize;
                    let count = (kernel.end_idx - kernel.start_idx + 1) as usize;
                    let sum = if let Some(slice) = in_row.get(start..start + count) {
                        slice
                            .iter()
                            .zip(kernel.weights.iter())
                            .map(|(&p, &w)| f32::from(p) * w)
                            .sum::<f32>()
                    } else {
                        0.0
                    };
                    *out_pixel = T::from_f32(sum);
                }
            }
            continue;
        }

        // 3. Fused Separable Convolution (Horizontal + Vertical)
        if need_horizontal && need_vertical {
            let h_kernels = kernels.horizontal.as_ref().unwrap();
            let v_kernels = kernels.vertical.as_ref().unwrap();

            let max_v_taps = v_kernels
                .iter()
                .map(|k| (k.end_idx - k.start_idx + 1) as usize)
                .max()
                .unwrap_or(1);
            let special_div = crate::mathops::compute_mod_u32(max_v_taps as u64);

            let mut ring_buffer = vec![0.0_f32; max_v_taps * out_width];
            let mut buffer_contents = vec![usize::MAX; max_v_taps];
            let mut row_accumulator = vec![0.0_f32; out_width];

            let v_kernels_chunk = &v_kernels[region.y_offset..region.y_offset + region.height];

            for (v_kernel, out_row) in v_kernels_chunk
                .iter()
                .zip(dest_ch.chunks_exact_mut(out_width))
            {
                let v_start = v_kernel.start_idx as usize;
                let v_end = v_kernel.end_idx as usize;

                // Horizontal passes into the ring buffer
                for in_y in v_start..=v_end {
                    let buffer_idx =
                        crate::mathops::fastmod_u32(in_y as u32, special_div, max_v_taps as u32)
                            as usize;

                    if buffer_contents[buffer_idx] != in_y {
                        let in_row = &src_ch[in_y * in_width..in_y * in_width + in_width];
                        let ring_row = &mut ring_buffer
                            [buffer_idx * out_width..buffer_idx * out_width + out_width];

                        for (out_pixel, kernel) in ring_row.iter_mut().zip(h_kernels.iter()) {
                            let start = kernel.start_idx as usize;
                            let count = (kernel.end_idx - kernel.start_idx + 1) as usize;
                            *out_pixel = if let Some(slice) = in_row.get(start..start + count) {
                                slice
                                    .iter()
                                    .zip(kernel.weights.iter())
                                    .map(|(&p, &w)| f32::from(p) * w)
                                    .sum::<f32>()
                            } else {
                                0.0
                            };
                        }
                        buffer_contents[buffer_idx] = in_y;
                    }
                }

                // Vertical pass accumulating from the ring buffer
                row_accumulator.fill(0.0);
                for (i, in_y) in (v_start..=v_end).enumerate() {
                    let weight = v_kernel.weights[i];
                    let buffer_idx =
                        crate::mathops::fastmod_u32(in_y as u32, special_div, max_v_taps as u32)
                            as usize;
                    let ring_row =
                        &ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

                    for (acc, &rv) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                        *acc += rv * weight;
                    }
                }

                for (px, &acc) in out_row.iter_mut().zip(row_accumulator.iter()) {
                    *px = T::from_f32(acc);
                }
            }
        }
    }
}

pub fn resample_region_u8(
    region: &mut PlanarRegionMut<'_, u8>, src_channels: &[&[u8]], in_width: usize,
    kernels: &PrecomputedKernels,
) {
    let out_width = region.width;
    let h_kernels = kernels.horizontal.as_ref().unwrap();
    let v_kernels = kernels.vertical.as_ref().unwrap();

    let max_v_taps = v_kernels
        .iter()
        .map(|k| (k.end_idx - k.start_idx + 1) as usize)
        .max()
        .unwrap_or(1);
    let special_div = crate::mathops::compute_mod_u32(max_v_taps as u64);

    for (src_ch, dest_ch) in src_channels.iter().zip(region.channels.iter_mut()) {
        let mut ring_buffer = vec![0_i32; max_v_taps * out_width];
        let mut buffer_contents = vec![usize::MAX; max_v_taps];
        let mut row_accumulator = vec![0_i64; out_width];

        let v_kernels_chunk = &v_kernels[region.y_offset..region.y_offset + region.height];

        for (v_kernel, out_row) in v_kernels_chunk
            .iter()
            .zip(dest_ch.chunks_exact_mut(out_width))
        {
            let v_start = v_kernel.start_idx as usize;
            let v_end = v_kernel.end_idx as usize;

            for in_y in v_start..=v_end {
                let buffer_idx =
                    crate::mathops::fastmod_u32(in_y as u32, special_div, max_v_taps as u32)
                        as usize;

                if buffer_contents[buffer_idx] != in_y {
                    let in_row = &src_ch[in_y * in_width..in_y * in_width + in_width];
                    let ring_row = &mut ring_buffer
                        [buffer_idx * out_width..buffer_idx * out_width + out_width];

                    for (out_pixel, kernel) in ring_row.iter_mut().zip(h_kernels.iter()) {
                        let start = kernel.start_idx as usize;
                        let count = (kernel.end_idx - kernel.start_idx + 1) as usize;
                        *out_pixel = if let Some(slice) = in_row.get(start..start + count) {
                            slice
                                .iter()
                                .zip(kernel.weights_i32.iter())
                                .map(|(&p, &w)| i32::from(p) * w)
                                .sum::<i32>()
                        } else {
                            0
                        };
                    }
                    buffer_contents[buffer_idx] = in_y;
                }
            }

            row_accumulator.fill(0);

            for (i, in_y) in (v_start..=v_end).enumerate() {
                let weight = i64::from(v_kernel.weights_i32[i]);
                let buffer_idx =
                    crate::mathops::fastmod_u32(in_y as u32, special_div, max_v_taps as u32)
                        as usize;
                let ring_row =
                    &ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

                for (acc, &rv) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                    *acc += i64::from(rv) * weight;
                }
            }

            // Shift Q32 -> u8 with rounding and clamp
            for (px, &acc) in out_row.iter_mut().zip(row_accumulator.iter()) {
                *px = ((acc + (1_i64 << 31)) >> 32).clamp(0, 255) as u8;
            }
        }
    }
}

// ============================================================================
// Kernel functions
// ============================================================================

/// Lanczos kernel with lobed parameter A.
#[inline]
fn lanczos_kernel<const A: i32>(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        return 1.0;
    }
    let a = A as f32;
    if x < a {
        let pi_x = std::f32::consts::PI * x;
        let pi_x_a = pi_x / a;
        (pi_x.sin() / pi_x) * (pi_x_a.sin() / pi_x_a)
    } else {
        0.0
    }
}

/// Mitchell-Netravali bicubic family.
/// B=1/3, C=1/3 → Mitchell  |  B=0, C=1/2 → Catmull-Rom
/// B=1,   C=0   → B-Spline  |  B=0, C=0   → Hermite
#[inline]
fn bicubic_kernel(x: f32, b: f32, c: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 {
        let x2 = x * x;
        let x3 = x2 * x;
        ((12.0 - 9.0 * b - 6.0 * c) * x3 + (-18.0 + 12.0 * b + 6.0 * c) * x2 + (6.0 - 2.0 * b))
            / 6.0
    } else if x < 2.0 {
        let x2 = x * x;
        let x3 = x2 * x;
        ((-b - 6.0 * c) * x3
            + (6.0 * b + 30.0 * c) * x2
            + (-12.0 * b - 48.0 * c) * x
            + (8.0 * b + 24.0 * c))
            / 6.0
    } else {
        0.0
    }
}

/// Sinc kernel with window radius A.
#[inline]
fn sinc_kernel<const A: i32>(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        return 1.0;
    }
    let a = A as f32;
    if x < a {
        let pi_x = std::f32::consts::PI * x;
        pi_x.sin() / pi_x
    } else {
        0.0
    }
}

/// Bilinear (triangle) kernel.
#[inline]
fn bilinear_kernel(x: f32) -> f32 {
    let x = x.abs();
    if x < 1.0 {
        1.0 - x
    } else {
        0.0
    }
}
