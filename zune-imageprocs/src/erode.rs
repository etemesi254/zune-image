use std::fmt::Debug;

/// Median returns a new image in which each pixel is the median of its neighbors.
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub fn erode<T: Copy + Ord + Default + Debug>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize
)
{
    for (in_c, out_c) in in_channel
        .chunks_exact(width)
        .zip(out_channel.chunks_exact_mut(width))
    {
        // left edges
        {
            for i in 1..radius
            {
                out_c[i] = *in_c[0..i * 2].iter().max().unwrap_or(&T::default());
            }
        }
        // main edges
        for (window, out) in in_c.windows(radius * 2 + 1).zip(out_c[radius..].iter_mut())
        {
            *out = *window.iter().max().unwrap_or(&T::default());
        }
        // right edges
        {
            *out_c.last_mut().unwrap() = *in_c.last().unwrap();

            let start = out_c.len().saturating_sub(radius * 2);
            let out_start = out_c.len().saturating_sub(radius);

            for i in 0..radius
            {
                let j = i * 2;

                out_c[out_start + i] = *in_c[j + start..].iter().max().unwrap_or(&T::default());
            }
        }
    }
}
