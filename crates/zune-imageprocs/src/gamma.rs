#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn gamma(pixels: &mut [u8], value: f32)
{
    // Default gamma is 1/2.2
    let gamma_inv = 1.0 / value;

    // We have to do the inverse ourselves, fp math won't convert it
    // to inv slightly slowing the loop below.
    //
    // The reason we are slow is because the powf cannot be inlines
    // so this can't be vectorized and unrolling doesn't help as execution
    // always has to jump to the caller
    let value_inv = 1.0 / 255.0;

    for pixel in pixels
    {
        let pixel_f32 = f32::from(*pixel) * value_inv;
        let gamma_corrected = 255.0 * pixel_f32.powf(value);
        let new_pix_val = 255.0 * (gamma_corrected * value_inv).powf(1.0 / 2.2);
        *pixel = new_pix_val.clamp(0.0, 255.0) as u8;
    }
}
