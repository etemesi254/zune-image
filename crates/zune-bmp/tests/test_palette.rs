// /*
//  * Copyright (c) 2023.
//  *
//  * This software is free software;
//  *
//  * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
//  */
//
// use std::fs::read;
// use std::path::Path;
//
// use zune_bmp::BmpDecoder;
//
// fn open_and_read<P: AsRef<Path>>(path: P) -> Vec<u8> {
//     read(path).unwrap()
// }
//
// fn decode(file: &String) -> Vec<u8> {
//     let file = open_and_read(file);
//     BmpDecoder::new(&file).decode().unwrap()
// }
// //
// #[test]
// fn decode_palette_8bpp() {
//     let path = env!("CARGO_MANIFEST_DIR").to_string() + "/test-images/pal8.bmp";
//     let _ = decode(&path);
// }
