pub(crate) mod jpeg;

use std::ffi::OsString;

use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, ArgGroup, Command, ValueEnum};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum MmapOptions
{
    No,
    Always,
    Auto,
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
            Self::Auto => PossibleValue::new("auto"),
        })
    }
}

pub(crate) struct CmdOptions
{
    mmap: MmapOptions,
}
impl CmdOptions
{
    pub fn new() -> CmdOptions
    {
        CmdOptions {
            mmap: MmapOptions::Auto,
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

#[rustfmt::skip]
pub fn create_cmd_args() -> Command {
    Command::new("zune-images")
        .subcommand(crate::cmd_args::jpeg::options())
        .subcommand_precedence_over_arg(true)
        .arg(Arg::new("in")
            .short('i')
            .help("Input file to read data from")
            .long("input")
            .action(ArgAction::Append)
            .value_parser(value_parser!(OsString))
            .required(true))
        .arg(Arg::new("out")
            .short('o')
            .help("Output to write the data to")
            .action(ArgAction::Append)
            .value_parser(value_parser!(OsString))
            .required(true))
        .arg(Arg::new("debug")
            .long("debug")
            .action(ArgAction::SetTrue)
            .help_heading("LOGGING")
            .help("Display debug information and higher"))
        .arg(Arg::new("trace")
            .long("trace")
            .action(ArgAction::SetTrue)
            .help_heading("LOGGING")
            .help("Display very verbose information"))
        .arg(Arg::new("warn")
            .long("warn")
            .action(ArgAction::SetTrue)
            .help_heading("LOGGING")
            .help("Display warnings and errors"))
        .arg(Arg::new("info")
            .long("info")
            .action(ArgAction::SetTrue)
            .help_heading("LOGGING")
            .help("Display information about the decoding options"))
        .arg(Arg::new("mmap")
            .long("mmap")
            .help_heading("ADVANCED")
            //.takes_value(true)
            .help("Influence the use of memory maps")
            .long_help("Change use of memory maps and how they are used for decoding.\nMemory maps are preferred for large images to keep memory usage low.")
            .value_parser(value_parser!(MmapOptions))
        ).arg(Arg::new("all-yes")
            .long("yes")
            .help("Answer yes to all queries asked")
            .action(ArgAction::SetTrue)
        )
        .args(add_operations())
        .group(ArgGroup::new("operations")
            .args(["flip","transpose","grayscale","flop"])
            .multiple(true))
}
fn add_operations() -> [Arg; 4]
{
    [
        Arg::new("grayscale")
            .long("grayscale")
            .help_heading("OPERATIONS")
            .action(ArgAction::SetTrue)
            .help("Convert the image to grayscale")
            .long_help("Change image type from RGB to grayscale")
            .group("operations"),

        Arg::new("transpose")
            .long("transpose")
            .help_heading("OPERATIONS")
            .action(ArgAction::SetTrue)
            .help("Transpose an image")
            .group("operations")
            .long_help("Transpose an image\nThis mirrors the image along the image top left to bottom-right diagonal"),

        Arg::new("flip")
            .long("flip")
            .help_heading("OPERATIONS")
            .action(ArgAction::SetTrue)
            .help("Flip an image")
            .group("operations"),

        Arg::new("flop")
            .long("flop")
            .help_heading("OPERATIONS")
            .action(ArgAction::SetTrue)
            .help("Flop an image")
            .group("operations")
    ]
}

#[test]
fn verify_cli()
{
    create_cmd_args().debug_assert();
}
