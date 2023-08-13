/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(test)]

use crate::codecs::ImageFormat;

#[test]
fn test_fractal() {
    use zune_core::colorspace::ColorSpace;

    let img_x = 800;
    let img_y = 800;

    let scale_x = 3.0 / img_x as f32;
    let scale_y = 3.0 / img_y as f32;

    let mut image = crate::image::Image::from_fn(img_x, img_y, ColorSpace::RGB, |x, y, px| {
        let r = (0.3 * x as f32) as u8;
        let b = (0.3 * y as f32) as u8;
        px[0] = r;
        px[2] = b;
    });

    image
        .modify_pixels_mut::<u8, _>(|y, x, px| {
            let cx = y as f32 * scale_x - 1.5;
            let cy = x as f32 * scale_y - 1.5;

            let c = num_complex::Complex::new(-0.4, 0.6);
            let mut z = num_complex::Complex::new(cx, cy);

            let mut i = 0;
            while i < 255 && z.norm() <= 2.0 {
                z = z * z + c;
                i += 1;
            }
            // write it
            *px[1] = i as u8;
        })
        .unwrap();
    image.save_to("a.ppm", ImageFormat::PPM).unwrap()
}
