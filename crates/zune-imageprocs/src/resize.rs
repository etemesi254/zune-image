use crate::traits::NumOps;

mod bilinear;

#[derive(Copy, Clone, Debug)]
pub enum ResizeMethod
{
    Bilinear
}

/// Resize an image to new dimensions
///
/// # Arguments
/// - in_image: A contiguous slice of a single channel of an image
/// - out_image: Where we will store the new resized pixels
/// - method: The resizing method to use
/// - in_width: `in_image`'s width
/// - in_height:  `in_image`'s height.
/// - out_width: The expected width
/// - out_height: The expected height.
/// # Panics
/// - `in_width*in_height` do not match `in_image.len()`.
/// - `out_width*out_height` do not match `out_image.len()`.
pub fn resize<T>(
    in_image: &[T], out_image: &mut [T], method: ResizeMethod, in_width: usize, in_height: usize,
    out_width: usize, out_height: usize
) where
    T: Copy + NumOps<T>,
    f64: std::convert::From<T>
{
    match method
    {
        ResizeMethod::Bilinear =>
        {
            bilinear::bilinear_impl(
                in_image, out_image, in_width, in_height, out_width, out_height
            );
        }
    }
}
