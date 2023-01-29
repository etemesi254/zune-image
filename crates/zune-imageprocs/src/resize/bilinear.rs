use crate::traits::NumOps;

///
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
pub fn bilinear_impl<T>(
    in_image: &[T], out_image: &mut [T], in_width: usize, in_height: usize, out_width: usize,
    out_height: usize
) where
    T: Copy + NumOps<T>,
    f64: std::convert::From<T>
{
    // algorithm is from https://chao-ji.github.io/jekyll/update/2018/07/19/BilinearResize.html
    // and rewritten to remove most working from the inner loop
    if out_width < 1 || out_height < 1
    {
        return;
    }
    assert_eq!(
        in_image.len(),
        in_width * in_height,
        "{in_width},{in_height}"
    );
    let x_ratio = ((in_width - 1) as f64) / ((out_width - 1) as f64);
    let y_ratio = ((in_height - 1) as f64) / ((out_height - 1) as f64);

    for i in 0..out_height
    {
        let y_l = (y_ratio * i as f64).floor();
        let y_h = (y_ratio * i as f64).ceil();
        let y_weight = (y_ratio * i as f64) - y_l;
        let y_l = y_l as usize;
        let y_h = y_h as usize;
        let y_l_stride = y_l * in_height;
        let y_h_stride = y_h * in_height;
        for j in 0..out_width
        {
            let x_l = (x_ratio * j as f64).floor();
            let x_h = (x_ratio * j as f64).ceil();
            let x_weight = (x_ratio * j as f64) - x_l;

            let x_l = x_l as usize;
            let x_h = x_h as usize;

            let a = f64::from(in_image[y_l_stride + x_l]);
            let b = f64::from(in_image[y_l_stride + x_h]);
            let c = f64::from(in_image[y_h_stride + x_l]);
            let d = f64::from(in_image[y_h_stride + x_h]);

            let pixel = a * (1.0 - x_weight) * (1.0 - y_weight)
                + b * x_weight * (1.0 - y_weight)
                + c * y_weight * (1.0 - x_weight)
                + d * x_weight * y_weight;

            out_image[i * out_width + j] = T::from_f64(pixel);
        }
    }
}

#[test]
#[rustfmt::skip]
fn test_bilinear_resize()
{
    let result: [u8; 20] = [
        114, 159, 201, 234, 245, 225, 155, 53, 89, 159,
        152, 165, 170, 137, 126, 146, 185, 234, 154, 41
    ];

    let array: [u8; 30] = [
        114, 195, 254, 217, 33, 160,
        110, 91, 184, 143, 190, 124,
        212, 163, 245, 39, 83, 188,
        23, 206, 62, 7, 5, 206, 152,
        177, 118, 155, 245, 41
    ];

    let new_width = 10;
    let new_height = 2;
    let old_width = 6;
    let old_height = 5;

    let mut new_array = vec![0; new_width * new_height];

    bilinear_impl(
        &array,
        &mut new_array,
        old_width,
        old_height,
        new_width,
        new_height,
    );
    assert_eq!(&new_array, &result);
}
