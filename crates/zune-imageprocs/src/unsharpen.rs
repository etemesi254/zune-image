use crate::gaussian_blur::gaussian_blur;

pub fn unsharpen(
    channel: &mut [u16], blur_buffer: &mut [u16], blur_scratch_buffer: &mut [u16], sigma: f32,
    threshold: u16, width: usize, height: usize,
)
{
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur(blur_buffer, blur_scratch_buffer, width, height, sigma);
    // blur buffer now contains gaussian blurred pixels
    // so iterate replacing them
    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter())
    {
        let diff = in_pix.saturating_sub(*blur_pix);
        // pull some branchless tricks to help the optimizer
        // here

        // We conditionally take the added version or whatever we had based on this mask
        //  godbolt link: https://godbolt.org/z/YYnEaPedM

        let threshold_mask = u16::from(diff > threshold).wrapping_sub(1);

        // if diff > threshold { pix = (diff + pix) } else { pix }
        *in_pix = (in_pix.saturating_add(diff) & !threshold_mask) | (*in_pix & threshold_mask);
    }
}
