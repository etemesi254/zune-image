use crate::traits::NumOps;

#[allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    clippy::cast_lossless,
    clippy::needless_range_loop,
    clippy::cast_precision_loss
)]
pub fn gamma<T>(pixels: &mut [T], value: f32, max_value: u16)
where
    T: Copy + NumOps<T> + Default
{
    // build a lookup table which we use for gamma correction in the next stage
    // it is faster to do it this way as calling pow in the inner loop is slow

    // must be inclusive so that 65535 and 255 are covered
    let mut lut = vec![T::default(); usize::from(max_value) + 1];

    let max_usize = usize::from(max_value);
    let max_value = max_value as f32;
    let value_inv = 1.0 / max_value;
    // optimizer hint to remove bounds check, these values should be
    // powers of two, currently we support 255 and 65535
    assert!(lut.len().is_power_of_two());
    let lut_mask = lut.len() - 1;

    for x in 0..=max_usize
    {
        let pixel_f32 = (x as f32) * value_inv;
        let mut new_pix_val = max_value * pixel_f32.powf(value);

        if new_pix_val > max_value
        {
            new_pix_val = max_value;
        }

        lut[x & lut_mask] = T::from_f32(new_pix_val);
    }
    // now do gamma correction
    for px in pixels
    {
        *px = lut[(*px).to_usize() & lut_mask];
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::gamma::gamma;

    #[bench]
    fn gamma_bench(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0_u16; dimensions];

        b.iter(|| {
            gamma(&mut c1, 2.3, 255);
        });
    }
}
