use clap::ArgMatches;
use zune_core::options::DecoderOptions;

use crate::cmd_args::arg_parsers::IColorSpace;

pub mod global_options;

pub mod filters;
pub mod operations;

/// Fill arguments into a Vec according to the
/// order which they were specified in the command line
pub fn fill_args(options: &ArgMatches) -> Vec<String>
{
    let mut map = Vec::with_capacity(20);

    for (_pos, id) in options.ids().enumerate()
    {
        if options.try_get_many::<clap::Id>(id.as_str()).is_ok()
        {
            // ignore groups
            continue;
        }

        let value_source = options
            .value_source(id.as_str())
            .expect("id came from matches");

        if value_source != clap::parser::ValueSource::CommandLine
        {
            // ignore things not passed via command line
            continue;
        }
        let argument = id.to_string();
        map.push(argument)
    }
    map
}

pub fn get_decoder_options(options: &ArgMatches) -> DecoderOptions
{
    let max_width = *options.get_one::<usize>("max-width").unwrap();
    let max_height = *options.get_one::<usize>("max-height").unwrap();
    let use_unsafe = *options.get_one::<bool>("use-unsafe").unwrap();
    let strict_mode = *options.get_one::<bool>("strict").unwrap();
    let out_colorspace = options
        .get_one::<IColorSpace>("colorspace")
        .unwrap()
        .to_colorspace();

    DecoderOptions {
        max_width,
        max_height,
        use_unsafe,
        strict_mode,
        out_colorspace,
        ..Default::default()
    }
}