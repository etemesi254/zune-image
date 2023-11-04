#![allow(dead_code, unused_variables)]
use zune_image::image::Image;

use crate::traits::NumOps;

pub struct Blend<'src> {
    image: &'src Image,
    alpha: f32
}

pub fn blend_single_channel<T>(src: &[T], dest: &mut [T], src_alpha: f32)
where
    f32: std::convert::From<T>,
    T: Copy + NumOps<T>
{
    let dest_alpha = f32::from(T::max_val()) - src_alpha;
    for (src, dest) in src.iter().zip(dest.iter_mut()) {}
}
