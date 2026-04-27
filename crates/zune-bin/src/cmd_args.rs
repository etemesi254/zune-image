/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::ffi::OsString;

use clap::builder::{PossibleValue, PossibleValuesParser};
use clap::{value_parser, Arg, ArgAction, ArgGroup, Command, ValueEnum};
use zune_image::codecs::ImageFormat;

use crate::cmd_args::arg_parsers::{IColorSpace, IResizeMethod};
use crate::cmd_args::help_strings::{
    AFFINE_TRANSFORM_HELP, AFTER_HELP, APPEND_HELP, AUTO_ORIENT_HELP, AVERAGE_HELP,
    BILATERAL_FILTER_HELP, BLEND_HELP, BOX_BLUR_HELP, BRIGHTEN_HELP, COLORSPACE_HELP,
    COLOR_TRANSFORM_HELP, COMPOSITE_HELP, CONTRAST_HELP, CONVOLVE_HELP, CROP_HELP, DEPTH_HELP,
    EFFORT_HELP, ENCODE_THREADS_HELP, EXPOSURE_HELP, FLIP_HELP, FLOP_HELP, GAMMA_HELP,
    GAUSSIAN_BLUR_HELP, GRAYSCALE_HELP, HALD_CLUT, HUEROTATE_HELP, INVERT_HELP, LIGHTNESS_HELP,
    MEAN_BLUR_HELP, MEDIAN_BLUR_HELP, MIRROR_HELP, PROGRESSIVE_HELP, QUALITY_HELP, RESIZE_HELP,
    RESIZE_METHOD_HELP, ROTATE_HELP, SATURATE_HELP, SCHARR_HELP, SOBEL_HELP, SSIM_HELP,
    STATISTIC_HELP, STRETCH_CONTRAST_HELP, STRIP_HELP, THRESHOLD_HELP, TRANSPOSE_HELP,
    UNSHARPEN_HELP, V_FLIP_HELP,
};

pub mod arg_parsers;
pub mod help_strings;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MmapOptions {
    No,
    Always,
    Auto,
}

impl ValueEnum for MmapOptions {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::No, Self::Auto, Self::Always]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        Some(match self {
            Self::No => PossibleValue::new("no"),
            Self::Always => PossibleValue::new("always"),
            Self::Auto => PossibleValue::new("auto"),
        })
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CmdImageFormats {
    Format(ImageFormat),
}
impl ValueEnum for CmdImageFormats {
    fn value_variants<'a>() -> &'a [Self] {
        &[
            Self::Format(ImageFormat::BMP),
            Self::Format(ImageFormat::JPEG),
            Self::Format(ImageFormat::JPEG_XL),
            Self::Format(ImageFormat::HDR),
            Self::Format(ImageFormat::Farbfeld),
            Self::Format(ImageFormat::PNG),
            Self::Format(ImageFormat::PPM),
            Self::Format(ImageFormat::PSD),
            Self::Format(ImageFormat::QOI),
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            CmdImageFormats::Format(c) => match c {
                ImageFormat::JPEG => Some(PossibleValue::new("jpeg")),
                ImageFormat::PNG => Some(PossibleValue::new("png")),
                ImageFormat::PPM => Some(PossibleValue::new("ppm")),
                ImageFormat::PSD => Some(PossibleValue::new("psd")),
                ImageFormat::Farbfeld => Some(PossibleValue::new("farbfeld")),
                ImageFormat::QOI => Some(PossibleValue::new("qoi")),
                ImageFormat::JPEG_XL => Some(PossibleValue::new("jxl")),
                ImageFormat::HDR => Some(PossibleValue::new("hdr")),
                ImageFormat::BMP => Some(PossibleValue::new("bmp")),
                _ => None,
            },
        }
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
            .action(ArgAction::Append)
            .value_parser(value_parser!(OsString))
            .required(true))
        .arg(Arg::new("out")
            .short('o')
            .long("out")
            .help("Output to write the data to")
            .action(ArgAction::Append)
            .value_parser(value_parser!(OsString))
        )
        .arg(Arg::new("output-format")
            .long("output-format")
            .help("Output format to use when output is command line, to be used in conjunction with '-o -'")
            .value_parser(value_parser!(CmdImageFormats)))
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

fn add_logging_options() -> [Arg; 5] {
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
            .help("Display information about the decoding options"),
        Arg::new("no-log")
            .long("no-log")
            .action(ArgAction::SetTrue)
            .help_heading("Logging")
            .help("No Logging, do not log anything"),
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
            .action(ArgAction::SetTrue)
            .help("Convert the image to grayscale")
            .long_help(GRAYSCALE_HELP),
        Arg::new("transpose")
            .long("transpose")
            .action(ArgAction::SetTrue)
            .help("Transpose an image")
            .long_help(TRANSPOSE_HELP),
        Arg::new("flip")
            .long("flip")
            .action(ArgAction::SetTrue)
            .help("Flip an image on the vertical axis")
            .long_help(FLIP_HELP),
        Arg::new("flop")
            .long("flop")
            .action(ArgAction::SetTrue)
            .help("Flop an image")
            .long_help(FLOP_HELP),
        Arg::new("v-flip")
            .long("v-flip")
            .action(ArgAction::SetTrue)
            .help("Flip an image on the vertical axis")
            .long_help(V_FLIP_HELP),
        Arg::new("mirror")
            .long("mirror")
            .value_parser(["north", "south", "east", "west"])
            .help("Mirror the image")
            .long_help(MIRROR_HELP),
        Arg::new("invert")
            .long("invert")
            .action(ArgAction::SetTrue)
            .help("Invert image pixels")
            .long_help(INVERT_HELP),
        Arg::new("brighten")
            .long("brighten")
            .help("Brighten (or darken) an image.")
            .long_help(BRIGHTEN_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32)),
        Arg::new("crop")
            .long("crop")
            .value_names(["width", "height", "x", "y"])
            .help("Crop an image ")
            .long_help(CROP_HELP),
        Arg::new("threshold")
            .long("threshold")
            .value_names(["threshold", "mode"])
            .help("Replace pixels in an image depending on intensity of the pixel.")
            .long_help(THRESHOLD_HELP),
        Arg::new("gamma")
            .long("gamma")
            .help("Gamma adjust an image")
            .long_help(GAMMA_HELP)
            .value_parser(value_parser!(f32)),
        Arg::new("stretch-contrast")
            .long("stretch-contrast")
            .value_parser(value_parser!(f32))
            .value_names(["lower", "upper"])
            .help("Linearly stretch contrast in an image")
            .long_help(STRETCH_CONTRAST_HELP),
        Arg::new("contrast")
            .long("contrast")
            .value_name("contrast")
            .help("Adjust contrast of the image")
            .long_help(CONTRAST_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32)),
        Arg::new("resize")
            .long("resize")
            .value_names(["value"])
            .long_help(RESIZE_HELP)
            .help("Resize an image (e.g., 800x600, 50%, or 50%x75%)"),
        Arg::new("resize-method")
            .long("resize-method")
            .value_parser(value_parser!(IResizeMethod))
            .help("Resizing method to use")
            .long_help(RESIZE_METHOD_HELP),
        Arg::new("depth")
            .long("depth")
            .help("Change image depth")
            .long_help(DEPTH_HELP)
            .value_parser(value_parser!(u8)),
        Arg::new("auto-orient")
            .long("auto-orient")
            .help("Automatically orient the image based on exif tag")
            .long_help(AUTO_ORIENT_HELP)
            .action(ArgAction::SetTrue),
        Arg::new("exposure")
            .long("exposure")
            .help("Adjust exposure of image, value is capped between -3 and 3")
            .long_help(EXPOSURE_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32)),
        Arg::new("huerotate")
            .long("huerotate")
            .help("Hue rotate the image by certain degrees, (between 0 and 360)")
            .long_help(HUEROTATE_HELP)
            .value_parser(value_parser!(f32)),
        Arg::new("saturate")
            .long("saturate")
            .help("Adjust image saturation")
            .long_help(SATURATE_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(f32)),
        Arg::new("lightness")
            .long("lightness")
            .allow_negative_numbers(true)
            .help("Adjust image brightness")
            .long_help(LIGHTNESS_HELP)
            .value_parser(value_parser!(f32)),
        Arg::new("rotate")
            .long("rotate")
            .allow_negative_numbers(false)
            .help("Rotate image by 90, 180 or 270")
            .long_help(ROTATE_HELP)
            .value_parser(value_parser!(f32)),
        Arg::new("composite")
            .long("composite")
            .help("Composite the last two loaded images. Usage: --composite <Method>")
            .action(ArgAction::Set)
            .long_help(COMPOSITE_HELP)
            .value_parser([
                "Over", "Src", "Dst", "DstIn", "DstOut", "SrcIn", "SrcOut", "Xor", "Multiply",
                "Screen",
            ]),
        Arg::new("geometry")
            .long("geometry")
            .help("Position for composite (e.g., 100,50)")
            .action(ArgAction::Set)
            .requires("composite"),
        Arg::new("blend")
            .long("blend")
            .help("Blend the last two loaded images together using an alpha value (0.0 to 1.0).")
            .long_help(BLEND_HELP)
            .action(ArgAction::Set)
            .value_parser(clap::value_parser!(f32)),
        Arg::new("append")
            .long("append")
            .help("Append the loaded images (horizontal or vertical).")
            .long_help(APPEND_HELP)
            .action(ArgAction::Set)
            .value_parser(["horizontal", "vertical"]),
        Arg::new("ssim")
            .long("ssim")
            .help("Calculate the SSIM metric between the last two loaded images.")
            .long_help(SSIM_HELP)
            .action(ArgAction::SetTrue),
        Arg::new("average")
            .long("average")
            .help("Average all currently loaded images into a single image.")
            .long_help(AVERAGE_HELP)
            .action(ArgAction::SetTrue),
        Arg::new("swap")
            .long("swap")
            .help("Swap two images in the stack")
            .action(ArgAction::Append)
            .value_names(["a","b"])
            .value_parser(value_parser!(usize)),
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));

    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);

    (
        args.map(|f| f.help_heading(HELP_HEADING).group(GROUP))
            .to_vec(),
        arg_group,
    )
}

fn add_encode_options() -> (Vec<Arg>, ArgGroup) {
    static HELP_HEADING: &str = "Encode Operations";
    static GROUP: &str = "Encode operations";
    let mut args = [
        Arg::new("quality")
            .long("quality")
            .help("Encoding quality")
            .long_help(QUALITY_HELP)
            .default_value("80")
            .value_name("quality")
            .help_heading(HELP_HEADING)
            .value_parser(value_parser!(u8))
            .group(GROUP),
        Arg::new("encode-threads")
            .long("encode-threads")
            .help("Number of threads to use when encoding")
            .long_help(ENCODE_THREADS_HELP)
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
            .long_help(EFFORT_HELP)
            .group(GROUP)
            .help_heading(HELP_HEADING),
        Arg::new("progressive")
            .long("progressive")
            .help("Encode images using progressive encoding where supported")
            .long_help(PROGRESSIVE_HELP)
            .action(ArgAction::SetTrue)
            .group(GROUP)
            .help_heading(HELP_HEADING),
        Arg::new("strip")
            .long("strip")
            .help("Strip metadata when encoding images (where supported)")
            .long_help(STRIP_HELP)
            .action(ArgAction::SetTrue)
            .group(GROUP)
            .help_heading(HELP_HEADING),
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
            .value_parser(value_parser!(usize)),
        Arg::new("blur")
            .long("blur")
            .help("Perform a gaussian blur")
            .value_name("sigma")
            .long_help(GAUSSIAN_BLUR_HELP)
            .value_parser(value_parser!(f32)),
        Arg::new("sharpen")
            .long("sharpen")
            .help("Perform an unsharp mask")
            .long_help(UNSHARPEN_HELP)
            .value_names(["sigma", "threshold", "percentage"])
            .value_parser(value_parser!(f32)),
        Arg::new("statistic")
            .long("statistic")
            .help("Replace each pixel with corresponding statistic from the neighbourhood")
            .long_help(STATISTIC_HELP)
            .value_names(["radius", "statistic"]),
        Arg::new("mean-blur")
            .long("mean-blur")
            .help("Perform a mean blur")
            .long_help(MEAN_BLUR_HELP)
            .value_name("radius")
            .value_parser(value_parser!(usize)),
        Arg::new("sobel")
            .long("sobel")
            .help("Perform a 3x3 sobel convolution operation")
            .long_help(SOBEL_HELP)
            .action(ArgAction::SetTrue),
        Arg::new("scharr")
            .long("scharr")
            .help("Perform a 3x3 scharr convolution operation")
            .long_help(SCHARR_HELP)
            .action(ArgAction::SetTrue),
        Arg::new("convolve")
            .long("convolve")
            .allow_hyphen_values(true)
            .help("Perform a 2D NxN convolution. N can be either of 3, 5 or 7")
            .long_help(CONVOLVE_HELP)
            .num_args(..=49)
            .action(ArgAction::Append)
            .value_parser(value_parser!(f32)),
        Arg::new("median-blur")
            .long("median-blur")
            .help("Perform a median blur on an image, this replaces a pixel with the median of it's neighbours")
            .long_help(MEDIAN_BLUR_HELP)
            .value_name("radius")
            .value_parser(value_parser!(usize)),
        Arg::new("color-transform")
            .long("color-transform")
            .help("Parse the ICC chunk of an image and perform a color transform")
            .long_help(COLOR_TRANSFORM_HELP)
            .default_missing_value("rgb")
            .value_parser(PossibleValuesParser::new(["rgb", "adobe-rgb", "display-p3", "bt-2020"]))
            .value_name("color-transform"),
        Arg::new("affine-transform")
            .long("affine-transform")
            .allow_hyphen_values(true)
            .value_names(["a", "b", "c", "d", "tx", "ty"])
            .value_parser(value_parser!(f32))
            .help("Affine transform an image")
            .long_help(AFFINE_TRANSFORM_HELP),
        Arg::new("bilateral")
            .long("bilateral")
            .value_name("D,COLOR,SPACE")
            .help("Apply a bilateral filter/edge smoothing (e.g., '9,75.0,75.0')")
            .long_help(BILATERAL_FILTER_HELP),
        Arg::new("hald-clut")
            .long("hald-clut")
            .help("Apply a Hald-CLUT color grade using the last two loaded images.")
            .long_help(HALD_CLUT)
            .action(ArgAction::SetTrue)
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    let arg_group = ArgGroup::new(GROUP)
        .args(args.iter().map(|x| x.get_id()))
        .multiple(true);

    (
        args.map(|f| f.help_heading(GROUP).group(GROUP)).to_vec(),
        arg_group,
    )
}

fn add_image_specific_settings() -> (Vec<Arg>, ArgGroup) {
    static GROUP: &str = "Image Decoder Knobs";

    let mut args = [
        Arg::new("jpeg-grayscale")
            .long("jpeg-grayscale")
            .help("Load jpeg images as grayscale")
            .action(ArgAction::SetTrue)
            .help_heading(GROUP)
            .group(GROUP),
        Arg::new("hevc-software-decode")
            .long("hevc-software-decode")
            .help("Use zune-heic software decoder instead of apple-videotoolbox decoder on macos")
            .action(ArgAction::SetTrue)
            .help_heading(GROUP)
            .group(GROUP),
        Arg::new("png-ignore-crc")
            .long("png-ignore-crc")
            .help("Ignore CRC errors on decoding PNG images")
            .action(ArgAction::SetTrue)
            .help_heading(GROUP)
            .group(GROUP),
    ];

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
