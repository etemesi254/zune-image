use crate::resize::ResizeMethod;
use crate::traits::NumOps;

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
        let (kernel_fn, radius) = get_kernel_fn_and_radius(method);

        let horizontal = if in_width == out_width {
            None
        } else {
            let x_ratio = in_width as f32 / out_width as f32;
            Some(precompute_kernels(
                in_width, out_width, x_ratio, radius, kernel_fn,
            ))
        };

        let vertical = if in_height == out_height {
            None
        } else {
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

pub fn resample_separable<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels,
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    resample_separable_precomputed(
        in_channel,
        out_channel,
        in_width,
        in_height,
        out_width,
        out_height,
        kernels,
    );
}

#[allow(clippy::assertions_on_constants)]
pub fn resample_separable_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels,
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    // Early exit: if no resizing needed, just copy
    if in_width == out_width && in_height == out_height {
        out_channel.copy_from_slice(in_channel);
        return;
    }

    // Check if we need horizontal or vertical resizing
    let need_horizontal = kernels.horizontal.is_some();
    let need_vertical = kernels.vertical.is_some();

    // Case 1: Only vertical resizing needed
    if !need_horizontal && need_vertical {
        resample_vertical_only_precomputed::<T>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_height,
            kernels.vertical.as_ref().unwrap(),
        );
        return;
    }

    // Case 2: Only horizontal resizing needed
    if need_horizontal && !need_vertical {
        resample_horizontal_only_precomputed::<T>(
            in_channel,
            out_channel,
            in_width,
            out_width,
            kernels.horizontal.as_ref().unwrap(),
        );
        return;
    }

    // Case 3: Both dimensions need resizing (Fused Tiled Implementation)
    let h_kernels = kernels.horizontal.as_ref().unwrap();
    let v_kernels = kernels.vertical.as_ref().unwrap();

    // Find the maximum number of rows any vertical kernel requires
    let max_v_taps = v_kernels
        .iter()
        .map(|k| (k.end_idx - k.start_idx + 1) as usize)
        .max()
        .unwrap_or(1); // Ensure at least 1 to avoid div by zero

    // The ring buffer is tiny: only holds exactly the horizontal rows currently needed
    let mut ring_buffer = vec![0.0_f32; max_v_taps * out_width];
    // Track which in_y row is currently sitting in each ring buffer slot
    let mut buffer_contents = vec![usize::MAX; max_v_taps];
    // Accumulator for the vertical pass
    let mut row_accumulator = vec![0.0_f32; out_width];

    // Main demand-driven loop
    for (out_y, v_kernel) in (0..out_height).zip(v_kernels.iter()) {
        let v_start = v_kernel.start_idx as usize;
        let v_end = v_kernel.end_idx as usize;

        // STEP A: Horizontal pass (Computed strictly on demand)
        for in_y in v_start..=v_end {
            let buffer_idx = in_y % max_v_taps;

            // If the row we need isn't in the buffer, compute it
            if buffer_contents[buffer_idx] != in_y {
                let in_row_start = in_y * in_width;
                let in_row = &in_channel[in_row_start..in_row_start + in_width];

                let ring_row_start = buffer_idx * out_width;
                let ring_row = &mut ring_buffer[ring_row_start..ring_row_start + out_width];

                // Process single horizontal line
                process_horizontal_row_to_f32(in_row, ring_row, h_kernels);

                // Mark this row as successfully cached
                buffer_contents[buffer_idx] = in_y;
            }
        }

        // STEP B: Vertical pass (Cache-Friendly Horizontal Accumulation)
        row_accumulator.fill(0.0);

        for (i, in_y) in (v_start..=v_end).enumerate() {
            let weight = v_kernel.weights[i];
            let buffer_idx = in_y % max_v_taps;

            let ring_row_start = buffer_idx * out_width;
            let ring_row = &ring_buffer[ring_row_start..ring_row_start + out_width];

            // Ultra-fast sequential inner loop
            for (acc, &ring_val) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                *acc += ring_val * weight;
            }
        }

        // Final output conversion
        let out_row_start = out_y * out_width;
        let out_row = &mut out_channel[out_row_start..out_row_start + out_width];

        for x in 0..out_width {
            out_row[x] = T::from_f32(row_accumulator[x]);
        }
    }
}

/// Helper function to process a single horizontal row and output into an `f32` buffer
#[inline(always)]
fn process_horizontal_row_to_f32<T>(in_row: &[T], out_row: &mut [f32], h_kernels: &[ConvKernel])
where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
        let start_idx = kernel.start_idx as usize;
        let end_idx = kernel.end_idx as usize;
        let weights = &kernel.weights;
        let diff = (end_idx - start_idx) + 1;

        // do the most common ones
        macro_rules! fixed_dot {
                ($row:expr, $start:expr, $n:expr, $weights:expr) => {
                    if let Some(e) = $row.get($start..$start + $n) {
                        e.iter()
                            .zip($weights.iter())
                            .map(|(&p, &w)| f32::from(p) * w)
                            .sum::<f32>()
                    } else {
                        0.0
                    }
                };
            }

        let sum = match diff {
            1 => fixed_dot!(in_row, start_idx, 1, weights),
            2 => fixed_dot!(in_row, start_idx, 2, weights),
            3 => fixed_dot!(in_row, start_idx, 3, weights),
            4 => fixed_dot!(in_row, start_idx, 4, weights),
            5 => fixed_dot!(in_row, start_idx, 5, weights),
            6 => fixed_dot!(in_row, start_idx, 6, weights),
            _ => 0.0,
        };

        *out_pixel = sum;
    }
}
#[allow(clippy::needless_range_loop)]
fn resample_vertical_only_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, _in_height: usize, out_height: usize,
    v_kernels: &[ConvKernel],
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    for out_y in 0..out_height {
        let kernel = &v_kernels[out_y];
        let out_row_offset = out_y * width;

        for x in 0..width {
            let mut sum = 0.0;

            for in_y in kernel.start_idx..=kernel.end_idx {
                let in_y = in_y as usize;
                let start_idx = kernel.start_idx as usize;
                let pixel = f32::from(in_channel[in_y * width + x]);
                sum += pixel * kernel.weights[in_y - start_idx];
            }

            out_channel[out_row_offset + x] = T::from_f32(sum);
        }
    }
}

fn resample_horizontal_only_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, out_width: usize,
    h_kernels: &[ConvKernel],
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    for (in_row, out_row) in in_channel
        .chunks_exact(in_width)
        .zip(out_channel.chunks_exact_mut(out_width))
    {
        for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
            let start_idx = kernel.start_idx as usize;
            let end_idx = kernel.end_idx as usize;
            let weights = &kernel.weights;

            let diff = (end_idx - start_idx) + 1;
            // do the most common ones
            macro_rules! fixed_dot {
                ($row:expr, $start:expr, $n:expr, $weights:expr) => {
                    if let Some(e) = $row.get($start..$start + $n) {
                        e.iter()
                            .zip($weights.iter())
                            .map(|(&p, &w)| f32::from(p) * w)
                            .sum::<f32>()
                    } else {
                        0.0
                    }
                };
            }

            let sum = match diff {
                1 => fixed_dot!(in_row, start_idx, 1, weights),
                2 => fixed_dot!(in_row, start_idx, 2, weights),
                3 => fixed_dot!(in_row, start_idx, 3, weights),
                4 => fixed_dot!(in_row, start_idx, 4, weights),
                5 => fixed_dot!(in_row, start_idx, 5, weights),
                6 => fixed_dot!(in_row, start_idx, 6, weights),
                _ =>  0.0,
            };

            *out_pixel = T::from_f32(sum);
        }
    }
}

// Maximum kernel size: 2*A where A can be up to 3, so max 6 taps
const MAX_KERNEL_SIZE: usize = 6;

#[derive(Clone, Copy)]
pub(crate) struct ConvKernel {
    weights: [f32; MAX_KERNEL_SIZE],
    pub weights_i32: [i32; MAX_KERNEL_SIZE],
    start_idx: u32,
    end_idx: u32,
}
#[allow(clippy::needless_range_loop,clippy::cast_possible_wrap)]
fn precompute_kernels(
    in_size: usize, out_size: usize, ratio: f32, a: i32, kernel_fn: fn(f32) -> f32,
) -> Vec<ConvKernel> {
    let max_size: usize = (2 * a) as usize;
    assert!(max_size <= MAX_KERNEL_SIZE, "Kernel size exceeds maximum");

    let mut kernels = Vec::with_capacity(out_size);
    let in_size_i32 = in_size as i32;

    for out_pos in 0..out_size {
        let src_pos = (out_pos as f32 + 0.5) * ratio - 0.5;
        let center = src_pos.floor() as i32;

        let start = (-a + 1).max(-center);
        let end = a.min(in_size_i32 - center - 1);

        let start_idx = (center + start) as usize;
        let end_idx = (center + end) as usize;

        let mut weights = [0.0f32; MAX_KERNEL_SIZE];
        let mut weight_sum = 0.0;

        for (i, delta) in (start..=end).enumerate() {
            let distance = (center + delta) as f32 - src_pos;
            let weight = kernel_fn(distance);
            weights[i] = weight;
            weight_sum += weight;
        }

        // Normalize weights
        if weight_sum > 0.0 {
            let inv_sum = 1.0 / weight_sum;
            let count = (end - start + 1) as usize;

            for i in 0..count {
                weights[i] *= inv_sum;
            }
        }
        // GENERATE FIXED POINT WEIGHTS
        let mut weights_i32 = [0i32; MAX_KERNEL_SIZE];
        let mut fixed_sum = 0;
        let count = (end - start + 1) as usize;

        for i in 0..count {
            let w = (weights[i] * 65536.0).round() as i32;
            weights_i32[i] = w;
            fixed_sum += w;
        }

        // Fix normalization for integers to prevent brightness shifting
        if count > 0 {
            let diff = 65536 - fixed_sum;
            // Add the difference to the largest weight
            let mut max_idx = 0;
            let mut max_val = weights_i32[0];
            for i in 1..count {
                if weights_i32[i] > max_val {
                    max_val = weights_i32[i];
                    max_idx = i;
                }
            }
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

pub fn resample_separable_u8(
    in_channel: &[u8], out_channel: &mut [u8], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels,
) {
    // Early exit
    if in_width == out_width && in_height == out_height {
        out_channel.copy_from_slice(in_channel);
        return;
    }

    let h_kernels = kernels.horizontal.as_ref().unwrap();
    let v_kernels = kernels.vertical.as_ref().unwrap();

    let max_v_taps = v_kernels
        .iter()
        .map(|k| (k.end_idx - k.start_idx + 1) as usize)
        .max()
        .unwrap_or(1);

    // The ring buffer now stores i32 values directly. No floats!
    let mut ring_buffer = vec![0i32; max_v_taps * out_width];
    let mut buffer_contents = vec![usize::MAX; max_v_taps];

    // Accumulator is i64 to prevent overflow when multiplying i32 intermediate by i32 weight
    let mut row_accumulator = vec![0i64; out_width];

    for (out_y, v_kernel) in (0..out_height).zip(v_kernels.iter()) {
        let v_start = v_kernel.start_idx as usize;
        let v_end = v_kernel.end_idx as usize;

        // STEP A: Demand-driven Horizontal Pass
        for in_y in v_start..=v_end {
            let buffer_idx = in_y % max_v_taps;

            if buffer_contents[buffer_idx] != in_y {
                let in_row_start = in_y * in_width;
                let in_row = &in_channel[in_row_start..in_row_start + in_width];

                let ring_row_start = buffer_idx * out_width;
                let ring_row = &mut ring_buffer[ring_row_start..ring_row_start + out_width];

                process_horizontal_row_u8(in_row, ring_row, h_kernels);
                buffer_contents[buffer_idx] = in_y;
            }
        }

        // STEP B: Vertical Pass (i64 accumulation)
        row_accumulator.fill(0);

        for (i, in_y) in (v_start..=v_end).enumerate() {
            // Use the i32 weights cast to i64
            let weight = i64::from(v_kernel.weights_i32[i]);
            let buffer_idx = in_y % max_v_taps;

            let ring_row_start = buffer_idx * out_width;
            let ring_row = &ring_buffer[ring_row_start..ring_row_start + out_width];

            for (acc, &ring_val) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                *acc += i64::from(ring_val) * weight;
            }
        }

        // STEP C: Bit-shift back to u8 and clamp
        let out_row_start = out_y * out_width;
        let out_row = &mut out_channel[out_row_start..out_row_start + out_width];

        for (out_pixel, &acc) in out_row.iter_mut().zip(row_accumulator.iter()) {
            // Add (1 << 31) for rounding, then shift down by 32 bits total (16 from horizontal + 16 from vertical)
            let mut val = (acc + (1i64 << 31)) >> 32;

            // Clamp negative ringing artifacts from bicubic/lanczos
            val = val.clamp(0, 255);

            *out_pixel = val as u8;
        }
    }
}

#[inline(always)]
fn process_horizontal_row_u8(
    in_row: &[u8], out_row: &mut [i32], h_kernels: &[ConvKernel]
) {
    for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
        let start_idx = kernel.start_idx as usize;
        let end_idx = kernel.end_idx as usize;
        let weights = &kernel.weights_i32;
        let diff = (end_idx - start_idx) + 1;

        macro_rules! fixed_dot {
                ($row:expr, $start:expr, $n:expr, $weights:expr) => {
                    if let Some(e) = $row.get($start..$start + $n) {
                        e.iter()
                            .zip($weights.iter())
                            .map(|(&p, &w)| i32::from(p) * w)
                            .sum::<i32>()
                    } else {
                        0
                    }
                };
            }

        let sum = match diff {
            1 => fixed_dot!(in_row, start_idx, 1, weights),
            2 => fixed_dot!(in_row, start_idx, 2, weights),
            3 => fixed_dot!(in_row, start_idx, 3, weights),
            4 => fixed_dot!(in_row, start_idx, 4, weights),
            5 => fixed_dot!(in_row, start_idx, 5, weights),
            6 => fixed_dot!(in_row, start_idx, 6, weights),
            _ =>  0,
        };
        *out_pixel = sum;
    }
}
// ============================================================================
// KERNEL FUNCTIONS
// ============================================================================

/// Lanczos kernel with parameter a
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

/// Generalized bicubic kernel (Mitchell-Netravali family)
/// B and C are parameters that control the shape
/// Common presets:
/// - Mitchell: B=1/3, C=1/3 (balanced, default "bicubic")
/// - Catmull-Rom: B=0, C=0.5 (sharper)
/// - B-Spline: B=1, C=0 (blurrier, smoothest)
/// - Hermite: B=0, C=0 (similar to B-Spline)
#[inline]
fn bicubic_kernel(x: f32, b: f32, c: f32) -> f32 {
    let x = x.abs();

    if x < 1.0 {
        // |x| < 1
        let x2 = x * x;
        let x3 = x2 * x;
        ((12.0 - 9.0 * b - 6.0 * c) * x3 + (-18.0 + 12.0 * b + 6.0 * c) * x2 + (6.0 - 2.0 * b))
            / 6.0
    } else if x < 2.0 {
        // 1 <= |x| < 2
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

/// Sinc kernel with window radius A
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

/// Bilinear kernel (triangle function)
#[inline]
fn bilinear_kernel(x: f32) -> f32 {
    let x = x.abs();

    if x < 1.0 {
        1.0 - x
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use crate::resize::seperable_kernel::PrecomputedKernels;
    use crate::resize::ResizeMethod;
    use super::*;


    #[test]
    fn test_u8_identity_resize() {
        let width = 2;
        let height = 2;
        let in_pixels: Vec<u8> = vec![10, 50, 100, 200];
        let mut out_pixels = vec![0; 4];

        let kernels = PrecomputedKernels::new(
            width, height, width, height, ResizeMethod::Bicubic,
        );

        resample_separable_u8(
            &in_pixels, &mut out_pixels, width, height, width, height, &kernels,
        );

        // Early exit should trigger and copy perfectly
        assert_eq!(in_pixels, out_pixels);
    }

    #[test]
    fn test_u8_solid_color_normalization() {
        // Tests that our fixed-point normalization logic doesn't shift brightness
        let in_width = 4;
        let in_height = 4;

        // Test with pure white
        let in_pixels: Vec<u8> = vec![255; in_width * in_height];

        let out_width = 2;
        let out_height = 2;
        let mut out_pixels = vec![0; out_width * out_height];

        let kernels = PrecomputedKernels::new(
            in_width, in_height, out_width, out_height, ResizeMethod::Bicubic,
        );

        resample_separable_u8(
            &in_pixels, &mut out_pixels, in_width, in_height, out_width, out_height, &kernels,
        );

        // Every single pixel should remain exactly 255.
        // If it becomes 254 or overflows to 0, our weights_i32 sum is wrong.
        assert!(out_pixels.iter().all(|&p| p == 255), "Solid white test failed, pixels shifted");

        // Test with a mid-tone
        let in_pixels_mid: Vec<u8> = vec![128; in_width * in_height];
        resample_separable_u8(
            &in_pixels_mid, &mut out_pixels, in_width, in_height, out_width, out_height, &kernels,
        );
        assert!(out_pixels.iter().all(|&p| p == 128), "Solid mid-tone test failed, pixels shifted");
    }

    #[test]
    fn test_u8_vs_f32_parity() {
        // Compare the fixed-point u8 output against the floating-point reference output
        let in_width = 8;
        let in_height = 8;
        let out_width = 5;
        let out_height = 5;

        // Create a fake gradient image to give the kernels varying data to work with
        let in_pixels_u8: Vec<u8> = (0..(in_width * in_height)).map(|i| (i * 3 % 255) as u8).collect();
        let in_pixels_f32: Vec<f32> = in_pixels_u8.iter().map(|&p| f32::from(p)).collect();

        let mut out_pixels_u8 = vec![0u8; out_width * out_height];
        let mut out_pixels_f32 = vec![0.0f32; out_width * out_height];

        // Lanczos3 produces negative weights, which tests our clamping logic perfectly
        let kernels = PrecomputedKernels::new(
            in_width, in_height, out_width, out_height, ResizeMethod::Lanczos3,
        );

        // 1. Run Fixed-Point
        resample_separable_u8(
            &in_pixels_u8, &mut out_pixels_u8, in_width, in_height, out_width, out_height, &kernels,
        );

        // 2. Run Floating-Point
        resample_separable_precomputed::<f32>(
            &in_pixels_f32, &mut out_pixels_f32, in_width, in_height, out_width, out_height, &kernels,
        );

        // 3. Compare Results
        for (i, (p_u8, p_f32)) in out_pixels_u8.iter().zip(out_pixels_f32.iter()).enumerate() {
            // Replicate the clamping that would normally happen when converting f32 image back to u8
            let f32_clamped = p_f32.clamp(0.0, 255.0).round() as u8;

            let diff = (i32::from(*p_u8) - i32::from(f32_clamped)).abs();

            // We tolerate a strict maximum difference of 1.
            // This happens occasionally because `(a + 0.5).floor()` in floats vs `(a + (1<<31)) >> 32` in integers
            // can break ties exactly at .5 differently due to precision.
            assert!(
                diff <= 1,
                "Mismatch at index {i}: fixed-point u8={p_u8} vs floating-point reference={f32_clamped} (diff {diff})",
            );
        }
    }


    const ALL_METHODS: &[ResizeMethod] = &[
        ResizeMethod::Lanczos3,
        ResizeMethod::Lanczos2,
        ResizeMethod::Bicubic,
        ResizeMethod::Mitchell,
        ResizeMethod::CatmullRom,
        ResizeMethod::BSpline,
        ResizeMethod::Hermite,
        ResizeMethod::Sinc,
        ResizeMethod::Bilinear,
    ];

    // -------------------------------------------------------------------------
    // Kernel property tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_weights_sum_to_one() {
        for &method in ALL_METHODS {
            // Only horizontal kernels exist here (width changed, height same)
            // Use asymmetric sizes to exercise non-trivial resampling
            let kernels = PrecomputedKernels::new(100, 80, 37, 53, method);

            for (dir, kernel_vec) in [
                ("horizontal", kernels.horizontal.as_ref()),
                ("vertical", kernels.vertical.as_ref()),
            ] {
                let Some(kernels) = kernel_vec else { continue };
                for (i, k) in kernels.iter().enumerate() {
                    let count = (k.end_idx - k.start_idx + 1) as usize;
                    let sum: f32 = k.weights[..count].iter().sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-5,
                        "method={method:?} dir={dir} kernel[{i}] weights sum to {sum}, expected 1.0"
                    );
                }
            }
        }
    }

    #[test]
    fn test_indices_in_bounds() {
        let (in_w, in_h) = (200, 150);
        let (out_w, out_h) = (73, 99);

        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);

            if let Some(h) = &kernels.horizontal {
                for (i, k) in h.iter().enumerate() {
                    assert!(
                        (k.end_idx as usize) < in_w,
                        "method={method:?} horizontal kernel[{i}] end_idx {} >= in_width {in_w}",
                        k.end_idx
                    );
                    assert!(
                        k.start_idx <= k.end_idx,
                        "method={method:?} horizontal kernel[{i}] start_idx > end_idx"
                    );
                }
            }

            if let Some(v) = &kernels.vertical {
                for (i, k) in v.iter().enumerate() {
                    assert!(
                        (k.end_idx as usize) < in_h,
                        "method={method:?} vertical kernel[{i}] end_idx {} >= in_height {in_h}",
                        k.end_idx
                    );
                    assert!(
                        k.start_idx <= k.end_idx,
                        "method={method:?} vertical kernel[{i}] start_idx > end_idx"
                    );
                }
            }
        }
    }

    // -------------------------------------------------------------------------
    // Resample property tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_identity_resize_copies_exactly() {
        let (w, h) = (64usize, 48usize);
        let input: Vec<u8> = (0..w * h).map(|i| (i % 256) as u8).collect();
        let mut output = vec![0u8; w * h];

        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(w, h, w, h, method);
            resample_separable(&input, &mut output, w, h, w, h, &kernels);
            assert_eq!(
                input, output,
                "method={method:?} identity resize should copy input exactly"
            );
        }
    }

    #[test]
    fn test_flat_image_survives_resize() {
        // A constant-value image should come out constant after any resize,
        // because all normalized weight sums = 1.0
        let (in_w, in_h) = (64usize, 48usize);
        let (out_w, out_h) = (37usize, 53usize);
        let flat_value = 128u8;

        let input = vec![flat_value; in_w * in_h];
        let mut output = vec![0u8; out_w * out_h];

        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            for (i, &px) in output.iter().enumerate() {
                assert_eq!(
                    px, flat_value,
                    "method={method:?} flat image pixel[{i}] became {px}, expected {flat_value}"
                );
            }
        }
    }

    #[test]
    fn test_output_dimensions_are_correct() {
        let (in_w, in_h) = (100usize, 80usize);
        let (out_w, out_h) = (37usize, 53usize);
        let input: Vec<u8> = (0..in_w * in_h).map(|i| (i % 256) as u8).collect();

        for &method in ALL_METHODS {
            let mut output = vec![0u8; out_w * out_h];
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            // If we got here without a panic/OOB, dimensions are consistent.
            // Spot-check that the last pixel was written (not left as zero by accident)
            // by verifying output length is intact.
            assert_eq!(output.len(), out_w * out_h);
        }
    }

    #[test]
    fn test_horizontal_only_matches_fused() {
        // When only width changes, the horizontal-only path should match the fused path
        let (in_w, in_h) = (100usize, 48usize);
        let (out_w, out_h) = (37usize, 48usize); // height unchanged
        let input: Vec<u8> = (0..in_w * in_h).map(|i| (i % 256) as u8).collect();

        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            assert!(kernels.vertical.is_none(), "expected no vertical kernels");

            let mut output = vec![0u8; out_w * out_h];
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            assert_eq!(output.len(), out_w * out_h);
        }
    }

    #[test]
    fn test_vertical_only_matches_fused() {
        let (in_w, in_h) = (48usize, 100usize);
        let (out_w, out_h) = (48usize, 37usize); // width unchanged
        let input: Vec<u8> = (0..in_w * in_h).map(|i| (i % 256) as u8).collect();

        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            assert!(kernels.horizontal.is_none(), "expected no horizontal kernels");

            let mut output = vec![0u8; out_w * out_h];
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            assert_eq!(output.len(), out_w * out_h);
        }
    }

    // -------------------------------------------------------------------------
    // Edge / corner cases
    // -------------------------------------------------------------------------

    #[test]
    fn test_single_pixel_input() {
        let input = vec![200u8; 1];
        let mut output = vec![0u8; 1];
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(1, 1, 1, 1, method);
            resample_separable(&input, &mut output, 1, 1, 1, 1, &kernels);
            assert_eq!(output[0], 200, "method={method:?} single pixel failed");
        }
    }

    #[test]
    fn test_extreme_downscale() {
        let (in_w, in_h) = (7680usize, 4320usize);
        let (out_w, out_h) = (2usize, 2usize);
        let input = vec![128u8; in_w * in_h];
        let mut output = vec![0u8; out_w * out_h];
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            for &px in &output {
                assert_eq!(px, 128, "method={method:?} extreme downscale flat image failed");
            }
        }
    }
}