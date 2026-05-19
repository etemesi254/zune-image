use std::f32;

use zune_core::bit_depth::BitType;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;

/// A fast Box Blur filter operation.
///
/// A box blur is a spatial domain linear filter where each pixel in the resulting image
/// has a value equal to the average value of its neighboring pixels. It acts as a low-pass
/// filter, blurring the image by reducing sharp localized changes in pixel values.
///
/// # Algorithmic Details
///
/// This implementation is highly optimized:
/// 1. **Separable Passes:** A 2D box blur is mathematically separable. This applies a 1D
///    horizontal blur, transposes the image, applies a second 1D horizontal blur, and
///    transposes back. This reduces algorithmic complexity from $O(R^2)$ to $O(R)$.
/// 2. **Sliding Window:** During the 1D passes, it uses a sliding window (moving average)
///    accumulator. Instead of summing all pixels in the radius for every pixel, it simply
///    adds the incoming pixel and subtracts the outgoing pixel, dropping the per-pixel
///    complexity to $O(1)$ regardless of the radius size.
/// 3. **Fast Division:** Replaces expensive integer division in the inner loop with
///    multiplication by a constant (`fastdiv_u32`).
///
/// # Threads
///
/// If the `threads` feature is enabled, this operation automatically parallelizes processing
/// across the image's color channels.
///
/// # Example
///
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::errors::ImageErrors;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::box_blur::BoxBlur;
///
/// let blur = BoxBlur::new(5);
/// let mut image = Image::fill(10_u8, ColorSpace::RGB, 100, 100);
/// blur.execute(&mut image)?;
/// # Ok::<(), ImageErrors>(())
/// ```
#[derive(Default)]
pub struct BoxBlur {
    radius: usize,
}

impl BoxBlur {
    #[must_use]
    pub fn new(radius: usize) -> BoxBlur {
        BoxBlur { radius }
    }
}

impl OperationsTrait for BoxBlur {
    fn name(&self) -> &'static str {
        "Box blur"
    }
    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();

        let depth = image.depth();

        #[cfg(feature = "threads")]
        {
            trace!("Running box blur in multithreaded mode");
            std::thread::scope(|s| {
                let mut errors = vec![];
                // blur each channel on a separate thread
                for channel in image.channels_mut(false) {
                    let result = s.spawn(|| match depth.bit_type() {
                        BitType::U16 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u16>()?;
                            box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        BitType::U8 => {
                            let mut scratch_space = vec![0; width * height];
                            let data = channel.reinterpret_as_mut::<u8>()?;
                            box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }

                        BitType::F32 => {
                            let mut scratch_space = vec![0.0; width * height];
                            let data = channel.reinterpret_as_mut::<f32>()?;
                            box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                            Ok(())
                        }
                        d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d)),
                    });
                    errors.push(result);
                }
                errors
                    .into_iter()
                    .map(|x| x.join().unwrap())
                    .collect::<Result<Vec<()>, ImageErrors>>()
            })?;
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running box blur in single threaded mode");

            match depth.bit_type() {
                BitType::U16 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u16>()?;
                        box_blur_u16(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                BitType::U8 => {
                    let mut scratch_space = vec![0; width * height];

                    for channel in image.channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<u8>()?;
                        box_blur_u8(data, &mut scratch_space, width, height, self.radius);
                    }
                }

                BitType::F32 => {
                    let mut scratch_space = vec![0.0; width * height];

                    for channel in image.channels_mut(false) {
                        let data = channel.reinterpret_as_mut::<f32>()?;
                        box_blur_f32(data, &mut scratch_space, width, height, self.radius);
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented("box_blur", d)),
            }
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

pub fn box_blur_u16(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize,
    mut radius: usize,
) {
    if width == 0 || radius <= 1 {
        return;
    }
    if radius.is_multiple_of(2) {
        radius += 1;
    }

    // Horizontal pass: reads from in_out_image, writes to scratch_space
    box_blur_inner(in_out_image, scratch_space, width, radius);
    // Vertical pass: reads from scratch_space, writes DIRECTLY back to in_out_image
    box_blur_vertical_inner(scratch_space, in_out_image, width, height, radius);
}

pub fn box_blur_u8(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize,
    mut radius: usize,
) {
    if width == 0 || radius <= 1 {
        return;
    }
    if radius.is_multiple_of(2) {
        radius += 1;
    }

    box_blur_inner(in_out_image, scratch_space, width, radius);
    box_blur_vertical_inner(scratch_space, in_out_image, width, height, radius);
}

pub fn box_blur_f32(
    in_out_image: &mut [f32], scratch_space: &mut [f32], width: usize, height: usize,
    mut radius: usize,
) {
    if width == 0 || radius <= 1 {
        return;
    }
    if radius.is_multiple_of(2) {
        radius += 1;
    }

    box_blur_f32_inner(in_out_image, scratch_space, width, radius);
    box_blur_f32_vertical_inner(scratch_space, in_out_image, width, height, radius);
}

// --------------------------------------------------------------------------
// 1D Horizontal Passes
// --------------------------------------------------------------------------

pub(crate) fn box_blur_inner<T>(in_image: &[T], out_image: &mut [T], width: usize, radius: usize)
where
    T: Copy + NumOps<T>,
    u32: std::convert::From<T>,
{
    let diameter = (radius * 2) + 1;
    if width <= 1 || diameter <= 1 {
        return;
    }
    let m_radius = compute_mod_u32(diameter as u64);

    for (stride_in, stride_out) in in_image
        .chunks_exact(width)
        .zip(out_image.chunks_exact_mut(width))
    {
        let mut accumulator: u32 = 0;

        // Setup initial window for x = 0
        accumulator += (radius as u32) * u32::from(stride_in[0]);
        let safe_r = radius.min(width - 1);
        for x in 0..=safe_r {
            accumulator += u32::from(stride_in[x]);
        }
        if radius > safe_r {
            accumulator += ((radius - safe_r) as u32) * u32::from(*stride_in.last().unwrap());
        }

        stride_out[0] = T::from_u32(fastdiv_u32(accumulator, m_radius));

        // FAST PATH: If the image is wide enough, split into Left, Middle, and Right
        if width > diameter {
            // 1. LEFT edge (Ramp-up)
            // 'leaving' is clamped to 0. 'entering' is x + radius.
            let clamped_leaving = u32::from(stride_in[0]);
            for x in 1..=radius {
                let entering = u32::from(stride_in[x + radius]);
                accumulator = accumulator + entering - clamped_leaving;
                stride_out[x] = T::from_u32(fastdiv_u32(accumulator, m_radius));
            }

            // 2. STEADY STATE (Middle)
            // No bounds checking, no branches! We zip three slices of the EXACT same length.
            let leaving_slice = &stride_in[0..(width - diameter)];
            let entering_slice = &stride_in[diameter..width];
            let out_slice = &mut stride_out[(radius + 1)..(width - radius)];

            for ((&leaving, &entering), out) in leaving_slice
                .iter()
                .zip(entering_slice.iter())
                .zip(out_slice.iter_mut())
            {
                // Using wrapping ops just in case, though standard +/- is usually fine here
                accumulator = accumulator
                    .wrapping_add(u32::from(entering))
                    .wrapping_sub(u32::from(leaving));

                *out = T::from_u32(fastdiv_u32(accumulator, m_radius));
            }

            // 3. RIGHT edge
            // 'entering' is clamped to the last pixel. 'leaving' continues to increment.
            let clamped_entering = u32::from(stride_in[width - 1]);
            for x in (width - radius)..width {
                let leaving = u32::from(stride_in[x - radius - 1]);
                accumulator = accumulator + clamped_entering - leaving;
                stride_out[x] = T::from_u32(fastdiv_u32(accumulator, m_radius));
            }
        } else {
            // SLOW PATH: For very small images or huge radii, fallback to the safe branchy version.
            for x in 1..width {
                let leaving = if x <= radius { stride_in[0] } else { stride_in[x - radius - 1] };
                let right_idx = (x + radius).min(width - 1);
                let entering = stride_in[right_idx];

                accumulator = accumulator + u32::from(entering) - u32::from(leaving);
                stride_out[x] = T::from_u32(fastdiv_u32(accumulator, m_radius));
            }
        }
    }
}

pub(crate) fn box_blur_f32_inner(
    in_image: &[f32], out_image: &mut [f32], width: usize, radius: usize,
) {
    let diameter = (radius * 2) + 1;
    if width <= 1 || diameter <= 1 {
        return;
    }
    let recip = 1.0 / (diameter as f64);

    for (stride_in, stride_out) in in_image
        .chunks_exact(width)
        .zip(out_image.chunks_exact_mut(width))
    {
        let mut accumulator: f64 = 0.0;

        accumulator += (radius as f64) * f64::from(stride_in[0]);
        let safe_r = radius.min(width - 1);
        for x in 0..=safe_r {
            accumulator += f64::from(stride_in[x]);
        }
        if radius > safe_r {
            accumulator += ((radius - safe_r) as f64) * f64::from(*stride_in.last().unwrap());
        }

        stride_out[0] = (accumulator * recip) as f32;

        for x in 1..width {
            let leaving = if x <= radius {
                f64::from(stride_in[0])
            } else {
                f64::from(stride_in[x - radius - 1])
            };
            let right_idx = (x + radius).min(width - 1);
            let entering = f64::from(stride_in[right_idx]);

            accumulator += entering;
            accumulator -= leaving;

            stride_out[x] = (accumulator * recip) as f32;
        }
    }
}

// --------------------------------------------------------------------------
// 1D Vertical Passes (Column Striding)
// --------------------------------------------------------------------------

pub(crate) fn box_blur_vertical_inner<T>(
    in_image: &[T], out_image: &mut [T], width: usize, height: usize, radius: usize,
) where
    T: Copy + NumOps<T>,
    u32: std::convert::From<T>,
{
    let diameter = (radius * 2) + 1;
    if height <= 1 || diameter <= 1 {
        return;
    }
    let m_radius = compute_mod_u32(diameter as u64);

    for x in 0..width {
        let mut accumulator: u32 = 0;

        let top_val = u32::from(in_image[x]);
        accumulator += (radius as u32) * top_val;

        let safe_r = radius.min(height - 1);
        for y in 0..=safe_r {
            accumulator += u32::from(in_image[y * width + x]);
        }
        if radius > safe_r {
            let bottom_val = u32::from(in_image[(height - 1) * width + x]);
            accumulator += ((radius - safe_r) as u32) * bottom_val;
        }

        out_image[x] = T::from_u32(fastdiv_u32(accumulator, m_radius));

        for y in 1..height {
            let leaving = if y <= radius {
                top_val
            } else {
                u32::from(in_image[(y - radius - 1) * width + x])
            };

            let right_y = (y + radius).min(height - 1);
            let entering = u32::from(in_image[right_y * width + x]);

            accumulator += entering;
            accumulator -= leaving;

            out_image[y * width + x] = T::from_u32(fastdiv_u32(accumulator, m_radius));
        }
    }
}

pub(crate) fn box_blur_f32_vertical_inner(
    in_image: &[f32], out_image: &mut [f32], width: usize, height: usize, radius: usize,
) {
    let diameter = (radius * 2) + 1;
    if height <= 1 || diameter <= 1 {
        return;
    }
    let recip = 1.0 / (diameter as f64);

    for x in 0..width {
        let mut accumulator: f64 = 0.0;

        let top_val = f64::from(in_image[x]);
        accumulator += (radius as f64) * top_val;

        let safe_r = radius.min(height - 1);
        for y in 0..=safe_r {
            accumulator += f64::from(in_image[y * width + x]);
        }
        if radius > safe_r {
            let bottom_val = f64::from(in_image[(height - 1) * width + x]);
            accumulator += ((radius - safe_r) as f64) * bottom_val;
        }

        out_image[x] = (accumulator * recip) as f32;

        for y in 1..height {
            let leaving = if y <= radius {
                top_val
            } else {
                f64::from(in_image[(y - radius - 1) * width + x])
            };

            let right_y = (y + radius).min(height - 1);
            let entering = f64::from(in_image[right_y * width + x]);

            accumulator += entering;
            accumulator -= leaving;

            out_image[y * width + x] = (accumulator * recip) as f32;
        }
    }
}

pub(crate) fn box_blur_inner_4x<T>(
    in_rows: [&[T]; 4], out_rows: [&mut [T]; 4], width: usize, radius: usize,
) where
    T: Copy + NumOps<T>,
    u32: std::convert::From<T>,
{
    let diameter = (radius * 2) + 1;
    if width <= 1 || diameter <= 1 {
        return;
    }
    let m_radius = crate::mathops::compute_mod_u32(diameter as u64);

    let [in0, in1, in2, in3] = in_rows;
    let [out0, out1, out2, out3] = out_rows;

    let mut acc0: u32 = 0;
    let mut acc1: u32 = 0;
    let mut acc2: u32 = 0;
    let mut acc3: u32 = 0;

    // --- INITIALIZATION (x = 0) ---
    let safe_r = radius.min(width - 1);

    acc0 += (radius as u32) * u32::from(in0[0]);
    acc1 += (radius as u32) * u32::from(in1[0]);
    acc2 += (radius as u32) * u32::from(in2[0]);
    acc3 += (radius as u32) * u32::from(in3[0]);

    for x in 0..=safe_r {
        acc0 += u32::from(in0[x]);
        acc1 += u32::from(in1[x]);
        acc2 += u32::from(in2[x]);
        acc3 += u32::from(in3[x]);
    }

    if radius > safe_r {
        let diff = (radius - safe_r) as u32;
        acc0 += diff * u32::from(in0[width - 1]);
        acc1 += diff * u32::from(in1[width - 1]);
        acc2 += diff * u32::from(in2[width - 1]);
        acc3 += diff * u32::from(in3[width - 1]);
    }

    out0[0] = T::from_u32(fastdiv_u32(acc0, m_radius));
    out1[0] = T::from_u32(fastdiv_u32(acc1, m_radius));
    out2[0] = T::from_u32(fastdiv_u32(acc2, m_radius));
    out3[0] = T::from_u32(fastdiv_u32(acc3, m_radius));

    // --- FAST PATH: 3-Phase Branchless Loop ---
    if width > diameter {
        // 1. LEFT EDGE
        // 'leaving' is clamped to 0. 'entering' is x + radius.
        let l0 = u32::from(in0[0]);
        let l1 = u32::from(in1[0]);
        let l2 = u32::from(in2[0]);
        let l3 = u32::from(in3[0]);

        for x in 1..=radius {
            let right_idx = x + radius;

            acc0 = acc0 + u32::from(in0[right_idx]) - l0;
            acc1 = acc1 + u32::from(in1[right_idx]) - l1;
            acc2 = acc2 + u32::from(in2[right_idx]) - l2;
            acc3 = acc3 + u32::from(in3[right_idx]) - l3;

            out0[x] = T::from_u32(fastdiv_u32(acc0, m_radius));
            out1[x] = T::from_u32(fastdiv_u32(acc1, m_radius));
            out2[x] = T::from_u32(fastdiv_u32(acc2, m_radius));
            out3[x] = T::from_u32(fastdiv_u32(acc3, m_radius));
        }

        // 2. (Middle)
        // No bounds checking. We slice exactly what we need.
        let mid_len = width - diameter;

        let in0_l = &in0[0..mid_len];
        let in0_e = &in0[diameter..width];
        let out0_m = &mut out0[(radius + 1)..(width - radius)];

        let in1_l = &in1[0..mid_len];
        let in1_e = &in1[diameter..width];
        let out1_m = &mut out1[(radius + 1)..(width - radius)];

        let in2_l = &in2[0..mid_len];
        let in2_e = &in2[diameter..width];
        let out2_m = &mut out2[(radius + 1)..(width - radius)];

        let in3_l = &in3[0..mid_len];
        let in3_e = &in3[diameter..width];
        let out3_m = &mut out3[(radius + 1)..(width - radius)];


        for i in 0..mid_len {
            acc0 = acc0 + u32::from(in0_e[i]) - u32::from(in0_l[i]);
            acc1 = acc1 + u32::from(in1_e[i]) - u32::from(in1_l[i]);
            acc2 = acc2 + u32::from(in2_e[i]) - u32::from(in2_l[i]);
            acc3 = acc3 + u32::from(in3_e[i]) - u32::from(in3_l[i]);

            out0_m[i] = T::from_u32(fastdiv_u32(acc0, m_radius));
            out1_m[i] = T::from_u32(fastdiv_u32(acc1, m_radius));
            out2_m[i] = T::from_u32(fastdiv_u32(acc2, m_radius));
            out3_m[i] = T::from_u32(fastdiv_u32(acc3, m_radius));
        }

        // 3. RIGHT EDGE
        // 'entering' is clamped to the last pixel. 'leaving' continues to increment.
        let e0 = u32::from(in0[width - 1]);
        let e1 = u32::from(in1[width - 1]);
        let e2 = u32::from(in2[width - 1]);
        let e3 = u32::from(in3[width - 1]);

        for x in (width - radius)..width {
            let left_idx = x - radius - 1;

            acc0 = acc0 + e0 - u32::from(in0[left_idx]);
            acc1 = acc1 + e1 - u32::from(in1[left_idx]);
            acc2 = acc2 + e2 - u32::from(in2[left_idx]);
            acc3 = acc3 + e3 - u32::from(in3[left_idx]);

            out0[x] = T::from_u32(crate::mathops::fastdiv_u32(acc0, m_radius));
            out1[x] = T::from_u32(crate::mathops::fastdiv_u32(acc1, m_radius));
            out2[x] = T::from_u32(crate::mathops::fastdiv_u32(acc2, m_radius));
            out3[x] = T::from_u32(crate::mathops::fastdiv_u32(acc3, m_radius));
        }
    } else {
        // --- SLOW PATH (Fallback for tiny images or huge radii) ---
        for x in 1..width {
            let left_idx = if x <= radius { 0 } else { x - radius - 1 };
            let right_idx = (x + radius).min(width - 1);

            acc0 = acc0 + u32::from(in0[right_idx]) - u32::from(in0[left_idx]);
            acc1 = acc1 + u32::from(in1[right_idx]) - u32::from(in1[left_idx]);
            acc2 = acc2 + u32::from(in2[right_idx]) - u32::from(in2[left_idx]);
            acc3 = acc3 + u32::from(in3[right_idx]) - u32::from(in3[left_idx]);

            out0[x] = T::from_u32(crate::mathops::fastdiv_u32(acc0, m_radius));
            out1[x] = T::from_u32(crate::mathops::fastdiv_u32(acc1, m_radius));
            out2[x] = T::from_u32(crate::mathops::fastdiv_u32(acc2, m_radius));
            out3[x] = T::from_u32(crate::mathops::fastdiv_u32(acc3, m_radius));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_box_blur_alignment_bug_fixed() {
        // A simple 1D test to ensure pixels don't shift left/right
        let width = 5;
        let input: Vec<u8> = vec![10, 20, 30, 40, 50];
        let mut scratch = vec![0; 5];

        box_blur_inner(&input, &mut scratch, width, 1);

        // Expected for r=1, diameter=3:
        // x=0: (10+10+20)/3 = 13.33 -> 13
        // x=1: (10+20+30)/3 = 20.00 -> 20
        // x=2: (20+30+40)/3 = 30.00 -> 30
        // x=3: (30+40+50)/3 = 40.00 -> 40
        // x=4: (40+50+50)/3 = 46.66 -> 46
        assert_eq!(scratch, vec![13, 20, 30, 40, 46]);
    }

    #[test]
    fn test_box_blur_f32_precision() {
        let width = 4;
        let input: Vec<f32> = vec![100.0, 200.0, 300.0, 400.0];
        let mut scratch = vec![0.0; 4];

        box_blur_f32_inner(&input, &mut scratch, width, 1);

        // Allow minor floating point jitter
        assert!((scratch[0] - 133.333).abs() < 0.1);
        assert!((scratch[1] - 200.0).abs() < 0.1);
        assert!((scratch[2] - 300.0).abs() < 0.1);
        assert!((scratch[3] - 366.666).abs() < 0.1);
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;
    use super::*;

    #[bench]
    fn bench_box_blur_u16_full(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        // This tests both horizontal and vertical passes
        b.iter(|| {
            box_blur_u16(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }

    #[bench]
    fn bench_box_blur_u8_full(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        b.iter(|| {
            box_blur_u8(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }

    #[bench]
    fn bench_box_blur_f32_full(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![1.0; dimensions];
        let mut scratch_space = vec![0.0; dimensions];

        b.iter(|| {
            box_blur_f32(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }
}
