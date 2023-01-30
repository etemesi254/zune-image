use crate::traits::NumOps;

///
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn bilinear_impl<T>(
    _in_image: &[T], _out_image: &mut [T], _in_width: usize, _in_height: usize, _out_width: usize,
    _out_height: usize
) where
    T: Copy + NumOps<T>,
    f64: std::convert::From<T>
{
    // stump
    return;
}
