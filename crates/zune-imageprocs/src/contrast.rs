//! Calculate image contrast
//! Algorithm is from https://www.dfstudios.co.uk/articles/programming/image-programming-algorithms/image-processing-algorithms-part-5-contrast-adjustment/
//!
//! Steps repeated here for convenience
//!
//! First step is to calculate a contrast correlation factor
//!
//! ```text
//! f = 259(c+255)/(255(259-c))
//!```
//! `c` is the desired level of contrast.
//! `f` is the constant correlation factor.
//!
//! The next step is to perform the contrast adjustment
//! ```text
//! R' = F(R-128)+128
//! ```

/// Calculate the contrast of an image
///
/// See module docs for formula
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn contrast_u8(channel: &mut [u8], contrast: f32)
{
    // calculate correlation factor
    let factor = (259.0 * (contrast + 255.0)) / (255.0 * (259.0 - contrast));

    for pix in channel
    {
        let float_pix = f32::from(*pix);
        let new_val = ((factor * (float_pix - 128.0)) + 128.0).clamp(0.0, 255.0);
        // clamp should happen automatically??
        *pix = new_val as u8;
    }
}
