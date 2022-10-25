use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use log::{error, info, Level};

use crate::cmd_args::{CmdOptions, MmapOptions};
use crate::workflow::create_and_exec_workflow_from_cmd;

mod cmd_args;
mod workflow;

fn parse_options(options: &ArgMatches) -> CmdOptions
{
    let mut cmd_options = CmdOptions::new();

    if let Some(mmap_opt) = options.value_source("mmap")
    {
        if mmap_opt == CommandLine
        {
            info!("Mmap option present");
            let mmap = *options.get_one::<MmapOptions>("mmap").unwrap();
            info!("Setting mmap to be {:?}", mmap);
            cmd_options.set_mmap_options(mmap);
        }
    }

    cmd_options
}
fn setup_logger(options: &ArgMatches)
{
    let log_level;

    if *options.get_one::<bool>("debug").unwrap()
    {
        log_level = Level::Debug;
    }
    else if *options.get_one::<bool>("trace").unwrap()
    {
        log_level = Level::Trace;
    }
    else if *options.get_one::<bool>("warn").unwrap()
    {
        log_level = Level::Warn
    }
    else if *options.get_one::<bool>("info").unwrap()
    {
        log_level = Level::Info;
    }
    else
    {
        log_level = Level::Error;
    }

    simple_logger::init_with_level(log_level).unwrap();

    info!("Initialized logger");
    info!("Log level :{}", log_level);
}
pub fn main() -> i32
{
    let cmd = cmd_args::create_cmd_args();
    let options = cmd.get_matches();

    setup_logger(&options);

    let parsed_opts = parse_options(&options);
    let result = create_and_exec_workflow_from_cmd(&options, &parsed_opts);

    if result.is_err()
    {
        println!();
        error!(
            " Could not complete workflow, reason {:?}",
            result.err().unwrap()
        );
        println!();
        return 1;
    }
    0
}
