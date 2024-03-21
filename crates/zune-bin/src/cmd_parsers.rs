/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use clap::ArgMatches;
use zune_core::colorspace::ColorSpace;
use zune_core::options::{DecoderOptions, EncoderOptions};

pub mod global_options;

pub mod filters;
pub mod operations;

pub fn decoder_options(options: &ArgMatches) -> DecoderOptions {
    let max_width = *options.get_one::<usize>("max-width").unwrap();
    let max_height = *options.get_one::<usize>("max-height").unwrap();
    let use_unsafe = !*options.get_one::<bool>("safe").unwrap();
    let strict_mode = *options.get_one::<bool>("strict").unwrap();
    let jpeg_grayscale = *options.get_one::<bool>("jpeg-grayscale").unwrap_or(&false);

    let mut options = DecoderOptions::new_cmd()
        .set_max_height(max_height)
        .set_max_width(max_width)
        .set_use_unsafe(use_unsafe)
        .set_strict_mode(strict_mode);

    if jpeg_grayscale {
        options = options.jpeg_set_out_colorspace(ColorSpace::Luma);
    }
    options
}

pub fn encoder_options(options: &ArgMatches) -> EncoderOptions {
    let quality = *options.get_one::<u8>("quality").unwrap();
    let encode_threads = *options.get_one::<u8>("encode-threads").unwrap();
    let effort = *options.get_one::<u8>("effort").unwrap();
    let progressive = options.contains_id("progressive");
    let strip_metadata = options.contains_id("strip");

    EncoderOptions::default()
        .set_quality(quality)
        .set_num_threads(encode_threads)
        .set_effort(effort)
        .set_strip_metadata(strip_metadata)
        .set_jpeg_encode_progressive(progressive)
}
