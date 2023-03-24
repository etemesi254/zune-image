use clap::ArgMatches;
use zune_core::options::DecoderOptions;

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
    let use_unsafe = !*options.get_one::<bool>("safe").unwrap();
    let strict_mode = *options.get_one::<bool>("strict").unwrap();

    DecoderOptions::new_cmd()
        .set_max_height(max_height)
        .set_max_width(max_width)
        .set_use_unsafe(use_unsafe)
        .set_strict_mode(strict_mode)
}
