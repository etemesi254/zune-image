use std::fmt::Debug;

/// Median returns a new image in which each pixel is the median of its neighbors.
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub fn median<T: Copy + Ord + Default + Debug>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize
)
{
    let mut local_storage = vec![T::default(); (2 * radius) + 1];

    let mut prev = T::default();
    let middle = local_storage.len() / 2;
    for (in_c, out_c) in in_channel
        .chunks_exact(width)
        .zip(out_channel.chunks_exact_mut(width))
    {
        let mut seen = false;

        // left edges
        {
            for i in 1..radius
            {
                let j = i * 2;
                local_storage[0..j].copy_from_slice(&in_c[0..j]);

                local_storage[0..j].sort_unstable();

                out_c[i] = local_storage[j / 2];
            }
        }
        // move width wise
        for (window, out) in in_c.windows(radius * 2 + 1).zip(out_c[radius..].iter_mut())
        {
            if seen
            {
                // if we hit here we know that local_storage contains our sorted item
                // and prev contains our item that dropped out of the window
                // so to get the other array, we simply find position of prev
                // and replace it with the item in the window that just came in
                // (the rightmost)
                // and sort that
                //
                // This helps us to eliminate a lot of sorting and shuffling
                // Cuts the runtime by A-LOT
                //
                // Clay Banks Median filter radius 300
                // - NAIVE: 53714 ms
                // - This : 7497 ms (half is handling edge pixels)
                //
                // That's what we call an optimization baby!!!
                //
                let pos = local_storage.binary_search(&prev).unwrap();
                // at pos overwrite with new item in window
                local_storage[pos] = *window.last().unwrap_or(&T::default());
                // now sort
                local_storage.sort_unstable();

                *out = local_storage[middle];
                // next iteration
                prev = window[0];
            }
            else
            {
                prev = window[0];
                // copy to local storage
                local_storage.copy_from_slice(window);
                // sort local storage
                local_storage.sort_unstable();
                // get middle
                *out = local_storage[middle];
                seen = true;
            }
        }
        // right edges
        {
            *out_c.last_mut().unwrap() = *in_c.last().unwrap();

            let start = out_c.len().saturating_sub(radius * 2);
            let out_start = out_c.len().saturating_sub(radius);

            for i in 0..radius
            {
                let j = i * 2;

                local_storage[0..((radius * 2) - j)].copy_from_slice(&in_c[j + start..]);
                local_storage[0..((radius * 2) - j)].sort_unstable();
                out_c[out_start + i] = local_storage[((radius * 2) - j) / 2];
            }
        }
    }
}
