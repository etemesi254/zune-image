use std::ops::Sub;

use crate::traits::NumOps;

///Invert a pixel
///
/// The formula for inverting a 8 bit pixel
///  is `pixel[x,y] = 255-pixel[x,y]`
pub fn invert<T: NumOps<T> + Sub<Output = T> + Copy>(in_image: &mut [T])
{
    in_image.iter_mut().for_each(|x| *x = T::max_val() - *x);
}
