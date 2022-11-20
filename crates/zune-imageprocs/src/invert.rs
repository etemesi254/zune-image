use std::ops::Sub;

use crate::traits::NumOps;

///Invert a pixel
///
/// The formula for inverting a 8 bit pixel
///  is `pixel[x,y] = 255-pixel[x,y]`
pub fn invert<T>(in_image: &mut [T])
where
    T: NumOps<T> + Sub<Output = T> + Copy
{
    for pixel in in_image.iter_mut()
    {
        *pixel = T::max_val() - *pixel;
    }
}
