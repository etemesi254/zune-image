use crate::traits::NumOps;

/// Bilinear interpolation of a single channel, this interpolates a single channel, but not an image
///
///
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn bilinear_impl<T>(
    in_channel: &[T], out_channel: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize
) where
    T: Copy + NumOps<T>,
    f32: std::convert::From<T>
{
    let w_ratio = 1.0 / out_width as f32 * in_width as f32;
    let h_ratio = 1.0 / out_height as f32 * in_height as f32;

    let smaller_image_to_larger = w_ratio < 1.0 && h_ratio < 1.0;

    for y in 0..out_height {
        let new_y = y as f32 * h_ratio;
        let mut y0 = new_y.floor() as usize;
        let mut y1 = y0 + 1;

        if smaller_image_to_larger {
            y1 = y1.min(in_height - 1);
            y0 = y0.min(in_height - 1);
        }
        let b = new_y - y0 as f32;

        for x in 0..out_width {
            let new_x = x as f32 * w_ratio;
            // floor and truncate are slow due to handling overflow and such, so avoid them here
            let mut x0 = new_x.floor() as usize;
            let mut x1 = x0 + 1;

            // PS: I'm not sure about the impact, but it cuts down on code executed
            // the branch is deterministic hence the CPU should have an easy time predicting it
            if smaller_image_to_larger {
                // in case of result image being greater than source image, it may happen that
                // the above go beyond picture dimensions, so clamp them here if they do
                // clamp to image width and height
                x1 = x1.min(in_width - 1);
                x0 = x0.min(in_width - 1);
            }

            let a = new_x - x0 as f32;

            let p00 = f32::from(in_channel[y0 * in_width + x0]);
            let p10 = f32::from(in_channel[y0 * in_width + x1]);
            let p01 = f32::from(in_channel[y1 * in_width + x0]);
            let p11 = f32::from(in_channel[y1 * in_width + x1]);

            let interpolated_pixel = p00 * (1.0 - a) * (1.0 - b)
                + p10 * a * (1.0 - b)
                + p01 * (1.0 - a) * b
                + p11 * a * b;

            out_channel[y * out_width + x] = T::from_f32(interpolated_pixel);
        }
    }
}
