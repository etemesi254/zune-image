use crate::traits::NumOps;

#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + NumOps<T> + Default>(
    channel: &mut [T], value: T, _max_value: T
)
{
    channel
        .iter_mut()
        .for_each(|x| *x = (*x).saturating_add(value));
}

pub fn brighten_f32(channel: &mut [f32], value: f32, max_value: f32)
{
    channel
        .iter_mut()
        .for_each(|x| *x = (*x + value).clamp(0.0, max_value));
}
