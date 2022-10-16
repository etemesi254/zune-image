#[allow(clippy::cast_sign_loss)]
pub fn brighten(in_matrix: &mut [u8], value: i16)
{
    in_matrix
        .iter_mut()
        .for_each(|x| *x = i16::from(*x).wrapping_add(value).clamp(0, 255) as u8);
}
