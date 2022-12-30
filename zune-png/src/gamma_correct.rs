// copied from zune-image/gamma
/// Gamma correct an image
///
/// # Arguments
/// - pixels: Image pixels
/// - value: Recorded gamma of the image, this is what is found in the gAMA chunk after dividing by 100000
/// - max_value: Maximum pixel value, for 8 bit or less images this is 255, for 16 bit images this is 65535
/// # Note
/// It is recommended that you call this once, after having all images, not
/// repeatedly per scanline
#[allow(clippy::needless_range_loop)]
pub fn gamma_correct<T>(pixels: &mut [T], value: f32, max_value: u16)
where
    T: Copy + NumOps<T> + Default
{
    // build a lookup table which we use for gamma correction in the next stage
    // it is faster to do it this way as calling pow in the inner loop is slow

    // must be inclusive so that 65535 and 255 are covered
    let mut lut = vec![f32::default(); usize::from(max_value) + 1];

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
        let gamma_corrected = max_value * pixel_f32.powf(value);

        let mut new_pix_val = max_value * (gamma_corrected * value_inv).powf(1.0 / 2.2);

        if new_pix_val > max_value
        {
            new_pix_val = max_value;
        }

        lut[x & lut_mask] = new_pix_val;
    }
    // now do gamma correction
    for px in pixels
    {
        *px = T::from_f32(lut[(*px).to_usize() & lut_mask]);
    }
}

pub trait NumOps<T>
{
    /// Return this as number casted
    /// to usize
    fn to_usize(self) -> usize;

    fn from_f32(other: f32) -> T;
}

impl NumOps<u16> for u16
{
    #[inline(always)]
    fn to_usize(self) -> usize
    {
        self as usize
    }
    #[inline(always)]
    fn from_f32(other: f32) -> u16
    {
        other as u16
    }
}

impl NumOps<u8> for u8
{
    #[inline(always)]
    fn to_usize(self) -> usize
    {
        self as usize
    }

    #[inline(always)]
    fn from_f32(other: f32) -> u8
    {
        other as u8
    }
}
