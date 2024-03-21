/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use zune_core::options::DecoderOptions;

use crate::serde::Metadata;

/// Probe input files, extract metadata, and print to standard output.
pub fn probe_input_files(args: &ArgMatches) {
    if let Some(view) = args.value_source("probe") {
        if view == CommandLine {
            for in_file in args.get_raw("in").unwrap() {
                if PathBuf::from(in_file).exists() {
                    let file = BufReader::new(File::open(in_file).unwrap());

                    if let Some((format, contents)) =
                        zune_image::codecs::ImageFormat::guess_format(file)
                    {
                        let size = contents.get_ref().metadata().unwrap().len();
                        // set to high to remove restrictions.
                        // We'll just be reading headers so it doesn't matter
                        let options = DecoderOptions::new_cmd()
                            .set_max_height(usize::MAX)
                            .set_max_width(usize::MAX);

                        let mut decoder =
                            format.decoder_with_options(contents, options).unwrap();

                        if let Ok(Some(metadata)) = decoder.read_headers() {
                            let real_metadata =
                                Metadata::new(in_file.to_os_string(), size, &metadata);

                            println!("{}", serde_json::to_string_pretty(&real_metadata).unwrap());
                        }
                    }
                }
            }
        }
    }
}
