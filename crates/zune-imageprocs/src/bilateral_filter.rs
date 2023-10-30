// use std::io::BufRead;
// use zune_core::colorspace::ColorSpace;
// use zune_core::options::DecoderOptions;
// use zune_image::image::Image;
// use crate::pad::PadMethod;
//
// pub fn bilateral_filter(
//     src: &[u8],
//     dest: &mut [u8],
//     width: usize,
//     height: usize,
//     mut d: i32,
//     mut sigma_color: f32,
//     mut sigma_space: f32,
// ) {
//     let gauss_color_coeff = -0.5 / (sigma_color * sigma_color);
//     let gauss_space_coeff = -0.5 / (sigma_space * sigma_space);
//     let cn = 1;
//     let radius: i32;
//
//     if sigma_color <= 0.0 {
//         sigma_color = 1.0;
//     }
//     if sigma_space <= 0.0 {
//         sigma_space = 1.0;
//     }
//
//     if d <= 0 {
//         radius = (sigma_space * 1.5).round() as _;
//     } else {
//         radius = d / 2;
//     }
//
//
//     let padded_src = crate::pad::pad(&src, width, height, radius as usize, radius as usize, PadMethod::Replicate);
//
//     let mut color_weight = vec![0.0_f32; cn * 256];
//     let mut space_weight = vec![0.0_f32; (d * d) as usize];
//     let mut space_offs = vec![0_i32; (d * d) as usize];
//
//     let new_height = (radius as usize * 2) + height;
//     let new_width = (radius as usize * 2) + width;
//
//     // initialize color-related bilateral filter coeffs
//     for i in 0..256 {
//         let c = i as f32;
//         color_weight[i] = (c * c * gauss_color_coeff).exp();
//     }
//     let mut makx = 0;
//     // initialize space-related bilateral coeffs
//     for i in -radius..=radius {
//         for j in -radius..=radius {
//             let r = (((i * i) + (j * j)) as f32).sqrt();
//             if r > radius as f32 {
//                 continue;
//             }
//             space_weight[makx] = (r * r * gauss_space_coeff).exp();
//             space_offs[makx] = ((i as i32 * width as i32) + (j * (cn as i32)));
//             makx += 1;
//         }
//     }
//     let rs = radius as usize;
//
//     for y in rs..new_height - rs {
//         let iy = (y - rs);
//         let src_offset = (((y * new_width) as isize) - space_offs[0] as isize) as usize;
//         let src_ptr = &padded_src[src_offset..];
//         let dst_ptr = &mut dest[iy * width..];
//
//         for z in rs..new_width - rs {
//             let j = z - rs;
//
//             let iy = y - rs;
//
//             let mut sum = 0.0;
//             let mut wsum = 0.0;
//             let val0 = padded_src[(y * new_width) + z] as i32;
//             for k in 0..makx {
//                 let space_off = space_offs[k] as isize;
//                 let offset = j as isize + space_off;
//                 let len=src_ptr.len();
//
//
//                 let val = unsafe{ *src_ptr.as_ptr().offset(offset) } as i32;
//                 let c = (val - val0).abs();
//
//                 let w = space_weight[k] * color_weight[(val - val0).abs() as usize];
//                 sum += (val as f32) * w;
//                 wsum += w;
//             }
//             let t = (sum / wsum);
//             let src_pix = src[j];
//             dst_ptr[j] = (sum / wsum).round() as u8;
//         }
//     }
//
//     // for i in 0..height {
//     //     let radius = radius as usize;
//     //     let pos = ((i + radius) * width + radius) as usize;
//     //     let src_ptr = &padded_src[pos..];
//     //     let dst_ptr = &mut dest[i * width..];
//     //     for j in 0..width {
//     //         for j in 0..width {
//     //             let mut sum = 0.0;
//     //             let mut wsum = 0.0;
//     //             let mut val0 = src_ptr[j] as i32;
//     //             for k in 0..makx {
//     //                 let offset = j as isize + space_offs[k] as isize;
//     //                 if offset.is_negative() {
//     //                     assert!((offset.abs() as usize) <= pos, "{},{}", offset.abs(), pos);
//     //                 }
//     //                 let val = unsafe { *src_ptr.as_ptr().offset(offset) } as i32;
//     //                 let c = (val - val0).abs();
//     //
//     //                 let w = space_weight[k] * color_weight[(val - val0).abs() as usize];
//     //                 sum += (val as f32) * w;
//     //                 wsum += w;
//     //             }
//     //             dst_ptr[j] = (sum / wsum).round() as u8;
//     //         }
//     //     }
//     // }
// }
//
//
// #[test]
// fn hello() {
//     let mut pixels = Image::open("/home/caleb/Pictures/ANIME/wallhaven-5d89w9_1920x1080.png").unwrap();
//     pixels.convert_color(ColorSpace::Luma).unwrap();
//     let src = &pixels.flatten_to_u8()[0];
//     let (width, height) = pixels.get_dimensions();
//     let radius = 20;
//     let mut dest = vec![0_u8; width * height];
//
//     bilateral_filter(&src, &mut dest, width, height, radius as i32, 75.0, 75.0);
//     Image::from_u8(&dest, width, height, ColorSpace::Luma).save("./hello.jpg").unwrap();
// }
