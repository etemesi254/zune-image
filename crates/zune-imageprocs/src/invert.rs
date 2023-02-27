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

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::invert::invert;

    #[bench]
    fn invert_u8(b: &mut test::Bencher)
    {
        let mut in_out = vec![0_u8; 800 * 800];

        b.iter(|| {
            invert(&mut in_out);
        });
    }

    #[bench]
    fn invert_u16(b: &mut test::Bencher)
    {
        let mut in_out = vec![0_u8; 800 * 800];

        b.iter(|| {
            invert(&mut in_out);
        });
    }
}
