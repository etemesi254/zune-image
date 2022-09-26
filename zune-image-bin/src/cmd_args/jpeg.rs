use std::num::NonZeroU32;

use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueEnum};
use log::trace;
use zune_jpeg::{ColorSpace, ZuneJpegOptions};

#[derive(Copy, Clone, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum IColorSpace
{
    RGB,
    GRAYSCALE,
    YCbCr,
    RGBA,
    RGBX,
}
impl IColorSpace
{
    const fn to_jpeg_colorspace(self) -> ColorSpace
    {
        match self
        {
            IColorSpace::RGB => ColorSpace::RGB,
            IColorSpace::GRAYSCALE => ColorSpace::GRAYSCALE,
            IColorSpace::YCbCr => ColorSpace::YCbCr,
            IColorSpace::RGBA => ColorSpace::RGBA,
            IColorSpace::RGBX => ColorSpace::RGBX,
        }
    }
}
const SHORT_ABOUT: &str = "JPEG decoder options";

const LONG_ABOUT: &str = "JPEG decoder options

You can set decoder options by passing them into this commands";

impl ValueEnum for IColorSpace
{
    fn value_variants<'a>() -> &'a [Self]
    {
        &[
            Self::RGBX,
            Self::RGBA,
            Self::RGB,
            Self::YCbCr,
            Self::GRAYSCALE,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue>
    {
        Some(match self
        {
            Self::RGBX => PossibleValue::new("rgbx"),
            Self::RGBA => PossibleValue::new("rgba"),
            Self::RGB => PossibleValue::new("rgb"),
            Self::YCbCr => PossibleValue::new("ycbcr"),
            Self::GRAYSCALE => PossibleValue::new("grayscale"),
        })
    }
}
impl std::str::FromStr for IColorSpace
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        for variant in Self::value_variants()
        {
            if variant.to_possible_value().unwrap().matches(s, false)
            {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {}", s))
    }
}

pub fn parse_options(options: &ArgMatches) -> ZuneJpegOptions
{
    let decoder_options;
    if let Some(arguments) = options.subcommand_matches("jpeg-decoder")
    {
        trace!("Parsing jpeg options");
        let use_unsafe = arguments.contains_id("unsafe");
        let strict_mode = arguments.contains_id("strict");

        let max_height = *arguments.get_one::<u16>("max-height").unwrap();
        let max_width = *arguments.get_one::<u16>("max-width").unwrap();
        let colorspace = *arguments.get_one::<IColorSpace>("colorspace").unwrap();
        let threads =
            NonZeroU32::new(u32::from(*arguments.get_one::<u16>("threads").unwrap())).unwrap();

        let z_colorspace = colorspace.to_jpeg_colorspace();

        decoder_options = ZuneJpegOptions::new()
            .set_use_unsafe(use_unsafe)
            .set_out_colorspace(z_colorspace)
            .set_max_height(max_height)
            .set_max_width(max_width)
            .set_num_threads(threads)
            .set_strict_mode(strict_mode);
    }
    else
    {
        decoder_options = ZuneJpegOptions::default();
    }
    decoder_options
}

#[rustfmt::skip]
pub fn options() -> Command {
    let b_long_version = Box::new(zune_jpeg::get_version().to_string() + " commit:"+zune_jpeg::get_git_hash());
    let long_version:&'static str = Box::leak(b_long_version.into_boxed_str());
    return Command::new("jpeg-decoder")
        .alias("jpg-decoder")
        .about(SHORT_ABOUT)
        .long_about(LONG_ABOUT)
        .version(zune_jpeg::get_version())
        .args_conflicts_with_subcommands(true)
        .long_version(long_version)
        .arg(
            Arg::new("threads")
                .long("threads")
                .value_parser(value_parser!(u16).range(1..))
                .default_value("4")
                .help("Number of threads to spawn to decode a single image")
                
                //(true)
        ).arg(
        Arg::new("strict")
            .long("strict")
            .action(ArgAction::SetTrue)
            .help("Treat some warnings as errors")
           // .takes_value(false)
    ).arg(
        Arg::new("colorspace")
            .long("colorspace")
            .help("Set the output colorspace")
         //   .takes_value(true)
            .default_value("rgb")
            .value_parser(value_parser!(IColorSpace))
    ).arg(
        Arg::new("unsafe")
            .long("unsafe")
            .action(ArgAction::SetTrue)
            .help("Use unsafe platform specific intrinsics")
          //  .takes_value(false)
    ).arg(
        Arg::new("max-width")
            .long("max-width")
            .help("Maximum width of images allowed")
            .default_value("32768")
            .value_parser(value_parser!(u16))
    ).arg(
        Arg::new("max-height")
            .long("max-height")
            .help("Maximum height of images allowed")
            .default_value("32768")
            .value_parser(value_parser!(u16))
    );
}
