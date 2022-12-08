use crate::traits::NumOps;

#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + NumOps<T> + Ord + Default>(
    channel: &mut [T], value: T, _max_value: T
)
{
    channel
        .iter_mut()
        .for_each(|x| *x = (*x).saturating_add(value));
}
