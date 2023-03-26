// #![cfg(test)]
//
// #[test]
// fn test_fractal()
// {
//     use zune_core::colorspace::ColorSpace;
//
//     use crate::codecs::jpeg::JpegEncoder;
//     use crate::traits::EncoderTrait;
//
//     let img_x = 800;
//     let img_y = 800;
//
//     let scale_x = 3.0 / img_x as f32;
//     let scale_y = 3.0 / img_y as f32;
//
//     let mut image = crate::image::Image::from_fn(img_x, img_y, ColorSpace::RGB, |x, y, px| {
//         let r = (0.3 * x as f32) as u8;
//         let b = (0.3 * y as f32) as u8;
//         px[0] = r;
//         px[2] = b;
//     });
//
//     image
//         .modify_pixels_mut::<u8, _>(|y, x, px| {
//             let cx = y as f32 * scale_x - 1.5;
//             let cy = x as f32 * scale_y - 1.5;
//
//             let c = num_complex::Complex::new(-0.4, 0.6);
//             let mut z = num_complex::Complex::new(cx, cy);
//
//             let mut i = 0;
//             while i < 255 && z.norm() <= 2.0
//             {
//                 z = z * z + c;
//                 i += 1;
//             }
//             // write it
//             *px[1] = i as u8;
//         })
//         .unwrap();
//
//     let mut encoder = JpegEncoder::new(80);
//     let px = encoder.encode(&image).unwrap();
//
//     std::fs::write("./p2.jpg", px).unwrap();
// }
//
// #[test]
// fn test_fractal_image_rs()
// {
//     let imgx = 800;
//     let imgy = 800;
//
//     let scalex = 3.0 / imgx as f32;
//     let scaley = 3.0 / imgy as f32;
//
//     // Create a new ImgBuf with width: imgx and height: imgy
//     let mut imgbuf = image::ImageBuffer::new(imgx, imgy);
//
//     // Iterate over the coordinates and pixels of the image
//     for (x, y, pixel) in imgbuf.enumerate_pixels_mut()
//     {
//         let r = (0.3 * x as f32) as u8;
//         let b = (0.3 * y as f32) as u8;
//         *pixel = image::Rgb([r, 0, b]);
//     }
//
//     // A redundant loop to demonstrate reading image data
//     for x in 0..imgx
//     {
//         for y in 0..imgy
//         {
//             let cx = y as f32 * scalex - 1.5;
//             let cy = x as f32 * scaley - 1.5;
//
//             let c = num_complex::Complex::new(-0.4, 0.6);
//             let mut z = num_complex::Complex::new(cx, cy);
//
//             let mut i = 0;
//             while i < 255 && z.norm() <= 2.0
//             {
//                 z = z * z + c;
//                 i += 1;
//             }
//
//             let pixel = imgbuf.get_pixel_mut(x, y);
//             let image::Rgb(data) = *pixel;
//             *pixel = image::Rgb([data[0], i as u8, data[2]]);
//         }
//     }
//
//     // Save the image as “fractal.png”, the format is deduced from the path
//     imgbuf.save("fractal.ppm").unwrap();
// }
