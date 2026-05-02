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

// ============================================================================
// Public generic resampler (f32 / u16 / etc.)
// ============================================================================

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

pub fn resample_separable_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels,
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
    // Early exit: no resize needed.
    if in_width == out_width && in_height == out_height {
        out_channel.copy_from_slice(in_channel);
        return;
    }

    let need_horizontal = kernels.horizontal.is_some();
    let need_vertical = kernels.vertical.is_some();

    // Vertical only.
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

    // Horizontal only.
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

    // Both dimensions — fused tiled implementation.
    let h_kernels = kernels.horizontal.as_ref().unwrap();
    let v_kernels = kernels.vertical.as_ref().unwrap();

    let max_v_taps = v_kernels
        .iter()
        .map(|k| (k.end_idx - k.start_idx + 1) as usize)
        .max()
        .unwrap_or(1);

    let mut ring_buffer = vec![0.0_f32; max_v_taps * out_width];
    let mut buffer_contents = vec![usize::MAX; max_v_taps];
    let mut row_accumulator = vec![0.0_f32; out_width];

    let special_div = crate::mathops::compute_mod_u32(max_v_taps as u64);

    for (v_kernel, out_row) in v_kernels
        .iter()
        .zip(out_channel.chunks_exact_mut(out_width))
    {
        let v_start = v_kernel.start_idx as usize;
        let v_end = v_kernel.end_idx as usize;

        for in_y in v_start..=v_end {
            let buffer_idx = crate::mathops::fastmod_u32(in_y as u32, special_div,max_v_taps as u32) as usize;

            if buffer_contents[buffer_idx] != in_y {

                let in_row = &in_channel[in_y * in_width..in_y * in_width + in_width];
                let ring_row = &mut ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

                process_horizontal_row_to_f32(in_row, ring_row, h_kernels);
                buffer_contents[buffer_idx] = in_y;
            }
        }

        // Vertical pass.

        // pass 1
        {
            let weight = v_kernel.weights[v_start];

            let buffer_idx = crate::mathops::fastmod_u32(v_start as u32, special_div,max_v_taps as u32) as usize;
            let ring_row = &ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

            for (acc, &rv) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                *acc = rv * weight;
            }
        }

        for (i, in_y) in (v_start+1..=v_end).enumerate() {
            let weight = v_kernel.weights[i];
            let buffer_idx = crate::mathops::fastmod_u32(in_y as u32, special_div,max_v_taps as u32) as usize;
            let ring_row = &ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

            for (acc, &rv) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                *acc += rv * weight;
            }
        }

        // Write output row.
        for (px, &acc) in out_row.iter_mut().zip(row_accumulator.iter()) {
            *px = T::from_f32(acc);
        }
    }
}

/// Process one input row through the horizontal kernels into an f32 buffer.
/// Works for any kernel width — no fixed-size match needed.
#[inline(always)]
fn process_horizontal_row_to_f32<T>(in_row: &[T], out_row: &mut [f32], h_kernels: &[ConvKernel])
where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>,
{
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
            let mut sum = 0.0_f32;
            for in_y in kernel.start_idx..=kernel.end_idx {
                let in_y = in_y as usize;
                let tap_idx = in_y - kernel.start_idx as usize;
                let pixel = f32::from(in_channel[in_y * width + x]);
                sum += pixel * kernel.weights[tap_idx];
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
}

// ============================================================================
// Optimised u8 fixed-point path
// ============================================================================

pub fn resample_separable_u8(
    in_channel: &[u8], out_channel: &mut [u8], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels,
) {
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

    let special_div = crate::mathops::compute_mod_u32(max_v_taps as u64);

    // Ring buffer stores Q16 horizontally-filtered rows.
    let mut ring_buffer = vec![0_i32; max_v_taps * out_width];
    let mut buffer_contents = vec![usize::MAX; max_v_taps];
    // i64 accumulator prevents overflow: Q16 × Q16 = Q32, fits in i64.
    let mut row_accumulator = vec![0_i64; out_width];

    for (v_kernel, out_row) in v_kernels
        .iter()
        .zip(out_channel.chunks_exact_mut(out_width))
    {
        let v_start = v_kernel.start_idx as usize;
        let v_end = v_kernel.end_idx as usize;

        for in_y in v_start..=v_end {
            let buffer_idx = crate::mathops::fastmod_u32(in_y as u32, special_div,max_v_taps as u32) as usize;

            if buffer_contents[buffer_idx] != in_y {
                let in_row = &in_channel[in_y * in_width..in_y * in_width + in_width];
                let ring_row =
                    &mut ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];
                process_horizontal_row_u8(in_row, ring_row, h_kernels);
                buffer_contents[buffer_idx] = in_y;
            }
        }

        // Vertical pass.
        row_accumulator.fill(0);

        for (i, in_y) in (v_start..=v_end).enumerate() {
            let weight = i64::from(v_kernel.weights_i32[i]);
            let buffer_idx = crate::mathops::fastmod_u32(in_y as u32, special_div,max_v_taps as u32) as usize;
            let ring_row = &ring_buffer[buffer_idx * out_width..buffer_idx * out_width + out_width];

            for (acc, &rv) in row_accumulator.iter_mut().zip(ring_row.iter()) {
                *acc += i64::from(rv) * weight;
            }
        }

        // Shift Q32 → u8 with rounding and clamp ringing artifacts.
        for (px, &acc) in out_row.iter_mut().zip(row_accumulator.iter()) {
            let val = ((acc + (1_i64 << 31)) >> 32).clamp(0, 255);
            *px = val as u8;
        }
    }
}

/// Horizontal pass into Q16 i32 buffer. Works for any kernel width.
#[inline(always)]
fn process_horizontal_row_u8(in_row: &[u8], out_row: &mut [i32], h_kernels: &[ConvKernel]) {
    for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
        let start = kernel.start_idx as usize;
        let count = (kernel.end_idx - kernel.start_idx + 1) as usize;

        let sum = if let Some(slice) = in_row.get(start..start + count) {
            slice
                .iter()
                .zip(kernel.weights_i32.iter())
                .map(|(&p, &w)| i32::from(p) * w)
                .sum::<i32>()
        } else {
            0
        };

        *out_pixel = sum;
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resize::seperable_kernel::PrecomputedKernels;
    use crate::resize::ResizeMethod;

    #[test]
    fn test_u8_identity_resize() {
        let width = 2;
        let height = 2;
        let in_pixels: Vec<u8> = vec![10, 50, 100, 200];
        let mut out_pixels = vec![0; 4];

        let kernels = PrecomputedKernels::new(width, height, width, height, ResizeMethod::Bicubic);
        resample_separable_u8(
            &in_pixels,
            &mut out_pixels,
            width,
            height,
            width,
            height,
            &kernels,
        );
        assert_eq!(in_pixels, out_pixels);
    }

    #[test]
    fn test_u8_solid_color_normalization() {
        let in_width = 4;
        let in_height = 4;
        let out_width = 2;
        let out_height = 2;
        let kernels = PrecomputedKernels::new(
            in_width,
            in_height,
            out_width,
            out_height,
            ResizeMethod::Bicubic,
        );

        let in_pixels: Vec<u8> = vec![255; in_width * in_height];
        let mut out_pixels = vec![0u8; out_width * out_height];
        resample_separable_u8(
            &in_pixels,
            &mut out_pixels,
            in_width,
            in_height,
            out_width,
            out_height,
            &kernels,
        );
        assert!(out_pixels.iter().all(|&p| p == 255), "Solid white shifted");

        let in_pixels_mid: Vec<u8> = vec![128; in_width * in_height];
        resample_separable_u8(
            &in_pixels_mid,
            &mut out_pixels,
            in_width,
            in_height,
            out_width,
            out_height,
            &kernels,
        );
        assert!(
            out_pixels.iter().all(|&p| p == 128),
            "Solid mid-tone shifted"
        );
    }

    #[test]
    fn test_u8_vs_f32_parity() {
        let in_width = 8;
        let in_height = 8;
        let out_width = 5;
        let out_height = 5;

        let in_pixels_u8: Vec<u8> = (0..in_width * in_height)
            .map(|i| (i * 3 % 255) as u8)
            .collect();
        let in_pixels_f32: Vec<f32> = in_pixels_u8.iter().map(|&p| f32::from(p)).collect();

        let mut out_u8 = vec![0u8; out_width * out_height];
        let mut out_f32 = vec![0.0f32; out_width * out_height];

        let kernels = PrecomputedKernels::new(
            in_width,
            in_height,
            out_width,
            out_height,
            ResizeMethod::Lanczos3,
        );

        resample_separable_u8(
            &in_pixels_u8,
            &mut out_u8,
            in_width,
            in_height,
            out_width,
            out_height,
            &kernels,
        );
        resample_separable_precomputed::<f32>(
            &in_pixels_f32,
            &mut out_f32,
            in_width,
            in_height,
            out_width,
            out_height,
            &kernels,
        );

        for (i, (p_u8, p_f32)) in out_u8.iter().zip(out_f32.iter()).enumerate() {
            let f32_clamped = p_f32.clamp(0.0, 255.0).round() as u8;
            let diff = (i32::from(*p_u8) - i32::from(f32_clamped)).abs();
            assert!(
                diff <= 1,
                "Mismatch at index {i}: u8={p_u8} vs f32={f32_clamped} (diff {diff})"
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

    #[test]
    fn test_weights_sum_to_one() {
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(100, 80, 37, 53, method);
            for (dir, kernel_vec) in [
                ("horizontal", kernels.horizontal.as_ref()),
                ("vertical", kernels.vertical.as_ref()),
            ] {
                let Some(kv) = kernel_vec else { continue };
                for (i, k) in kv.iter().enumerate() {
                    let sum: f32 = k.weights.iter().sum();
                    assert!(
                        (sum - 1.0).abs() < 1e-5,
                        "method={method:?} dir={dir} kernel[{i}] sum={sum}"
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
                        "method={method:?} h kernel[{i}] OOB"
                    );
                    assert!(
                        k.start_idx <= k.end_idx,
                        "method={method:?} h kernel[{i}] inverted"
                    );
                }
            }
            if let Some(v) = &kernels.vertical {
                for (i, k) in v.iter().enumerate() {
                    assert!(
                        (k.end_idx as usize) < in_h,
                        "method={method:?} v kernel[{i}] OOB"
                    );
                    assert!(
                        k.start_idx <= k.end_idx,
                        "method={method:?} v kernel[{i}] inverted"
                    );
                }
            }
        }
    }

    #[test]
    fn test_identity_resize_copies_exactly() {
        let (w, h) = (64usize, 48usize);
        let input: Vec<u8> = (0..w * h).map(|i| (i % 256) as u8).collect();
        let mut output = vec![0u8; w * h];
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(w, h, w, h, method);
            resample_separable(&input, &mut output, w, h, w, h, &kernels);
            assert_eq!(input, output, "method={method:?} identity failed");
        }
    }

    #[test]
    fn test_flat_image_survives_resize() {
        let (in_w, in_h) = (64usize, 48usize);
        let (out_w, out_h) = (37usize, 53usize);
        let flat_value = 128u8;
        let input = vec![flat_value; in_w * in_h];
        let mut output = vec![0u8; out_w * out_h];
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            for (i, &px) in output.iter().enumerate() {
                assert_eq!(px, flat_value, "method={method:?} flat pixel[{i}]={px}");
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
            assert_eq!(output.len(), out_w * out_h);
        }
    }

    #[test]
    fn test_horizontal_only_matches_fused() {
        let (in_w, in_h) = (100usize, 48usize);
        let (out_w, out_h) = (37usize, 48usize);
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
        let (out_w, out_h) = (48usize, 37usize);
        let input: Vec<u8> = (0..in_w * in_h).map(|i| (i % 256) as u8).collect();
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            assert!(
                kernels.horizontal.is_none(),
                "expected no horizontal kernels"
            );
            let mut output = vec![0u8; out_w * out_h];
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            assert_eq!(output.len(), out_w * out_h);
        }
    }

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
                assert_eq!(px, 128, "method={method:?} extreme downscale flat failed");
            }
        }
    }

    #[test]
    fn test_10x_downscale_flat_image() {
        // Specifically tests the 100→10% case from the bug report.
        let (in_w, in_h) = (1000usize, 1000usize);
        let (out_w, out_h) = (100usize, 100usize);
        let input = vec![200u8; in_w * in_h];
        let mut output = vec![0u8; out_w * out_h];
        for &method in ALL_METHODS {
            let kernels = PrecomputedKernels::new(in_w, in_h, out_w, out_h, method);
            resample_separable(&input, &mut output, in_w, in_h, out_w, out_h, &kernels);
            for (i, &px) in output.iter().enumerate() {
                assert_eq!(px, 200, "method={method:?} 10x downscale pixel[{i}]={px}");
            }
        }
    }
}
