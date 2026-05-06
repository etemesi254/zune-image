use std::time::Instant;

use crate::premul_alpha::PremultiplyAlpha;
use crate::resize::seperable_kernel::{
    resample_separable, resample_separable_u8, PrecomputedKernels,
};
use crate::resize::ResizeMethod;
use crate::transfer_curve::{ConversionType, TransferCurve, TransferFunction};
use crate::utils::execute_on;
use zune_core::bit_depth::{BitDepth, BitType};
use zune_core::colorspace::ColorCharacteristics;
use zune_core::log::{trace, warn};
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::{OperationColorValues, OperationsTrait};

// -----------------------------------------------------------------------------
// Exact Integer Math Helpers for Scanline Intersection
// -----------------------------------------------------------------------------
#[inline(always)]
fn div_floor(a: i64, b: i64) -> i64 {
    let res = a / b;
    let rem = a % b;
    if rem != 0 && ((a < 0) != (b < 0)) {
        res - 1
    } else {
        res
    }
}

#[inline(always)]
fn div_ceil(a: i64, b: i64) -> i64 {
    let res = a / b;
    let rem = a % b;
    if rem != 0 && ((a < 0) == (b < 0)) {
        res + 1
    } else {
        res
    }
}

#[inline(always)]
fn update_exact_bounds(v: i64, base: i64, max_val: i64, t_min: &mut i64, t_max: &mut i64) {
    if v == 0 {
        if base < 0 || base > max_val {
            *t_max = -1; // invalid
        }
    } else if v > 0 {
        let t1 = div_ceil(-base, v);
        let t2 = div_floor(max_val - base, v);
        *t_min = (*t_min).max(t1);
        *t_max = (*t_max).min(t2);
    } else {
        let t1 = div_floor(-base, v);
        let t2 = div_ceil(max_val - base, v);
        *t_min = (*t_min).max(t2);
        *t_max = (*t_max).min(t1);
    }
}

/// Affine transformation matrix in the form:
/// | a  b  tx |
/// | c  d  ty |
/// | 0  0  1  |
#[derive(Debug, Clone, Copy)]
pub struct AffineTransform {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl AffineTransform {
    #[must_use]
    pub fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self { a, b, c, d, tx, ty }
    }
    #[must_use]
    pub fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }

    #[must_use]
    pub fn rotation(angle: f32) -> Self {
        let rad = angle.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self::new(cos, -sin, sin, cos, 0.0, 0.0)
    }

    #[must_use]
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, tx, ty)
    }

    #[must_use]
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self::new(sx, 0.0, 0.0, sy, 0.0, 0.0)
    }

    #[must_use]
    pub fn shear(shx: f32, shy: f32) -> Self {
        Self::new(1.0, shx, shy, 1.0, 0.0, 0.0)
    }

    #[must_use]
    pub fn then(&self, other: &AffineTransform) -> Self {
        Self {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            tx: self.a * other.tx + self.b * other.ty + self.tx,
            ty: self.c * other.tx + self.d * other.ty + self.ty,
        }
    }

    #[must_use]
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty,
        )
    }

    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < f32::EPSILON {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            tx: (self.b * self.ty - self.d * self.tx) * inv_det,
            ty: (self.c * self.tx - self.a * self.ty) * inv_det,
        })
    }

    #[must_use]
    pub fn is_axis_aligned(&self) -> bool {
        self.b.abs() < f32::EPSILON && self.c.abs() < f32::EPSILON
    }
}

impl OperationsTrait for AffineTransform {
    fn name(&self) -> &'static str {
        "Affine Transform"
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    #[allow(clippy::too_many_lines, unused_variables)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let is_image_linear = image.metadata().color_trc() == Some(ColorCharacteristics::Linear);
        let transfer_function = image
            .metadata()
            .color_trc()
            .unwrap_or(ColorCharacteristics::sRGB);

        if !is_image_linear {
            let start = Instant::now();
            trace!("Converting image to linear along affine transform");
            TransferCurve::new(
                TransferFunction::from(transfer_function),
                ConversionType::GammaToLinear,
            )
            .execute_impl(image)?;
            trace!(
                "Image conversion to linear completed in {:.2?}",
                start.elapsed()
            );
        }

        let is_premultiplied = image.metadata().is_premultiplied_alpha();
        let has_alpha = image.colorspace().has_alpha();
        let original_depth = image.depth();

        if !is_premultiplied && has_alpha {
            image.convert_depth(BitDepth::Float32)?;
            trace!("Premultiplying alpha along affine transform");
            let start = Instant::now();
            PremultiplyAlpha::new(AlphaState::PreMultiplied).execute_impl(image)?;
            trace!("Premultiply completed in {:.2?}", start.elapsed());
        }

        let (w, h) = image.dimensions();

        if self.inverse().is_none() {
            warn!(
                "a={},b={},c={},d={},tx={},ty={}",
                self.a, self.b, self.c, self.d, self.tx, self.ty
            );
            return Err(ImageErrors::GenericString(
                "AffineTransform doesn't have inverse".to_string(),
            ));
        }

        let (new_w, new_h) = get_affine_output_dimensions(w, h, self);
        let depth = image.depth().bit_type();

        if self.is_axis_aligned() {
            trace!("Affine transform is axis-aligned — using separable resampler");
            let kernels = PrecomputedKernels::new(w, h, new_w, new_h, ResizeMethod::Bilinear);

            let affine_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
                let mut new_channel = Channel::new_with_bit_type(new_w * new_h, depth);
                match depth {
                    BitType::U8 => resample_separable_u8(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        &kernels,
                    ),
                    BitType::U16 => resample_separable::<u16>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        &kernels,
                    ),
                    BitType::F32 => resample_separable::<f32>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        &kernels,
                    ),
                    d => return Err(ImageErrors::ImageOperationNotImplemented("affine", d)),
                }
                *channel = new_channel;
                Ok(())
            };
            execute_on(affine_fn, image, false)?;
        } else {
            trace!("Affine transform is not axis-aligned — using 2D reverse-mapped sampler");
            let affine_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
                let mut new_channel = Channel::new_with_bit_type(new_w * new_h, depth);
                match depth {
                    BitType::U8 => affine_transform_channel::<u8>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        self,
                    ),
                    BitType::U16 => affine_transform_channel::<u16>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        self,
                    ),
                    BitType::F32 => affine_transform_channel::<f32>(
                        channel.reinterpret_as()?,
                        new_channel.reinterpret_as_mut()?,
                        w,
                        h,
                        new_w,
                        new_h,
                        self,
                    ),
                    d => return Err(ImageErrors::ImageOperationNotImplemented("affine", d)),
                }
                *channel = new_channel;
                Ok(())
            };
            execute_on(affine_fn, image, false)?;
        }

        image.set_dimensions(new_w, new_h);

        if !is_premultiplied && has_alpha {
            trace!("Un-premultiplying alpha along affine transform");
            let start = Instant::now();
            PremultiplyAlpha::new(AlphaState::NonPreMultiplied).execute_impl(image)?;
            image.convert_depth(original_depth)?;
            trace!("Un-premultiply completed in {:.2?}", start.elapsed());
        }

        if !is_image_linear {
            let start = Instant::now();
            trace!("Converting image back to gamma along affine transform");
            TransferCurve::new(
                TransferFunction::from(transfer_function),
                ConversionType::LinearToGamma,
            )
            .execute_impl(image)?;
            trace!(
                "Image conversion to gamma completed in {:.2?}",
                start.elapsed()
            );
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U16, BitType::U8]
    }
}

pub trait BilinearProcess: Copy + Default + Send + Sync {
   #[allow(clippy::too_many_arguments)]
    fn process_rows(
        in_channel: &[Self], out_band: &mut [Self], band_start_y: usize, in_width: usize,
        in_height: usize, out_width: usize, inv: &AffineTransform, min_x: f32, min_y: f32,
    );
}

// -----------------------------------------------------------------------------
// u8 Specialization
// -----------------------------------------------------------------------------
impl BilinearProcess for u8 {
    #[allow(clippy::many_single_char_names)]
    fn process_rows(
        in_channel: &[Self], out_band: &mut [Self], band_start_y: usize, in_width: usize,
        in_height: usize, out_width: usize, inv: &AffineTransform, min_x: f32, min_y: f32,
    ) {
        if in_width < 2 || in_height < 2 {
            out_band.fill(0);
            return;
        }

        let a_fx = (inv.a * 65536.0) as i64;
        let c_fx = (inv.c * 65536.0) as i64;
        let w = in_width;

        let max_x_fx = ((in_width as i64 - 1) << 16) - 1;
        let max_y_fx = ((in_height as i64 - 1) << 16) - 1;

        for (row_idx, out_row) in out_band.chunks_exact_mut(out_width).enumerate() {
            let y_f32 = (band_start_y + row_idx) as f32 + min_y;
            let base_x_fx = ((inv.a * min_x + inv.b * y_f32 + inv.tx) * 65536.0) as i64;
            let base_y_fx = ((inv.c * min_x + inv.d * y_f32 + inv.ty) * 65536.0) as i64;

            let mut t_min = 0i64;
            let mut t_max = out_width as i64 - 1;

            update_exact_bounds(a_fx, base_x_fx, max_x_fx, &mut t_min, &mut t_max);
            update_exact_bounds(c_fx, base_y_fx, max_y_fx, &mut t_min, &mut t_max);

            if t_min > t_max {
                out_row.fill(0);
                continue;
            }

            let start = t_min.max(0) as usize;
            let end = (t_max + 1).min(out_width as i64) as usize;

            if start > 0 {
                out_row[..start].fill(0);
            }
            if end < out_width {
                out_row[end..].fill(0);
            }

            let mut curr_x_fx = base_x_fx + start as i64 * a_fx;
            let mut curr_y_fx = base_y_fx + start as i64 * c_fx;

            // Pure branchless inner loop!
            for px in &mut out_row[start..end] {
                let ux = (curr_x_fx >> 16) as usize;
                let uy = (curr_y_fx >> 16) as usize;

                let row0 = &in_channel[uy * w..uy * w + w];
                let row1 = &in_channel[(uy + 1) * w..(uy + 1) * w + w];

                let p00 = u32::from(row0[ux]);
                let p10 = u32::from(row0[ux + 1]);
                let p01 = u32::from(row1[ux]);
                let p11 = u32::from(row1[ux + 1]);

                let fx = ((curr_x_fx >> 8) & 0xFF) as u32;
                let fy = ((curr_y_fx >> 8) & 0xFF) as u32;

                let inv_fx = 256 - fx;
                let inv_fy = 256 - fy;

                let w00 = inv_fx * inv_fy;
                let w10 = fx * inv_fy;
                let w01 = inv_fx * fy;
                let w11 = fx * fy;

                let sum = p00 * w00 + p10 * w10 + p01 * w01 + p11 * w11;
                *px = ((sum + 32_768) >> 16) as u8;

                curr_x_fx += a_fx;
                curr_y_fx += c_fx;
            }
        }
    }
}

// -----------------------------------------------------------------------------
// u16 Specialization
// -----------------------------------------------------------------------------
impl BilinearProcess for u16 {
    #[allow(clippy::many_single_char_names)]
    fn process_rows(
        in_channel: &[Self], out_band: &mut [Self], band_start_y: usize, in_width: usize,
        in_height: usize, out_width: usize, inv: &AffineTransform, min_x: f32, min_y: f32,
    ) {
        if in_width < 2 || in_height < 2 {
            out_band.fill(0);
            return;
        }

        let a_fx = (inv.a * 65536.0) as i64;
        let c_fx = (inv.c * 65536.0) as i64;
        let w = in_width;

        let max_x_fx = ((in_width as i64 - 1) << 16) - 1;
        let max_y_fx = ((in_height as i64 - 1) << 16) - 1;

        for (row_idx, out_row) in out_band.chunks_exact_mut(out_width).enumerate() {
            let y_f32 = (band_start_y + row_idx) as f32 + min_y;
            let base_x_fx = ((inv.a * min_x + inv.b * y_f32 + inv.tx) * 65536.0) as i64;
            let base_y_fx = ((inv.c * min_x + inv.d * y_f32 + inv.ty) * 65536.0) as i64;

            let mut t_min = 0i64;
            let mut t_max = out_width as i64 - 1;

            update_exact_bounds(a_fx, base_x_fx, max_x_fx, &mut t_min, &mut t_max);
            update_exact_bounds(c_fx, base_y_fx, max_y_fx, &mut t_min, &mut t_max);

            if t_min > t_max {
                out_row.fill(0);
                continue;
            }

            let start = t_min.max(0) as usize;
            let end = (t_max + 1).min(out_width as i64) as usize;

            if start > 0 {
                out_row[..start].fill(0);
            }
            if end < out_width {
                out_row[end..].fill(0);
            }

            let mut curr_x_fx = base_x_fx + start as i64 * a_fx;
            let mut curr_y_fx = base_y_fx + start as i64 * c_fx;

            for px in &mut out_row[start..end] {
                let ux = (curr_x_fx >> 16) as usize;
                let uy = (curr_y_fx >> 16) as usize;

                let row0 = &in_channel[uy * w..uy * w + w];
                let row1 = &in_channel[(uy + 1) * w..(uy + 1) * w + w];

                let p00 = u64::from(row0[ux]);
                let p10 = u64::from(row0[ux + 1]);
                let p01 = u64::from(row1[ux]);
                let p11 = u64::from(row1[ux + 1]);

                let fx = ((curr_x_fx >> 8) & 0xFF) as u64;
                let fy = ((curr_y_fx >> 8) & 0xFF) as u64;

                let inv_fx = 256 - fx;
                let inv_fy = 256 - fy;

                let w00 = inv_fx * inv_fy;
                let w10 = fx * inv_fy;
                let w01 = inv_fx * fy;
                let w11 = fx * fy;

                let sum = p00 * w00 + p10 * w10 + p01 * w01 + p11 * w11;
                *px = ((sum + 32_768) >> 16) as u16;

                curr_x_fx += a_fx;
                curr_y_fx += c_fx;
            }
        }
    }
}

// -----------------------------------------------------------------------------
// f32 Specialization
// -----------------------------------------------------------------------------
impl BilinearProcess for f32 {
    #[allow(clippy::many_single_char_names)]
    fn process_rows(
        in_channel: &[Self], out_band: &mut [Self], band_start_y: usize, in_width: usize,
        in_height: usize, out_width: usize, inv: &AffineTransform, min_x: f32, min_y: f32,
    ) {
        if in_width < 2 || in_height < 2 {
            out_band.fill(0.0);
            return;
        }

        let a_fx = (inv.a * 65536.0) as i64;
        let c_fx = (inv.c * 65536.0) as i64;
        let w = in_width;

        let max_x_fx = ((in_width as i64 - 1) << 16) - 1;
        let max_y_fx = ((in_height as i64 - 1) << 16) - 1;

        for (row_idx, out_row) in out_band.chunks_exact_mut(out_width).enumerate() {
            let y_f32 = (band_start_y + row_idx) as f32 + min_y;
            let base_x_fx = ((inv.a * min_x + inv.b * y_f32 + inv.tx) * 65536.0) as i64;
            let base_y_fx = ((inv.c * min_x + inv.d * y_f32 + inv.ty) * 65536.0) as i64;

            let mut t_min = 0i64;
            let mut t_max = out_width as i64 - 1;

            update_exact_bounds(a_fx, base_x_fx, max_x_fx, &mut t_min, &mut t_max);
            update_exact_bounds(c_fx, base_y_fx, max_y_fx, &mut t_min, &mut t_max);

            if t_min > t_max {
                out_row.fill(0.0);
                continue;
            }

            let start = t_min.max(0) as usize;
            let end = (t_max + 1).min(out_width as i64) as usize;

            if start > 0 {
                out_row[..start].fill(0.0);
            }
            if end < out_width {
                out_row[end..].fill(0.0);
            }

            let mut curr_x_fx = base_x_fx + start as i64 * a_fx;
            let mut curr_y_fx = base_y_fx + start as i64 * c_fx;

            for px in &mut out_row[start..end] {
                let ux = (curr_x_fx >> 16) as usize;
                let uy = (curr_y_fx >> 16) as usize;

                let row0 = &in_channel[uy * w..uy * w + w];
                let row1 = &in_channel[(uy + 1) * w..(uy + 1) * w + w];

                let p00 = row0[ux];
                let p10 = row0[ux + 1];
                let p01 = row1[ux];
                let p11 = row1[ux + 1];

                let fx = (curr_x_fx & 0xFFFF) as f32 * (1.0 / 65536.0);
                let fy = (curr_y_fx & 0xFFFF) as f32 * (1.0 / 65536.0);

                let inv_fx = 1.0 - fx;
                let inv_fy = 1.0 - fy;

                *px = p00 * inv_fx * inv_fy + p10 * fx * inv_fy + p01 * inv_fx * fy + p11 * fx * fy;

                curr_x_fx += a_fx;
                curr_y_fx += c_fx;
            }
        }
    }
}

#[must_use]
pub fn get_affine_output_dimensions(
    width: usize, height: usize, transform: &AffineTransform,
) -> (usize, usize) {
    let corners = [
        (0.0_f32, 0.0_f32),
        (width as f32, 0.0),
        (0.0, height as f32),
        (width as f32, height as f32),
    ];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (x, y) in corners {
        let (tx, ty) = transform.transform_point(x, y);
        min_x = min_x.min(tx);
        max_x = max_x.max(tx);
        min_y = min_y.min(ty);
        max_y = max_y.max(ty);
    }

    (
        (max_x - min_x).ceil() as usize,
        (max_y - min_y).ceil() as usize,
    )
}

#[allow(clippy::many_single_char_names)]
pub fn affine_transform_channel<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, transform: &AffineTransform,
) where
    T: BilinearProcess,
{
    let Some(inv) = transform.inverse() else {
        return;
    };
    out_channel.fill(T::default());

    let corners = [
        (0.0_f32, 0.0_f32),
        (in_width as f32, 0.0),
        (0.0, in_height as f32),
        (in_width as f32, in_height as f32),
    ];
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    for (x, y) in corners {
        let (cx, cy) = transform.transform_point(x, y);
        min_x = min_x.min(cx);
        min_y = min_y.min(cy);
    }

    #[cfg(feature = "threads")]
    {
        let num_threads = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .div_ceil(2)
            .max(1);

        if num_threads > 1 {
            let rows_per_thread = out_height.div_ceil(num_threads);

            std::thread::scope(|s| {
                for (chunk_idx, out_band) in out_channel
                    .chunks_mut(rows_per_thread * out_width)
                    .enumerate()
                {
                    let band_start_y = chunk_idx * rows_per_thread;
                    let inv_ref = &inv;

                    s.spawn(move || {
                        T::process_rows(
                            in_channel,
                            out_band,
                            band_start_y,
                            in_width,
                            in_height,
                            out_width,
                            inv_ref,
                            min_x,
                            min_y,
                        );
                    });
                }
            });
            return;
        }
    }

    T::process_rows(
        in_channel,
        out_channel,
        0,
        in_width,
        in_height,
        out_width,
        &inv,
        min_x,
        min_y,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // AffineTransform unit tests
    // -------------------------------------------------------------------------

    #[test]
    fn identity_transform_point() {
        let t = AffineTransform::identity();
        let (x, y) = t.transform_point(3.0, 7.0);
        assert!((x - 3.0).abs() < 1e-5);
        assert!((y - 7.0).abs() < 1e-5);
    }

    #[test]
    fn translation_moves_point() {
        let t = AffineTransform::translation(10.0, -5.0);
        let (x, y) = t.transform_point(1.0, 1.0);
        assert!((x - 11.0).abs() < 1e-5);
        assert!((y + 4.0).abs() < 1e-5);
    }

    #[test]
    fn scale_doubles_coordinates() {
        let t = AffineTransform::scale(2.0, 3.0);
        let (x, y) = t.transform_point(4.0, 5.0);
        assert!((x - 8.0).abs() < 1e-5);
        assert!((y - 15.0).abs() < 1e-5);
    }

    #[test]
    fn rotation_90_degrees() {
        // 90° CCW: (1,0) → (0,1)
        let t = AffineTransform::rotation(90.0);
        let (x, y) = t.transform_point(1.0, 0.0);
        assert!((x - 0.0).abs() < 1e-5, "x={x}");
        assert!((y - 1.0).abs() < 1e-5, "y={y}");
    }

    #[test]
    fn rotation_360_is_identity() {
        let t = AffineTransform::rotation(360.0);
        let (x, y) = t.transform_point(5.0, 3.0);
        assert!((x - 5.0).abs() < 1e-4);
        assert!((y - 3.0).abs() < 1e-4);
    }

    #[test]
    fn inverse_of_translation() {
        let t = AffineTransform::translation(7.0, -3.0);
        let inv = t.inverse().expect("translation is invertible");
        // inv should translate by (-7, 3)
        let (x, y) = inv.transform_point(10.0, 5.0);
        assert!((x - 3.0).abs() < 1e-5);
        assert!((y - 8.0).abs() < 1e-5);
    }

    #[test]
    fn forward_then_inverse_is_identity() {
        let t = AffineTransform::rotation(37.0)
            .then(&AffineTransform::scale(1.5, 0.8))
            .then(&AffineTransform::translation(20.0, -10.0));
        let inv = t.inverse().expect("composed transform must be invertible");

        let pts = [(0.0f32, 0.0f32), (100.0, 0.0), (0.0, 100.0), (50.0, 50.0)];
        for (px, py) in pts {
            let (fx, fy) = t.transform_point(px, py);
            let (rx, ry) = inv.transform_point(fx, fy);
            assert!((rx - px).abs() < 1e-3, "x round-trip failed: {rx} vs {px}");
            assert!((ry - py).abs() < 1e-3, "y round-trip failed: {ry} vs {py}");
        }
    }

    #[test]
    fn singular_matrix_has_no_inverse() {
        // Collapses the plane to a line — determinant is zero
        let t = AffineTransform::new(1.0, 2.0, 2.0, 4.0, 0.0, 0.0);
        assert!(t.inverse().is_none());
    }

    #[test]
    fn then_composition_order() {
        // Translate then scale ≠ scale then translate
        let a = AffineTransform::translation(10.0, 0.0);
        let b = AffineTransform::scale(2.0, 1.0);
        let ab = a.then(&b);
        let ba = b.then(&a);

        let (abx, _) = ab.transform_point(0.0, 0.0); // (0+10)*2 = 20
        let (bax, _) = ba.transform_point(0.0, 0.0); // 0*2+10 = 10
        assert!((abx - 10.0).abs() < 1e-5);
        assert!((bax - 20.0).abs() < 1e-5);
    }

    #[test]
    fn is_axis_aligned_for_scale_and_translate() {
        let t = AffineTransform::scale(2.0, 3.0)
            .then(&AffineTransform::translation(5.0, 5.0));
        assert!(t.is_axis_aligned());
    }

    #[test]
    fn is_not_axis_aligned_for_rotation() {
        let t = AffineTransform::rotation(45.0);
        assert!(!t.is_axis_aligned());
    }

    // -------------------------------------------------------------------------
    // Output dimension tests
    // -------------------------------------------------------------------------

    #[test]
    fn identity_preserves_dimensions() {
        let t = AffineTransform::identity();
        let (w, h) = get_affine_output_dimensions(100, 80, &t);
        assert_eq!(w, 100);
        assert_eq!(h, 80);
    }

    #[test]
    fn scale_2x_doubles_dimensions() {
        let t = AffineTransform::scale(2.0, 2.0);
        let (w, h) = get_affine_output_dimensions(50, 40, &t);
        assert_eq!(w, 100);
        assert_eq!(h, 80);
    }

    #[test]
    fn rotation_45_degrees_output_size() {
        // A 10×10 image rotated 45° fits in a ceil(10*√2) × ceil(10*√2) box
        let t = AffineTransform::rotation(45.0);
        let (w, h) = get_affine_output_dimensions(10, 10, &t);
        let expected = (10.0_f32 * 2.0_f32.sqrt()).ceil() as usize;
        // Allow ±1 pixel for floating-point rounding
        assert!((w as i64 - expected as i64).abs() <= 1, "w={w} expected≈{expected}");
        assert!((h as i64 - expected as i64).abs() <= 1, "h={h} expected≈{expected}");
    }

    #[test]
    fn rotation_180_preserves_dimensions() {
        let t = AffineTransform::rotation(180.0);
        let (w, h) = get_affine_output_dimensions(100, 80, &t);
        // 180° flip — bounding box is the same size
        assert!((w as i64 - 100).abs() <= 1);
        assert!((h as i64 - 80).abs() <= 1);
    }

    // -------------------------------------------------------------------------
    // Pixel-level channel transform tests
    // -------------------------------------------------------------------------

    fn solid_channel(w: usize, h: usize, val: u8) -> Vec<u8> {
        vec![val; w * h]
    }

    #[test]
    fn identity_channel_is_unchanged() {
        let w = 8usize;
        let h = 8usize;
        let input = (0..w * h).map(|i| i as u8).collect::<Vec<_>>();
        let t = AffineTransform::identity();
        let (ow, oh) = get_affine_output_dimensions(w, h, &t);
        let mut output = vec![0u8; ow * oh];
        affine_transform_channel(&input, &mut output, w, h, ow, oh, &t);
        // Centre pixels should match (edges may differ due to clamping)
        for row in 1..h - 1 {
            for col in 1..w - 1 {
                assert_eq!(output[row * ow + col], input[row * w + col],
                           "mismatch at ({col},{row})");
            }
        }
    }

    #[test]
    fn solid_image_survives_rotation() {
        // A uniform image rotated by any angle must stay uniform in all
        // non-background (non-zero) output pixels.
        let (w, h) = (20, 20);
        let val = 128u8;
        let input = solid_channel(w, h, val);

        for angle in [30.0f32, 45.0, 90.0, 180.0] {
            let t = AffineTransform::rotation(angle);
            let (ow, oh) = get_affine_output_dimensions(w, h, &t);
            let mut output = vec![0u8; ow * oh];
            affine_transform_channel(&input, &mut output, w, h, ow, oh, &t);

            // Interior output pixels (away from transparent border) must be val
            let interior: Vec<u8> = output.iter().cloned().filter(|&p| p > 0).collect();
            let all_correct = interior.iter().all(|&p| (p as i32 - val as i32).abs() <= 2);
            assert!(all_correct,
                    "rotation {angle}°: some non-border pixels deviate from {val}: {:?}",
                    interior.iter().filter(|&&p| (p as i32 - val as i32).abs() > 2).collect::<Vec<_>>());
        }
    }

    #[test]
    fn singular_transform_leaves_output_empty() {
        // A singular transform has no inverse; the channel fn should leave
        // the output zeroed rather than panicking.
        let (w, h) = (10, 10);
        let input = vec![255u8; w * h];
        let singular = AffineTransform::new(1.0, 2.0, 2.0, 4.0, 0.0, 0.0);
        // Use a fallback output size since get_affine_output_dimensions may
        // return degenerate values — just test for no panic.
        let mut output = vec![0u8; w * h];
        // Should not panic
        affine_transform_channel(&input, &mut output, w, h, w, h, &singular);
        assert!(output.iter().all(|&p| p == 0),
                "singular transform should produce all-zero output");
    }

    // -------------------------------------------------------------------------
    // Fixed-point arithmetic edge cases
    // -------------------------------------------------------------------------

    #[test]
    fn div_floor_matches_floor_division() {
        for a in -20i64..=20 {
            for b in [-3i64, -1, 1, 3, 7] {
                let expected = (a as f64 / b as f64).floor() as i64;
                assert_eq!(div_floor(a, b), expected, "div_floor({a},{b})");
            }
        }
    }

    #[test]
    fn div_ceil_matches_ceil_division() {
        for a in -20i64..=20 {
            for b in [-3i64, -1, 1, 3, 7] {
                let expected = (a as f64 / b as f64).ceil() as i64;
                assert_eq!(div_ceil(a, b), expected, "div_ceil({a},{b})");
            }
        }
    }
}