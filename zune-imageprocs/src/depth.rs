/// Convert an image depth from u16 to u8
///
/// This is a simple division depth rescaling, we simply rescale the image pixels
/// mapping the brightest image pixel (e.g 65535 for 16 bit images) to 255 and darkest to
/// zero, squeezing everything else in between.
///
/// # Arguments
///  - `from`: A reference to pixels in 16 bit format
///  - `to`: A mutable reference to pixels in 8 bit format where we will
/// write our pixels
/// - `max_value`: Maximum value we expect this pixel to store.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn depth_u16_to_u8(from: &[u16], to: &mut [u8], max_value: u16)
{
    //okay do scaling
    let max = 1.0 / f32::from(max_value);
    let scale = 255.0;

    for (old, new) in from.iter().zip(to.iter_mut())
    {
        let new_val = ((f32::from(*old) * max) * scale) as u8;
        *new = new_val;
    }
}

/// Convert an image depth from u8 to u16
///
/// This is a simple multiplication depth rescaling, we simply rescale the image pixels
/// mapping the brightest image pixel (e.g 255 for 16 bit images) to 65535(16 bit) and darkest to
/// zero, stretching everything else in between.
///
/// # Arguments
///  - `from`: A reference to pixels in 16 bit format
///  - `to`: A mutable reference to pixels in 8 bit format where we will
/// write our pixels
/// - `max_value`: Maximum value we expect this pixel to store.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn depth_u8_to_u16(from: &[u8], to: &mut [u16], max_value: u16)
{
    // okay do scaling
    let max = 1.0 / 255.0;
    let scale = f32::from(max_value);

    for (old, new) in from.iter().zip(to.iter_mut())
    {
        let new_val = ((f32::from(*old) * max) * scale) as u16;
        *new = new_val;
    }
}
