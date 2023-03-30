// #![cfg(feature = "exr")]
//
// use std::io::Cursor;
//
// use exr::prelude::{f16, ReadChannels, ReadLayers};
// use zune_core::colorspace::ColorSpace;
//
// use crate::errors::ImageErrors;
// use crate::image::Image;
// use crate::traits::DecoderTrait;
//
// pub struct ExrDecoder<'a>
// {
//     data: &'a [u8]
// }
//
// impl<'a> ExrDecoder<'a>
// {
//     fn a(&self)
//     {
//         let reader = exr::prelude::read()
//             .no_deep_data()
//             .largest_resolution_level()
//             .rgba_channels(
//                 // create our image based on the resolution of the file
//                 |resolution, (r, g, b, a)| {
//                     let mut output: Vec<f32> = vec![];
//
//                     output.resize(resolution.x() * resolution.y() * 4, 0.0);
//                     output
//                 },
//                 // insert a single pixel into out image
//                 |output, position, (r, g, b, a): (f32, f32, f32, f32)| {
//                     let first = (position.y() * position.width() + position.x()) * 4;
//
//                     output[first] = r;
//                     output[first + 1] = g;
//                     output[first + 2] = b;
//                     output[first + 3] = a;
//                 }
//             )
//             .first_valid_layer()
//             .all_attributes()
//             .from_buffered(Cursor::new(self.data))
//             .unwrap();
//     }
// }
//
// impl<'a> DecoderTrait<'a> for ExrDecoder<'a>
// {
//     fn decode(&mut self) -> Result<Image, ImageErrors>
//     {
//         todo!()
//     }
//
//     fn get_dimensions(&self) -> Option<(usize, usize)>
//     {
//         todo!()
//     }
//
//     fn get_out_colorspace(&self) -> ColorSpace
//     {
//         todo!()
//     }
//
//     fn get_name(&self) -> &'static str
//     {
//         "exr"
//     }
// }
