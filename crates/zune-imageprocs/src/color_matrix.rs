//! Perform a color matrix operation
//!
//! A color matrix is a type of operation where the colors of an RGBA image are multiplied by
//! an arbitrary 4*5 matrix.
//!
//! The matrix is equivalent to the operation
//! ```text
//! red   = m[0][0]*r + m[0][1]*g + m[0][2]*b + m[0][3]*a + m[0][4]
//! green = m[1][0]*r + m[1][1]*g + m[1][2]*b + m[1][3]*a + m[1][4]
//! blue  = m[2][0]*r + m[2][1]*g + m[2][2]*b + m[2][3]*a + m[2][4]
//! alpha = m[3][0]*r + m[3][1]*g + m[3][2]*b + m[3][3]*a + m[3][4]
//!```
//! This is most similar to Android's [ColorMatrix](https://developer.android.com/reference/android/graphics/ColorMatrix) operation
//!  with the difference being that matrix values are always between 0 and 1 and the library will do appropriate scaling
//!
//! This is similar to imagemagick's [color-matrix](https://imagemagick.org/script/command-line-options.php?#color-matrix) operator
//! with some examples provided in the website at [Color matrix operator](https://imagemagick.org/Usage/color_mods/#color-matrix)
//!
//!
//! A playground to build color matrices can be found [here](https://fecolormatrix.com/) (external link, not affiliated)
//!
//!
//! ## Examples of color matrix
//!
//! - An identity color matrix that does nothing
//! ```text
//! [[1.0,0.0,0.0,0.0,0.0],
//!  [0.0,1.0,0.0,0.0,0.0]
//!  [0.0,0.0,1.0,0.0,0.0]
//!  [0.0,0.0,0.0,1.0,0.0]]
//! ```
//! - A matrix that converts an RGB image to grayscale in the ratio .2,.5,.3
//!
//! ```text
//![[0.2, 0.5, 0.3, 0.0, 0.0],
//! [0.2, 0.5, 0.3, 0.0, 0.0],
//! [0.2, 0.5, 0.3, 0.0, 0.0],
//! [0.0, 0.0, 0.0, 1.0, 0.0]]
//! ```
//!
//!  - A Matrix that inverts it's color
//!
//! ```text
//! [[-1.0, 0.0, 0.0, 0.0, 1.0],
//   [0.0, -1.0, 0.0, 0.0, 1.0],
//   [0.0, 0.0, -0.1, 0.0, 1.0],
//   [0.0, 0.0,  0.0, 1.0, 1.0]]
//! ```
#![allow(dead_code)]

use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;

/// A color matrix filter
///
/// The filter will convert the colorspace into RGBA,apply the color matrix,
/// and then convert it back to the initial colorspace
///
/// # Example
/// ```rust
/// use zune_image::errors::ImageErrors;
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::traits::OperationsTrait;
/// use zune_imageprocs::color_matrix::ColorMatrix;
///
/// fn main()->Result<(),ImageErrors>{
///    
///     let mut image = Image::fill(0.0f32,ColorSpace::RGB,100,100);
///     // convert to grayscale using a color matrix
///     let filter = ColorMatrix::new(
///         [[0.2, 0.5, 0.3, 0.0, 0.0],
///         [0.2, 0.5, 0.3, 0.0, 0.0],
///         [0.2, 0.5, 0.3, 0.0, 0.0],
///         [0.0, 0.0, 0.0, 1.0, 0.0]]);
///
///     filter.execute(&mut image)?;
///     
///     Ok(())
/// }
/// ```
pub struct ColorMatrix {
    matrix: [[f32; 5]; 4]
}

impl ColorMatrix {
    /// Create a new color matrix
    ///
    /// This will also convert the image to RGBA, process it in that colorspace and then convert
    /// it to the original colorspace, so there is no
    #[must_use]
    pub fn new(matrix: [[f32; 5]; 4]) -> ColorMatrix {
        ColorMatrix { matrix }
    }
    /// Try to create a new color matrix from a slice
    /// of data, the length of the slice must be 20 otherwise
    /// this function will return None
    #[must_use]
    pub fn try_from_slice(slice: &[f32]) -> Option<ColorMatrix> {
        if slice.len() == 20 {
            let mut matrix = [[0f32; 5]; 4];
            let mut c = slice.chunks_exact(5);
            matrix
                .iter_mut()
                .for_each(|x| x.copy_from_slice(c.next().unwrap()));
            Some(ColorMatrix::new(matrix))
        } else {
            None
        }
    }
}

impl OperationsTrait for ColorMatrix {
    fn name(&self) -> &'static str {
        "Color Matrix"
    }

    #[allow(clippy::many_single_char_names)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let original_color = image.colorspace();

        // convert to RGBA
        image.convert_color(ColorSpace::RGBA)?;

        let depth = image.depth();
        for frame in image.frames_mut() {
            let channels = frame.channels_vec();

            let (r, rest) = channels.split_at_mut(1);
            let (g, rest) = rest.split_at_mut(1);
            let (b, a) = rest.split_at_mut(1);

            match depth.bit_type() {
                BitType::U8 => color_matrix_component::<u8>(
                    r[0].reinterpret_as_mut()?,
                    g[0].reinterpret_as_mut()?,
                    b[0].reinterpret_as_mut()?,
                    a[0].reinterpret_as_mut()?,
                    &self.matrix
                ),
                BitType::U16 => color_matrix_component::<u16>(
                    r[0].reinterpret_as_mut()?,
                    g[0].reinterpret_as_mut()?,
                    b[0].reinterpret_as_mut()?,
                    a[0].reinterpret_as_mut()?,
                    &self.matrix
                ),
                BitType::F32 => color_matrix_component::<f32>(
                    r[0].reinterpret_as_mut()?,
                    g[0].reinterpret_as_mut()?,
                    b[0].reinterpret_as_mut()?,
                    a[0].reinterpret_as_mut()?,
                    &self.matrix
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
        }
        // convert back to original color
        image.convert_color(original_color)?;

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

fn color_matrix_component<T: NumOps<T> + Copy>(
    c1: &mut [T], c2: &mut [T], c3: &mut [T], alpha: &mut [T], color_matrix: &[[f32; 5]; 4]
) where
    f32: From<T>
{
    let max_t = f32::from(T::max_val());
    // we need to multiply the first channel with the color matrix and then add the new offset only from the first row

    for (((r, g), b), a) in c1
        .iter_mut()
        .zip(c2.iter_mut())
        .zip(c3.iter_mut())
        .zip(alpha.iter_mut())
    {
        let r_f32 = f32::from(*r);
        let g_f32 = f32::from(*g);
        let b_f32 = f32::from(*b);
        let a_f32 = f32::from(*a);

        let r_matrix = color_matrix[0];
        let g_matrix = color_matrix[1];
        let b_matrix = color_matrix[2];
        let a_matrix = color_matrix[3];

        let new_r = (r_f32 * r_matrix[0])
            + (g_f32 * r_matrix[1])
            + (b_f32 * r_matrix[2])
            + (a_f32 * r_matrix[3])
            + (max_t * r_matrix[4]);

        let new_g = (r_f32 * g_matrix[0])
            + (g_f32 * g_matrix[1])
            + (b_f32 * g_matrix[2])
            + (a_f32 * g_matrix[3])
            + (max_t * g_matrix[4]);

        let new_b = (r_f32 * b_matrix[0])
            + (g_f32 * b_matrix[1])
            + (b_f32 * b_matrix[2])
            + (a_f32 * b_matrix[3])
            + (max_t * b_matrix[4]);

        let new_a = (r_f32 * a_matrix[0])
            + (g_f32 * a_matrix[1])
            + (b_f32 * a_matrix[2])
            + (a_f32 * a_matrix[3])
            + (max_t * a_matrix[4]);

        *r = T::from_f32(new_r);
        *g = T::from_f32(new_g);
        *b = T::from_f32(new_b);
        *a = T::from_f32(new_a);
    }
}

#[cfg(feature = "benchmarks")]
#[cfg(test)]
mod benchmarks {
    extern crate test;

    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    use crate::color_matrix::ColorMatrix;

    #[bench]
    fn bench_color_matrix_on_rgb_image(b: &mut test::Bencher) {
        let width = 800;
        let height = 800;
        let mut image = Image::fill(0.5f32, ColorSpace::RGB, width, height);
        let filter = ColorMatrix::new([
            [0.2, 0.5, 0.3, 0.0, 0.0],
            [0.2, 0.5, 0.3, 0.0, 0.0],
            [0.2, 0.5, 0.3, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0, 0.0]
        ]);

        b.iter(|| {
            filter.execute(&mut image).unwrap();
        });
    }
}
