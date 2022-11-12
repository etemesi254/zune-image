/// Create different box radius for each gaussian kernel function.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_range_loop
)]
fn create_box_gauss(sigma: f32) -> [usize; 3]
{
    let mut radii = [1_usize; 3];
    if sigma > 0.0
    {
        let n_float = 3.0;

        // Ideal averaging filter width
        let w_ideal = (12.0 * sigma * sigma / n_float).sqrt() + 1.0;
        let mut wl: i32 = w_ideal.floor() as i32;

        if wl % 2 == 0
        {
            wl -= 1;
        };

        let wu = (wl + 2) as usize;

        let wl_float = wl as f32;

        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);

        let m: usize = m_ideal.round() as usize;

        for i in 0..3
        {
            if i < m
            {
                radii[i] = wl as usize;
            }
            else
            {
                radii[i] = wu as usize;
            }
        }
    }

    radii
}

pub fn gaussian_blur(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize, sigma: f32,
)
{
    // use the box blur implementation
    let blur_radii = create_box_gauss(sigma);

    for blur_radius in blur_radii
    {
        // approximate gaussian blur using multiple box blurs
        crate::box_blur::box_blur(in_out_image, scratch_space, width, height, blur_radius);
    }
}
