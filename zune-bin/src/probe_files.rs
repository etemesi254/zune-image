use std::fs::File;
use std::ops::Deref;
use std::path::PathBuf;

use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use memmap2::Mmap;
use zune_core::options::DecoderOptions;

use crate::serde::Metadata;

/// Probe input files, extract metadata, and print to standard output.
pub fn probe_input_files(args: &ArgMatches)
{
    if let Some(view) = args.value_source("probe")
    {
        if view == CommandLine
        {
            for in_file in args.get_raw("in").unwrap()
            {
                if PathBuf::from(in_file).exists()
                {
                    let file = File::open(in_file).unwrap();
                    let file_size = file.metadata().unwrap().len();
                    // Unsafety: Mmap in Linux is not protected, interesting things
                    // will occur if you mess with the file
                    let mmap = unsafe { Mmap::map(&file).unwrap() };

                    let file_contents = mmap.deref();

                    if let Some(format) =
                        zune_image::codecs::ImageFormat::guess_format(file_contents)
                    {
                        // set to high to remove restrictions.
                        // We'll just be reading headers so it doesn't matter
                        let options = DecoderOptions::new_cmd()
                            .set_max_height(usize::MAX)
                            .set_max_width(usize::MAX);

                        let mut decoder = format.get_decoder_with_options(file_contents, options);

                        if let Ok(Some(metadata)) = decoder.read_headers()
                        {
                            let real_metadata =
                                Metadata::new(in_file.to_os_string(), file_size, &metadata);

                            println!("{}", serde_json::to_string_pretty(&real_metadata).unwrap());
                        }
                    }
                }
            }
        }
    }
}
