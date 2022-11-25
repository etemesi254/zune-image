use std::fmt::Debug;

use crate::utils::z_prefetch;

/// spatial goes through each pixel on an image collecting its neighbors and picking one
/// based on the function provided.
///
/// The resulting image is then returned.
/// The parameter radius corresponds to the radius of the neighbor area to be searched,
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
///
/// The parameter `function` is the function that receives the list of neighbors and returns the selected
/// neighbor to be used for the resulting image.
///
///
/// # Arguments
///
/// - in_channel: input channel, the width and height are padded with radius*2 edges
///   (use pad function for that). otherwise this function will panic.
///
/// - out_channel: Output channel, the width and height are not padded at all.
///
/// - radius: Area to be searched, example a radius of R will result in a search window
/// length of 2R+1 for each dimension.
///
/// - function: Any function that when given an array returns a single element.
///
pub fn spatial<T: Default + Copy + Debug, F: Fn(&[T]) -> T>(
    in_channel: &[T], out_channel: &mut [T], radius: usize, width: usize, height: usize,
    function: F
)
{
    let old_width = width;
    let height = (radius * 2) + height;
    let width = (radius * 2) + width;

    assert_eq!(height * width, in_channel.len());

    let radius_size = (2 * radius) + 1;

    let radius_loop = radius_size >> 1;

    let mut local_storage = vec![T::default(); radius_size * radius_size];

    for y in radius_loop..height - radius_loop
    {
        for x in radius_loop..width - radius_loop
        {
            let iy = y - radius_loop;
            let ix = x - radius_loop;

            let mut i = 0;

            for ky in 0..radius_size
            {
                let iy_i = iy + ky;

                let in_slice = &in_channel[(iy_i * width) + ix..(iy_i * width) + ix + radius_size];
                z_prefetch(in_channel, (iy_i + 1) * width + ix);
                local_storage[i..i + radius_size].copy_from_slice(in_slice);
                z_prefetch(in_channel, (iy_i + 2) * width + ix);

                i += radius_size;
            }

            let result = function(&local_storage);

            out_channel[iy * old_width + ix] = result;
        }
    }
}
