/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use clap::parser::ValueSource;
use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use log::{info, Level};

use crate::cmd_args::MmapOptions;

#[derive(Debug, Copy, Clone)]
pub struct CmdOptions {
    pub mmap:                 MmapOptions,
    pub max_width:            usize,
    pub max_height:           usize,
    pub strict_mode:          bool,
    pub override_files:       bool,
    pub experimental_formats: bool
}

impl CmdOptions {
    pub fn new() -> CmdOptions {
        CmdOptions {
            mmap:                 MmapOptions::No,
            max_width:            0,
            max_height:           0,
            strict_mode:          false,
            override_files:       false,
            experimental_formats: false
        }
    }
}

pub fn parse_options(options: &ArgMatches) -> CmdOptions {
    let mut cmd_options = CmdOptions::new();

    if let Some(mmap_opt) = options.value_source("mmap") {
        if mmap_opt == CommandLine {
            info!("Mmap option present");
            let mmap = *options.get_one::<MmapOptions>("mmap").unwrap();
            info!("Setting mmap to be {:?}", mmap);
            cmd_options.mmap = mmap;
        }
    }
    let width = *options.get_one::<usize>("max-width").unwrap();
    let height = *options.get_one::<usize>("max-height").unwrap();

    cmd_options.max_width = width;
    cmd_options.max_height = height;

    if options.value_source("all-yes") == Some(ValueSource::CommandLine) {
        info!("Setting all commands to yes");
        cmd_options.override_files = true;
    }

    if options.value_source("experimental") == Some(ValueSource::CommandLine) {
        info!("Allowing experimental image decoding");
        cmd_options.experimental_formats = true;
    }
    cmd_options
}

/// Set up logging options
pub fn setup_logger(options: &ArgMatches) {
    let log_level;

    if *options.get_one::<bool>("debug").unwrap() {
        log_level = Level::Debug;
    } else if *options.get_one::<bool>("trace").unwrap() {
        log_level = Level::Trace;
    } else if *options.get_one::<bool>("warn").unwrap() {
        log_level = Level::Warn
    } else if *options.get_one::<bool>("info").unwrap() {
        log_level = Level::Info;
    } else {
        log_level = Level::Warn;
    }

    simple_logger::init_with_level(log_level).unwrap();

    info!("Initialized logger");
    info!("Log level :{}", log_level);
}
