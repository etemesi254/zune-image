use crate::gaussian_blur::{gaussian_blur_u16, gaussian_blur_u8};

#[allow(clippy::too_many_arguments)]
pub fn unsharpen_u16(
    channel: &mut [u16], blur_buffer: &mut [u16], blur_scratch_buffer: &mut [u16], sigma: f32,
    threshold: u16, _percentage: u16, width: usize, height: usize
)
{
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u16(blur_buffer, blur_scratch_buffer, width, height, sigma);
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

        // let diff = (diff * percentage) / 100;

        // if diff > threshold { pix = (diff + pix) } else { pix }
        *in_pix = (in_pix.wrapping_add(diff) & !threshold_mask) | (*in_pix & threshold_mask);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn unsharpen_u8(
    channel: &mut [u8], blur_buffer: &mut [u8], blur_scratch_buffer: &mut [u8], sigma: f32,
    threshold: u8, _percentage: u8, width: usize, height: usize
)
{
    // copy channel to scratch space
    blur_buffer.copy_from_slice(channel);
    // carry out gaussian blur
    gaussian_blur_u8(blur_buffer, blur_scratch_buffer, width, height, sigma);
    // blur buffer now contains gaussian blurred pixels
    // so iterate replacing them
    for (in_pix, blur_pix) in channel.iter_mut().zip(blur_buffer.iter())
    {
        let diff = in_pix.wrapping_sub(*blur_pix);
        // pull some branchless tricks to help the optimizer
        // here

        // We conditionally take the added version or whatever we had based on this mask
        //  godbolt link: https://godbolt.org/z/YYnEaPedM
        let threshold_mask = u8::from(diff > threshold).wrapping_sub(1);

        // if diff > threshold { pix = (diff + pix) } else { pix }
        *in_pix = (in_pix.wrapping_add(diff) & !threshold_mask) | (*in_pix & threshold_mask);
    }
}
