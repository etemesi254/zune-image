use zune_core::bit_depth::BitType;
use zune_core::log::warn;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;
use crate::utils::execute_on;

/// Affine transformation matrix in the form:
/// | a  b  tx |
/// | c  d  ty |
/// | 0  0  1  |
#[derive(Debug, Clone, Copy)]
pub struct AffineTransform {
    pub a:  f32,
    pub b:  f32,
    pub c:  f32,
    pub d:  f32,
    pub tx: f32,
    pub ty: f32
}

impl AffineTransform {
    pub fn new(a: f32, b: f32, c: f32, d: f32, tx: f32, ty: f32) -> Self {
        Self { a, b, c, d, tx, ty }
    }
    /// Identity transform (no change)
    pub fn identity() -> Self {
        Self {
            a:  1.0,
            b:  0.0,
            c:  0.0,
            d:  1.0,
            tx: 0.0,
            ty: 0.0
        }
    }

    /// Rotation around origin
    pub fn rotation(angle: f32) -> Self {
        let rad = angle.to_radians();
        let cos = rad.cos();
        let sin = rad.sin();
        Self {
            a:  cos,
            b:  -sin,
            c:  sin,
            d:  cos,
            tx: 0.0,
            ty: 0.0
        }
    }

    /// Translation
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty
        }
    }

    /// Scaling
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a:  sx,
            b:  0.0,
            c:  0.0,
            d:  sy,
            tx: 0.0,
            ty: 0.0
        }
    }

    /// Shear
    pub fn shear(shx: f32, shy: f32) -> Self {
        Self {
            a:  1.0,
            b:  shx,
            c:  shy,
            d:  1.0,
            tx: 0.0,
            ty: 0.0
        }
    }

    /// Compose two transforms (multiply matrices)
    pub fn then(&self, other: &AffineTransform) -> Self {
        Self {
            a:  self.a * other.a + self.b * other.c,
            b:  self.a * other.b + self.b * other.d,
            c:  self.c * other.a + self.d * other.c,
            d:  self.c * other.b + self.d * other.d,
            tx: self.a * other.tx + self.b * other.ty + self.tx,
            ty: self.c * other.tx + self.d * other.ty + self.ty
        }
    }

    /// Apply transform to a point
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.b * y + self.tx,
            self.c * x + self.d * y + self.ty
        )
    }

    /// Invert the transform (for reverse mapping)
    pub fn inverse(&self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < f32::EPSILON {
            return None; // Non-invertible
        }

        let inv_det = 1.0 / det;
        Some(Self {
            a:  self.d * inv_det,
            b:  -self.b * inv_det,
            c:  -self.c * inv_det,
            d:  self.a * inv_det,
            tx: (self.b * self.ty - self.d * self.tx) * inv_det,
            ty: (self.c * self.tx - self.a * self.ty) * inv_det
        })
    }
}

impl OperationsTrait for AffineTransform {
    fn name(&self) -> &'static str {
        "Affine Transform"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (w, h) = image.dimensions();
        let depth = image.depth().bit_type();
        let (new_w, new_h) = get_affine_output_dimensions(w, h, self);

        // Get inverse transform for reverse mapping
        match self.inverse() {
            Some(inv) => inv,
            None => {
                warn!("a={},b={},c={},d={},tx={},ty={}", self.a, self.b, self.c,self.d, self.tx, self.ty);
                return Err(ImageErrors::GenericString("AffineTransform doesn't have inverse".to_string()));
            } // Degenerate transform
        };
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
                    self
                ),
                BitType::U16 => affine_transform_channel::<u16>(
                    channel.reinterpret_as()?,
                    new_channel.reinterpret_as_mut()?,
                    w,
                    h,
                    new_w,
                    new_h,
                    self
                ),
                BitType::F32 => affine_transform_channel::<f32>(
                    channel.reinterpret_as()?,
                    new_channel.reinterpret_as_mut()?,
                    w,
                    h,
                    new_w,
                    new_h,
                    self
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented("resize", d))
            }
            *channel = new_channel;
            Ok(())
        };

        execute_on(affine_fn, image, false)?;
        image.set_dimensions(new_w, new_h);
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U16, BitType::U8]
    }
}

/// Calculate output dimensions needed for an affine transform
pub fn get_affine_output_dimensions(
    width: usize, height: usize, transform: &AffineTransform
) -> (usize, usize) {
    // Transform all four corners
    let corners = [
        (0.0, 0.0),
        (width as f32, 0.0),
        (0.0, height as f32),
        (width as f32, height as f32)
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

    let new_width = (max_x - min_x).ceil() as usize;
    let new_height = (max_y - min_y).ceil() as usize;

    (new_width, new_height)
}

/// Apply affine transform to a single channel of u8 data
pub fn affine_transform_channel<T: Copy + Default + NumOps<T>>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize, transform: &AffineTransform
) {
    // Get inverse transform for reverse mapping
    let inv_transform = match transform.inverse() {
        Some(inv) => inv,
        None => {
            return;
        } // Degenerate transform
    };

    out_channel.fill(T::max_val());

    let a = inv_transform.a;
    let b = inv_transform.b;
    let c = inv_transform.c;
    let d = inv_transform.d;
    let tx = inv_transform.tx;
    let ty = inv_transform.ty;


    // Calculate offset to center the output
    let corners = [
        (0.0, 0.0),
        (in_width as f32, 0.0),
        (0.0, in_height as f32),
        (in_width as f32, in_height as f32)
    ];

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;

    for (x, y) in corners {
        let (tx, ty) = transform.transform_point(x, y);
        min_x = min_x.min(tx);
        min_y = min_y.min(ty);
    }

    // Process each output pixel
    for out_y in 0..out_height {
        let y = out_y as f32 + min_y;

        // Pre-compute y-dependent terms
        let by = b * y;
        let dy = d * y;

        for out_x in 0..out_width {

            let x = out_x as f32 + min_x;

            // Now just multiply and add
            let src_x = a * x + by + tx;
            let src_y = c * x + dy + ty;

            // Bilinear interpolation
            if src_x >= 0.0
                && src_x < (in_width - 1) as f32
                && src_y >= 0.0
                && src_y < (in_height - 1) as f32
            {
                let x0 = src_x.floor() as usize;
                let y0 = src_y.floor() as usize;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                let fx = src_x - x0 as f32;
                let fy = src_y - y0 as f32;

                let p00 = in_channel[y0 * in_width + x0].to_f32();
                let p10 = in_channel[y0 * in_width + x1].to_f32();
                let p01 = in_channel[y1 * in_width + x0].to_f32();
                let p11 = in_channel[y1 * in_width + x1].to_f32();

                let result = p00 * (1.0 - fx) * (1.0 - fy)
                    + p10 * fx * (1.0 - fy)
                    + p01 * (1.0 - fx) * fy
                    + p11 * fx * fy;

                out_channel[out_y * out_width + out_x] = T::from_f32(result);
            }
        }
    }
}
