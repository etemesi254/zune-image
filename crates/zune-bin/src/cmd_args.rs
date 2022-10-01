pub(crate) mod jpeg;

use clap::builder::PossibleValue;
use clap::{value_parser, Arg, ArgAction, Command, ValueEnum};

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
            .required(true))
        .arg(Arg::new("out")
            .short('o')
            .help("Output to write the data to")
            .action(ArgAction::Append)
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
        )
        .arg(Arg::new("grayscale")
            .long("grayscale")
            .help_heading("OPERATIONS")
            .action(ArgAction::SetTrue)
            .help("Convert the image to grayscale")
            .long_help("Change image type from RGB to grayscale")
            .group("operations")
            .group("deinterleave"))
}
