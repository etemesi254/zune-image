use std::process::exit;

use log::error;

use crate::cmd_args::MmapOptions;
use crate::workflow::create_and_exec_workflow_from_cmd;

mod cmd_args;
mod cmd_parsers;
mod file_io;
mod probe_files;
mod serde;
mod show_gui;
mod workflow;

pub fn main()
{
    let cmd = cmd_args::create_cmd_args();
    let options = cmd.get_matches();

    cmd_parsers::global_options::setup_logger(&options);

    let parsed_opts = cmd_parsers::global_options::parse_options(&options);

    let args = crate::cmd_parsers::fill_args(&options);

    let result = create_and_exec_workflow_from_cmd(&options, &args, &parsed_opts);

    if result.is_err()
    {
        println!();
        error!(
            " Could not complete workflow, reason {:?}",
            result.err().unwrap()
        );

        println!();
        exit(-1);
    }
}
