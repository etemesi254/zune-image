/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ffi::OsString;

use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, ArgGroup, Command, ValueEnum};

use crate::cmd_args::arg_parsers::IColorSpace;
use crate::cmd_args::help_strings::{
    AFTER_HELP, BOX_BLUR_HELP, BRIGHTEN_HELP, COLORSPACE_HELP, CROP_HELP, GAUSSIAN_BLUR_HELP,
    THRESHOLD_HELP, TRANSPOSE_HELP
};

pub mod arg_parsers;
pub mod help_strings;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MmapOptions {
    No,
    Always,
    Auto
}

impl ValueEnum for MmapOptions {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::No, Self::Auto, Self::Always]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::No => PossibleValue::new("no"),
            Self::Always => PossibleValue::new("always"),
            Self::Auto => PossibleValue::new("auto")
        })
    }
}
#[rustfmt::skip]
pub fn create_cmd_args() -> Command {
    let (options_args, option_group) = add_operations();
    let (filter_args, filter_group) = add_filters();
    let (encode_args, encode_group) = add_encode_options();
    let (image_args, image_args_group) = add_image_specific_settings();

    Command::new("zune")
        .after_help(AFTER_HELP)
        .author("Caleb Etemesi")
        .version(env!("CARGO_PKG_VERSION"))
        .next_line_help(false)
        .term_width(200)
        .arg(Arg::new("in")
            .short('i')
            .help("Input file to read data from")
            .long("input")
            .action(ArgAction::Set)
            .value_parser(value_parser!(OsString))
            .required(true))
        .arg(Arg::new("out")
            .short('o')
            .long("out")
            .help("Output to write the data to")
            .action(ArgAction::Append)
            .value_parser(value_parser!(OsString))
        )
        .arg(Arg::new("mmap")
            .long("mmap")
            .help_heading("ADVANCED")
            //.takes_value(true)
            .help("Influence the use of memory maps")
            .long_help("Change use of memory maps and how they are used for decoding.\nMemory maps are preferred for large images to keep memory usage low.")
            .value_parser(value_parser!(MmapOptions)))
        .arg(Arg::new("all-yes")
            .long("yes")
            .short('y')
            .help("Answer yes to all queries asked")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("view")
            .long("view")
            .help("View image effects after carrying out effects")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("probe")
            .long("probe")
            .help("Probe file for details")
            .long_help("Probe files to extract information, this has the highest priority and overrides all the other options")
            .action(ArgAction::SetTrue))
        .arg(Arg::new("experimental")
            .long("experimental")
            .help("Support experimental image decoders in the command line")
            .action(ArgAction::SetTrue))
        .args(add_logging_options())
        .args(add_settings())
        .args(options_args)
        .group(option_group)
        .args(filter_args)
        .group(filter_group)
        .args(encode_args)
        .group(encode_group)
        .args(image_args)
        .group(image_args_group)
}

fn add_logging_options() -> [Arg; 4] {
    [
        Arg::new("debug")
            .long("debug")
            .action(ArgAction::SetTrue)
            .help_heading("Logging")
            .help("Display debug information and higher"),
        Arg::new("trace")
            .long("trace")
            .action(ArgAction::SetTrue)
            .help_heading("Logging")
            .help("Display very verbose information"),
        Arg::new("warn")
            .long("warn")
            .action(ArgAction::SetTrue)
            .help_heading("Logging")
            .help("Display warnings and errors"),
        Arg::new("info")
            .long("info")
            .action(ArgAction::SetTrue)
            .help_heading("Logging")
            .help("Display information about the decoding options")
    ]
}

fn add_settings() -> Vec<Arg> {
    const HELP_HEADING: &str = "Image Settings";
    let mut args = [
        Arg::new("colorspace")
            .long("colorspace")
            .help_heading(HELP_HEADING)
            .help("Change the image colorspace")
            .long_help(COLORSPACE_HELP)
            .value_parser(value_parser!(IColorSpace))
            .hide_possible_values(true),
        Arg::new("max-width")
            .long("max-width")
            .help_heading(HELP_HEADING)
            .help("Maximum width of images allowed")
            .default_value("37268")
            .value_parser(value_parser!(usize)),
        Arg::new("max-height")
            .long("max-height")
            .help_heading(HELP_HEADING)
            .help("Maximum height of images allowed")
            .default_value("37268")
            .value_parser(value_parser!(usize)),
        Arg::new("strict")
            .long("strict")
            .help_heading(HELP_HEADING)
            .help("Treat most warnings as errors")
            .action(ArgAction::SetTrue)
            .default_value("false"),
        Arg::new("safe")
            .long("safe")
            .help_heading(HELP_HEADING)
            .help("Do not use unsafe paths routines where possible")
            .long_help("Do not use unsafe paths routines where possible\nMainly used for debugging and testing purposes")
            .action(ArgAction::SetTrue)
            .default_value("false")
    ];
    // list them in order
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    args.to_vec()
}

fn add_operations() -> (Vec<Arg>, ArgGroup) {
    static HELP_HEADING: &str = "Image Operations";
    static GROUP: &str = "Operations";

    let mut args = [
        Arg::new("grayscale")
            .long("grayscale")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Convert the image to grayscale")
            .long_help("Change image type from RGB to grayscale")
            .group(GROUP),
        Arg::new("transpose")
            .long("transpose")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Transpose an image")
            .group(GROUP)
            .long_help(TRANSPOSE_HELP),
        Arg::new("flip")
            .long("flip")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Flip an image on the vertical axis")
            .group(GROUP),
        Arg::new("flop")
            .long("flop")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Flop an image")
            .group(GROUP),
        Arg::new("v-flip")
            .long("v-flip")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Flip an image on the vertical axis")
            .group(GROUP),
        Arg::new("mirror")
            .long("mirror")
            .help_heading(HELP_HEADING)
            .value_parser(["north", "south", "east", "west"])
            .help("Mirror the image")
            .group(GROUP),
        Arg::new("invert")
            .long("invert")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Invert image pixels")
            .group(GROUP),
        Arg::new("brighten")
            .long("brighten")
            .help_heading(HELP_HEADING)
            .help("Brighten (or darken) an image.")
            .long_help(BRIGHTEN_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("crop")
            .long("crop")
            .help_heading(HELP_HEADING)
            .value_names(["width", "height", "x", "y"])
            .help("Crop an image ")
            .long_help(CROP_HELP)
            .group(GROUP),
        Arg::new("threshold")
            .long("threshold")
            .value_names(["threshold", "mode"])
            .help_heading(HELP_HEADING)
            .help("Replace pixels in an image depending on intensity of the pixel.")
            .long_help(THRESHOLD_HELP)
            .group(GROUP),
        Arg::new("gamma")
            .long("gamma")
            .help("Gamma adjust an image")
            .help_heading(HELP_HEADING)
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("stretch_contrast")
            .long("stretch-contrast")
            .value_parser(value_parser!(u16))
            .value_names(["lower", "upper"])
            .help_heading(HELP_HEADING)
            .help("Linearly stretch contrast in an image")
            .group(GROUP),
        Arg::new("contrast")
            .long("contrast")
            .value_name("contrast")
            .help_heading(HELP_HEADING)
            .help("Adjust contrast of the image")
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("resize")
            .long("resize")
            .value_names(["width", "height"])
            .help_heading(HELP_HEADING)
            .value_parser(value_parser!(usize))
            .help("Resize an image")
            .group(GROUP),
        Arg::new("depth")
            .long("depth")
            .help_heading(HELP_HEADING)
            .help("Change image depth")
            .value_parser(value_parser!(u8))
            .group(GROUP),
        Arg::new("auto-orient")
            .long("auto-orient")
            .help_heading(HELP_HEADING)
            .help("Automatically orient the image based on exif tag")
            .action(ArgAction::SetTrue)
            .group(GROUP),
        Arg::new("exposure")
            .long("exposure")
            .help_heading(HELP_HEADING)
            .help("Adjust exposure of image, value is capped between -3 and 3")
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("huerotate")
            .long("huerotate")
            .help_heading(HELP_HEADING)
            .help("Hue rotate the image by certain degrees, (between 0 and 360)")
            .value_parser(value_parser!(f32)),
        Arg::new("saturate")
            .long("saturate")
            .help_heading(HELP_HEADING)
            .help("Adjust image saturation")
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32)),
        Arg::new("lightness")
            .long("lightness")
            .help_heading(HELP_HEADING)
            .allow_negative_numbers(true)
            .help("Adjust image brightness")
            .value_parser(value_parser!(f32))
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));

    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);

    (args.to_vec(), arg_group)
}

fn add_encode_options() -> (Vec<Arg>, ArgGroup) {
    static HELP_HEADING: &str = "Encode Operations";
    static GROUP: &str = "Encode operations";
    let mut args = [
        Arg::new("quality")
            .long("quality")
            .help("Encoding quality")
            .default_value("80")
            .value_name("quality")
            .help_heading(HELP_HEADING)
            .value_parser(value_parser!(u8))
            .group(GROUP),
        Arg::new("encode-threads")
            .long("encode-threads")
            .help("Number of threads to use when encoding")
            .default_value("4")
            .value_parser(value_parser!(u8))
            .group(GROUP)
            .help_heading(HELP_HEADING),
        Arg::new("effort")
            .long("effort")
            .default_value("4")
            .value_name("effort")
            .value_parser(value_parser!(u8))
            .help("Effort to put into encoding")
            .group(GROUP)
            .help_heading(HELP_HEADING),
        Arg::new("progressive")
            .long("progressive")
            .help("Encode images using progressive encoding where supported")
            .action(ArgAction::SetTrue)
            .group(GROUP)
            .help_heading(HELP_HEADING),
        Arg::new("strip")
            .long("strip")
            .help("Strip metadata when encoding images (where supported)")
            .action(ArgAction::SetTrue)
            .group(GROUP)
            .help_heading(HELP_HEADING)
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);
    (args.to_vec(), arg_group)
}

fn add_filters() -> (Vec<Arg>, ArgGroup) {
    static GROUP: &str = "filters";

    let mut args = [
        Arg::new("box-blur")
            .long("box-blur")
            .help("Perform a box blur")
            .value_name("radius")
            .long_help(BOX_BLUR_HELP)
            .help_heading(GROUP)
            .value_parser(value_parser!(usize))
            .group(GROUP),
        Arg::new("blur")
            .long("blur")
            .help("Perform a gaussian blur")
            .value_name("sigma")
            .long_help(GAUSSIAN_BLUR_HELP)
            .help_heading(GROUP)
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("unsharpen")
            .long("unsharpen")
            .help("Perform an unsharp mask")
            .help_heading(GROUP)
            .value_names(["sigma", "threshold"])
            .value_parser(value_parser!(f32))
            .group(GROUP),
        Arg::new("statistic")
            .long("statistic")
            .help("Replace each pixel with corresponding statistic from the neighbourhood")
            .help_heading(GROUP)
            .value_names(["radius", "statistic"])
            .group(GROUP),
        Arg::new("mean-blur")
            .long("mean-blur")
            .help("Perform a mean blur")
            .value_name("radius")
            .help_heading(GROUP)
            .value_parser(value_parser!(usize))
            .group(GROUP),
        Arg::new("sobel")
            .long("sobel")
            .help("Perform a 3x3 sobel convolution operation")
            .action(ArgAction::SetTrue)
            .help_heading(GROUP)
            .group(GROUP),
        Arg::new("scharr")
            .long("scharr")
            .help("Perform a 3x3 scharr convolution operation")
            .action(ArgAction::SetTrue)
            .help_heading(GROUP)
            .group(GROUP),
        Arg::new("convolve")
            .long("convolve")
            .allow_hyphen_values(true)
            .help("Perform a 2D NxN convolution. N can be either of 3,5 or 7")
            .group(GROUP)
            .help_heading(GROUP)
            .num_args(..=49)
            .action(ArgAction::Append)
            .value_parser(value_parser!(f32)),
        Arg::new("median-blur")
            .long("median-blur")
            .help("Perform a median blur on an image, this replaces a pixel with the median of it's neighbours")
            .value_name("radius")
            .help_heading(GROUP)
            .value_parser(value_parser!(usize))
            .group(GROUP)
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);

    (args.to_vec(), arg_group)
}

fn add_image_specific_settings() -> (Vec<Arg>, ArgGroup) {
    static GROUP: &str = "Image Format Settings";

    let mut args = [Arg::new("jpeg-grayscale")
        .long("jpeg-grayscale")
        .help("Load jpeg images as grayscale")
        .action(ArgAction::SetTrue)
        .help_heading(GROUP)
        .group(GROUP)];

    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);

    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));

    (args.to_vec(), arg_group)
}

#[test]
fn verify_cli() {
    create_cmd_args().debug_assert();
}
