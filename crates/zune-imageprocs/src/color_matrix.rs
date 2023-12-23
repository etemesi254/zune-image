//! Perform a color matrix operation
//!
//! A color matrix is a type of operation where the colors
//! are multiplied by the
//!
//!
#![allow(dead_code)]
use zune_core::colorspace::ColorSpace;

use crate::traits::NumOps;

/// A color matrix filter
pub struct ColorMatrix {
    matrix: [[f32; 5]; 4]
}

impl ColorMatrix {
    /// Create a new color matrix
    ///
    /// This color matrix will be used
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

#[allow(clippy::cast_possible_truncation)]
fn _color_matrix_component1<T: NumOps<T> + Copy>(
    array: &mut [T], color_matrix: &[[f32; 5]; 4], color: ColorSpace
) where
    f32: From<T> + Copy
{
    // we only have to deal with color components and offsets
    // so let's go

    assert_eq!(color.num_components(), 1);
    // we need to multiply the first channel with the color matrix and then add the new offset only from the first row
    let mul_byte = color_matrix[0][0];
    let offset = color_matrix[0][4];
    // scale the offset
    let c = offset * (T::max_val().to_f64() as f32);

    for byte in array {
        let mul = f32::from(*byte);
        let result = (mul * mul_byte) + c;
        *byte = T::from_f32(result);
    }
}
fn _color_matrix_component1_with_alpha<T: NumOps<T> + Copy>(
    _c1: &mut [T], _alpha: &[T], color_matrix: &[[f32; 5]; 4], color: ColorSpace
) {
    assert_eq!(color.num_components(), 1);
    assert!(color.has_alpha());

    // we need to multiply the first channel with the color matrix and then add the new offset only from the first row
    let _c_mul_byte = color_matrix[0][0];
    let _c_alpha = color_matrix[0][3];
    let _c_offset = color_matrix[0][4];

    // let alpha_mul_byte;
    // // scale the offset
    // let c = offset * (T::max_val().to_f64() as f32);
}
