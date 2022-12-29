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
    let mut lut = vec![f32::default(); usize::from(max_value) + 1];

    {
        // create a lookup table for conversion.

        let max_usize = usize::from(max_value);
        let max_value = max_value as f32;
        let value_inv = 1.0 / max_value;

        // inclusive so that 65535 and 255 are covered
        for x in 0..=max_usize
        {
            let pixel_f32 = (x as f32) * value_inv;
            let gamma_corrected = max_value * pixel_f32.powf(value);

            let mut new_pix_val = max_value * (gamma_corrected * value_inv).powf(1.0 / 2.2);

            if new_pix_val > max_value
            {
                new_pix_val = max_value;
            }

            lut[x] = new_pix_val;
        }
    }
    // now do gamma correction
    for px in pixels
    {
        *px = T::from_f32(lut[(*px).to_usize()]);
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
