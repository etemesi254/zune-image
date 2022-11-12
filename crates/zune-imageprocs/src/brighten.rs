#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten(channel: &mut [u16], value: i16, max_value: u16)
{
    channel
        .iter_mut()
        .for_each(|x| *x = ((*x as i16).wrapping_add(value) as u16).clamp(0, max_value));
}
