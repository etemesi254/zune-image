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
        ResizeMethod::Bilinear => (bilinear_kernel, 1)
    }
}

pub(crate) struct PrecomputedKernels {
    pub horizontal: Option<Vec<ConvKernel>>,
    pub vertical:   Option<Vec<ConvKernel>>
}

impl PrecomputedKernels {
    pub fn new(
        in_width: usize, in_height: usize, out_width: usize, out_height: usize,
        method: ResizeMethod
    ) -> Self {
        let (kernel_fn, radius) = get_kernel_fn_and_radius(method);

        let horizontal = if in_width != out_width {
            let x_ratio = in_width as f32 / out_width as f32;
            Some(precompute_kernels(
                in_width, out_width, x_ratio, radius, kernel_fn
            ))
        } else {
            None
        };

        let vertical = if in_height != out_height {
            let y_ratio = in_height as f32 / out_height as f32;
            Some(precompute_kernels(
                in_height, out_height, y_ratio, radius, kernel_fn
            ))
        } else {
            None
        };

        PrecomputedKernels {
            horizontal,
            vertical
        }
    }
}

pub fn resample_separable<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    resample_separable_precomputed(
        in_channel,
        out_channel,
        in_width,
        in_height,
        out_width,
        out_height,
        kernels
    )
}

pub fn resample_separable_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernels: &PrecomputedKernels
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
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
            kernels.vertical.as_ref().unwrap()
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
            kernels.horizontal.as_ref().unwrap()
        );
        return;
    }

    // Case 3: Both dimensions need resizing
    let mut temp_buffer: Vec<f32> = vec![0.0; in_height * out_width];

    // PASS 1: Horizontal convolution
    let h_kernels = kernels.horizontal.as_ref().unwrap();

    for (in_row, out_row) in in_channel
        .chunks_exact(in_width)
        .zip(temp_buffer.chunks_exact_mut(out_width))
    {
        for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
            let start_idx = kernel.start_idx as usize;
            let end_idx = kernel.end_idx as usize;
            let weights = &kernel.weights;
            let diff = (end_idx - start_idx) + 1;

            // Use iterator with enumerate for better optimization,
            // also put most common indices for this to better optimize it
            let sum = match diff {
                6 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 6) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                5 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 5) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                4 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 4) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                3 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 3) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                2 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 2) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                1 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 1) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                _ => in_row[start_idx..=end_idx]
                    .iter()
                    .zip(weights.iter())
                    .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                    .sum::<f32>()
            };

            *out_pixel = sum;
        }
    }

    // PASS 2: Vertical pass
    let v_kernels = kernels.vertical.as_ref().unwrap();

    for out_x in 0..out_width {
        for (out_y, kernel) in (0..out_height).zip(v_kernels.iter()) {
            let start_idx = kernel.start_idx as usize;
            let end_idx = kernel.end_idx as usize;
            let weights = &kernel.weights;

            let sum: f32 = (start_idx..=end_idx)
                .zip(weights.iter())
                .map(|(in_y, &weight)| temp_buffer[in_y * out_width + out_x] * weight)
                .sum();

            out_channel[out_y * out_width + out_x] = T::from_f32(sum);
        }
    }
}

fn resample_vertical_only_precomputed<T>(
    in_channel: &[T], out_channel: &mut [T], width: usize, _in_height: usize, out_height: usize,
    v_kernels: &[ConvKernel]
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
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
    h_kernels: &[ConvKernel]
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
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
            let sum = match diff {
                6 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 6) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                5 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 5) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                4 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 4) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                3 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 3) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                2 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 2) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                1 => {
                    if let Some(e) = in_row.get(start_idx..start_idx + 1) {
                        e.iter()
                            .zip(weights.iter())
                            .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                            .sum::<f32>()
                    } else {
                        debug_assert!(true, "Panic on slice");
                        0.0
                    }
                }
                _ => in_row[start_idx..=end_idx]
                    .iter()
                    .zip(weights.iter())
                    .map(|(&pixel, &weight)| f32::from(pixel) * weight)
                    .sum::<f32>()
            };


            *out_pixel = T::from_f32(sum);
        }
    }
}

// Maximum kernel size: 2*A where A can be up to 3, so max 6 taps
const MAX_KERNEL_SIZE: usize = 6;

#[derive(Clone, Copy)]
pub(crate) struct ConvKernel {
    weights:   [f32; MAX_KERNEL_SIZE],
    start_idx: u32,
    end_idx:   u32
}

fn precompute_kernels(
    in_size: usize, out_size: usize, ratio: f32, a: i32, kernel_fn: fn(f32) -> f32
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

        kernels.push(ConvKernel {
            weights,
            start_idx: start_idx as u32,
            end_idx: end_idx as u32
        });
    }

    kernels
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
    use crate::resize::seperable_kernel::{precompute_kernels, PrecomputedKernels};
    use crate::resize::ResizeMethod;

    #[test]
    fn test_seperable_kernel() {
        let w = 7680;
        let h = 4320;
        let kernel = PrecomputedKernels::new(w, h, w / 2, h / 2, ResizeMethod::Bicubic);
        let values = kernel.vertical.unwrap().iter().last().unwrap().clone();
        let subtracted = (values.end_idx - values.start_idx) + 1;
        println!("{},{} ,{}", values.start_idx, values.end_idx, subtracted);
    }
}
