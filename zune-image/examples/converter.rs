/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Shows how you can write a simple image converter using
//! zune library.
//!
//! Any supported format will work with this, the library will
//! automatically encode it for you
//!
//!
//! Shows usage of the `Image::open` and `Image::save` functions
use zune_image::image::Image;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        println!("format {:?} [input] [output]", args[0]);
        return;
    }
    match Image::open(&args[1]) {
        Ok(image) => match image.save(&args[2]) {
            Err(e) => {
                eprintln!("An error occurred encoding image {e:?}")
            }
            Ok(()) => {
                eprintln!("Successfully encoded  {} to {}", &args[1], &args[2])
            }
        },
        Err(e) => {
            eprintln!("An error occurred decoding image {e:?}")
        }
    }
}
