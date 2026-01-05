use crate::resize::ResizeMethod;
use crate::traits::NumOps;

pub fn resample_separable<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, method: ResizeMethod
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    match method {
        ResizeMethod::Lanczos3 => resample_impl::<T, 3>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            lanczos_kernel::<3>
        ),
        ResizeMethod::Lanczos2 => resample_impl::<T, 2>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            lanczos_kernel::<2>
        ),
        ResizeMethod::Bicubic | ResizeMethod::Mitchell => resample_impl::<T, 2>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            |x| bicubic_kernel(x, 1.0 / 3.0, 1.0 / 3.0)
        ),
        ResizeMethod::CatmullRom => resample_impl::<T, 2>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            |x| bicubic_kernel(x, 0.0, 0.5)
        ),
        ResizeMethod::BSpline => resample_impl::<T, 2>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            |x| bicubic_kernel(x, 1.0, 0.0)
        ),
        ResizeMethod::Hermite => resample_impl::<T, 2>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            |x| bicubic_kernel(x, 0.0, 0.0)
        ),
        ResizeMethod::Sinc => resample_impl::<T, 3>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            sinc_kernel::<3>
        ),
        ResizeMethod::Bilinear => resample_impl::<T, 1>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_width,
            out_height,
            bilinear_kernel
        )
    }
}

fn resample_impl<T, const A: i32>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, kernel_fn: fn(f32) -> f32
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
    let need_horizontal = in_width != out_width;
    let need_vertical = in_height != out_height;

    // Case 1: Only vertical resizing needed
    if !need_horizontal && need_vertical {
        resample_vertical_only::<T, A>(
            in_channel,
            out_channel,
            in_width,
            in_height,
            out_height,
            kernel_fn
        );
        return;
    }

    // Case 2: Only horizontal resizing needed
    if need_horizontal && !need_vertical {
        resample_horizontal_only::<T, A>(
            in_channel,
            out_channel,
            in_width,
            out_width,
            kernel_fn
        );
        return;
    }

    // Case 3: Both dimensions need resizing
    let mut temp_buffer: Vec<f32> = vec![0.0; in_height * out_width];

    // PASS 1: Horizontal convolution
    let x_ratio = in_width as f32 / out_width as f32;
    let h_kernels = precompute_kernels(in_width, out_width, x_ratio, A, kernel_fn);

    for (in_row, out_row) in in_channel
        .chunks_exact(in_width)
        .zip(temp_buffer.chunks_exact_mut(out_width))
    {
        for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
            let mut sum = 0.0;

            // Iterate over the kernel range
            for idx in kernel.start_idx..=kernel.end_idx {
                let pixel = f32::from(in_row[idx]);
                sum += pixel * kernel.weights[idx - kernel.start_idx];
            }

            *out_pixel = sum;
        }
    }

    //  Vertical pass
    let y_ratio = in_height as f32 / out_height as f32;
    let v_kernels = precompute_kernels(in_height, out_height, y_ratio, A, kernel_fn);

    for out_y in 0..out_height {
        let kernel = &v_kernels[out_y];
        let out_row_offset = out_y * out_width;

        for out_x in 0..out_width {
            let mut sum = 0.0;

            // Iterate directly over row indices
            for in_y in kernel.start_idx..=kernel.end_idx {
                let pixel = temp_buffer[in_y * out_width + out_x];
                sum += pixel * kernel.weights[in_y - kernel.start_idx];
            }

            out_channel[out_row_offset + out_x] = T::from_f32(sum);
        }
    }
}

fn resample_vertical_only<T, const A: i32>(
    in_channel: &[T], out_channel: &mut [T], width: usize, in_height: usize, out_height: usize,
    kernel_fn: fn(f32) -> f32
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    let y_ratio = in_height as f32 / out_height as f32;
    let v_kernels = precompute_kernels(in_height, out_height, y_ratio, A, kernel_fn);

    for out_y in 0..out_height {
        let kernel = &v_kernels[out_y];
        let out_row_offset = out_y * width;

        for x in 0..width {
            let mut sum = 0.0;

            for in_y in kernel.start_idx..=kernel.end_idx {
                let pixel = f32::from(in_channel[in_y * width + x]);
                sum += pixel * kernel.weights[in_y - kernel.start_idx];
            }

            out_channel[out_row_offset + x] = T::from_f32(sum);
        }
    }
}

fn resample_horizontal_only<T, const A: i32>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, out_width: usize,
    kernel_fn: fn(f32) -> f32
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    let x_ratio = in_width as f32 / out_width as f32;
    let h_kernels = precompute_kernels(in_width, out_width, x_ratio, A, kernel_fn);

    for (in_row, out_row) in in_channel
        .chunks_exact(in_width)
        .zip(out_channel.chunks_exact_mut(out_width))
    {
        for (out_pixel, kernel) in out_row.iter_mut().zip(h_kernels.iter()) {
            let mut sum = 0.0;

            for idx in kernel.start_idx..=kernel.end_idx {
                let pixel = f32::from(in_row[idx]);
                sum += pixel * kernel.weights[idx - kernel.start_idx];
            }

            *out_pixel = T::from_f32(sum);
        }
    }
}

// Maximum kernel size: 2*A where A can be up to 3, so max 6 taps
const MAX_KERNEL_SIZE: usize = 6;

struct ConvKernel {
    weights:   [f32; MAX_KERNEL_SIZE],
    start_idx: usize,
    end_idx:   usize
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
            start_idx,
            end_idx
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
