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
pub(crate) enum MmapOptions
{
    No,
    Always,
    Auto
}

impl ValueEnum for MmapOptions
{
    fn value_variants<'a>() -> &'a [Self]
    {
        &[Self::No, Self::Auto, Self::Always]
    }

    fn to_possible_value(&self) -> Option<PossibleValue>
    {
        Some(match self
        {
            Self::No => PossibleValue::new("no"),
            Self::Always => PossibleValue::new("always"),
            Self::Auto => PossibleValue::new("auto")
        })
    }
}

pub(crate) struct CmdOptions
{
    mmap: MmapOptions
}
impl CmdOptions
{
    pub fn new() -> CmdOptions
    {
        CmdOptions {
            mmap: MmapOptions::Auto
        }
    }

    pub fn set_mmap_options(&mut self, mmap: MmapOptions)
    {
        self.mmap = mmap;
    }
    pub fn get_mmap_options(&self) -> MmapOptions
    {
        self.mmap
    }
}

fn get_long_version() -> &'static str
{
    let box_v = Box::new(format!(
        "Zune Version:{}\n\n\
        zune-jpeg Version :{}\n\
        zune-jpeg Git hash:{}\n",
        env!("CARGO_PKG_VERSION"),
        zune_image::codecs::jpeg::get_version(),
        zune_image::codecs::jpeg::get_git_hash(),
    ));

    Box::leak(box_v)
}

#[rustfmt::skip]
pub fn create_cmd_args() -> Command {
    Command::new("zune")
        .after_help(AFTER_HELP)
        .author("Caleb Etemesi")
        .version("0.1.1")
        .long_version(get_long_version())
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
            .required(true))
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
        .args(add_logging_options())
        .args(add_operations())
        .args(add_settings())
        .args(add_filters())
        .group(ArgGroup::new("operations")
            .args(["flip","transpose","grayscale","flop","mirror","invert","brighten","crop","threshold","gamma"])
            .multiple(true))
        .group(ArgGroup::new("filters")
            .args(["box-blur", "blur", "unsharpen", "median", "erode"])
            .multiple(true))
}

fn add_logging_options() -> [Arg; 4]
{
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
fn add_settings() -> Vec<Arg>
{
    let mut args = [
        Arg::new("colorspace")
            .long("colorspace")
            .help_heading("Image Settings")
            .help("Change the image colorspace during decoding")
            .long_help(COLORSPACE_HELP)
            .value_parser(value_parser!(IColorSpace))
            .hide_possible_values(true),
        Arg::new("max-width")
            .long("max-width")
            .help_heading("Image Settings")
            .help("Maximum width of images allowed")
            .default_value("37268")
            .value_parser(value_parser!(usize)),
        Arg::new("max-height")
            .long("max-height")
            .help_heading("Image Settings")
            .help("Maximum height of images allowed")
            .default_value("37268")
            .value_parser(value_parser!(usize)),
        Arg::new("strict")
            .long("strict")
            .help_heading("Image Settings")
            .help("Treat most warnings as errors")
            .action(ArgAction::SetTrue)
            .default_value("false")
    ];
    // list them in order
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));

    args.to_vec()
}
fn add_operations() -> Vec<Arg>
{
    static HELP_HEADING: &str = "Image Operations";

    let mut args = [
        Arg::new("grayscale")
            .long("grayscale")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Convert the image to grayscale")
            .long_help("Change image type from RGB to grayscale")
            .group("operations"),
        Arg::new("transpose")
            .long("transpose")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Transpose an image")
            .group("operations")
            .long_help(TRANSPOSE_HELP),
        Arg::new("flip")
            .long("flip")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Flip an image")
            .group("operations"),
        Arg::new("flop")
            .long("flop")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Flop an image")
            .group("operations"),
        Arg::new("mirror")
            .long("mirror")
            .help_heading(HELP_HEADING)
            .value_parser(["north", "south", "east", "west"])
            .help("Mirror the image")
            .group("operations"),
        Arg::new("invert")
            .long("invert")
            .help_heading(HELP_HEADING)
            .action(ArgAction::SetTrue)
            .help("Invert image pixels")
            .group("operations"),
        Arg::new("brighten")
            .long("brighten")
            .help_heading(HELP_HEADING)
            .help("Brighten (or darken) an image.")
            .long_help(BRIGHTEN_HELP)
            .allow_negative_numbers(true)
            .value_parser(value_parser!(i16).range(-255..=255))
            .group("operations"),
        Arg::new("crop")
            .long("crop")
            .help_heading(HELP_HEADING)
            .value_name("out_w:out_h:x:y")
            .help("Crop an image ")
            .long_help(CROP_HELP)
            .group("operations"),
        Arg::new("threshold")
            .long("threshold")
            .value_name("threshold:mode")
            .help_heading(HELP_HEADING)
            .help("Replace pixels in an image depending on intensity of the pixel.")
            .long_help(THRESHOLD_HELP)
            .group("operations"),
        Arg::new("gamma")
            .long("gamma")
            .help("Gamma adjust an image")
            .help_heading(HELP_HEADING)
            .value_parser(value_parser!(f32))
            .group("operations")
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    args.to_vec()
}
fn add_filters() -> Vec<Arg>
{
    let mut args = [
        Arg::new("box-blur")
            .long("box-blur")
            .help("Perform a box blur")
            .value_name("radius")
            .long_help(BOX_BLUR_HELP)
            .help_heading("Filters")
            .value_parser(value_parser!(usize))
            .group("filters"),
        Arg::new("blur")
            .long("blur")
            .help("Perform a gaussian blur")
            .value_name("sigma")
            .long_help(GAUSSIAN_BLUR_HELP)
            .help_heading("Filters")
            .value_parser(value_parser!(f32))
            .group("filters"),
        Arg::new("unsharpen")
            .long("unsharpen")
            .help("Perform an unsharp mask")
            .help_heading("Filters")
            .value_name("sigma:threshold:percentage")
            .group("filters"),
        Arg::new("statistic")
            .long("statistic")
            .help("Replace each pixel with corresponding statistic from the neighbourhood")
            .help_heading("Filters")
            .value_name("radius:statistic")
    ];
    args.sort_unstable_by(|x, y| x.get_id().cmp(y.get_id()));
    args.to_vec()
}

#[test]
fn verify_cli()
{
    create_cmd_args().debug_assert();
}
