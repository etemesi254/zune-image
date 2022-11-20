use std::ops::Add;

#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn brighten<T: Copy + PartialOrd + Add<Output = T> + Ord + Default>(
    channel: &mut [T], value: T, max_value: T
)
{
    channel
        .iter_mut()
        .for_each(|x| *x = ((*x).add(value)).clamp(T::default(), max_value));
}
