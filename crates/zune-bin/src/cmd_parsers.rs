use clap::ArgMatches;

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
