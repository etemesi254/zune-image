use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::ops::Deref;
use std::path::Path;
use std::string::String;

use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use log::Level::Debug;
use log::{debug, error, info, log_enabled};
use memmap2::Mmap;
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImgErrors;
use zune_image::traits::DecoderTrait;
use zune_image::workflow::WorkFlow;

use crate::cmd_parsers::get_decoder_options;
use crate::cmd_parsers::global_options::CmdOptions;
use crate::show_gui::open_in_default_app;
use crate::MmapOptions;

#[allow(unused_variables)]
pub(crate) fn create_and_exec_workflow_from_cmd(
    args: &ArgMatches, options: &[String], cmd_opts: &CmdOptions
) -> Result<(), ImgErrors>
{
    info!("Creating workflows from input");

    let decoder_options = get_decoder_options(args);

    let mut buf = Vec::with_capacity(1 << 20);
    for in_file in args.get_raw("in").unwrap()
    {
        // file i/o
        let mut fd = File::open(in_file).unwrap();
        let mmap = unsafe { Mmap::map(&fd).unwrap() };
        let mmap_opt = cmd_opts.mmap;

        // Decide how we are reading files
        // this has to be here due to Rust ownership rules, etc etc
        let data = {
            if mmap_opt == MmapOptions::Auto || mmap_opt == MmapOptions::Always
            {
                info!("Reading file via memory maps");
                mmap.deref()
            }
            else
            {
                info!("Reading file to memory");
                fd.read_to_end(&mut buf).unwrap();
                &buf
            }
        };
        // workflow

        // Rust was pretty good to catch this.
        // Thank you compiler gods.

        let mut workflow = WorkFlow::new();

        add_operations(args, options, &mut workflow)?;

        if let Some(format) = ImageFormat::guess_format(data)
        {
            let decoder: Box<dyn DecoderTrait> =
                format.get_decoder_with_options(data, decoder_options);

            if decoder.is_experimental() && !cmd_opts.experimental_formats
            {
                let msg = format!("The `{}` is currently experimental and can only be used when --experimental is passed via the command line", decoder.get_name());
                return Err(ImgErrors::from(msg));
            }
            workflow.add_decoder(decoder);
        }

        if let Some(source) = args.value_source("out")
        {
            if source == CommandLine
            {
                for out_file in args.get_raw("out").unwrap()
                {
                    if let Some(ext) = Path::new(out_file).extension()
                    {
                        if let Some((encode_type, encoder)) =
                            ImageFormat::get_encoder_for_extension(ext.to_str().unwrap())
                        {
                            debug!("Treating {:?} as a {:?} format", out_file, encode_type);
                            workflow.add_encoder(encoder);
                        }
                        else
                        {
                            error!("Unknown or unsupported format {:?}", out_file)
                        }
                    }
                    else
                    {
                        error!("Could not determine extension from {:?}", out_file)
                    }
                }
            }
        }

        workflow.advance_to_end()?;
        let results = workflow.get_results();
        if let Some(view) = args.value_source("probe")
        {
            if view == CommandLine
            {
                for image in workflow.get_images()
                {
                    let metadata = image.get_metadata();
                    let real_metadata =
                        crate::serde::Metadata::new(in_file.to_os_string(), metadata);
                    println!("{}", serde_json::to_string_pretty(&real_metadata).unwrap())
                }
            }
        }
        if let Some(source) = args.value_source("out")
        {
            if source == CommandLine
            {
                for out_file in args.get_raw("out").unwrap()
                {
                    let mut file = OpenOptions::new()
                        .create(true)
                        .append(false)
                        .truncate(true)
                        .write(true)
                        .open(out_file)
                        .unwrap();
                    //write to file
                    for result in results.iter().take(1)
                    {
                        info!(
                            "Writing data as {:?} format to file {:?}",
                            result.get_format(),
                            out_file
                        );

                        file.write_all(result.get_data()).unwrap();
                    }
                }
            }
        }

        if let Some(view) = args.value_source("view")
        {
            if view == CommandLine
            {
                for image in workflow.get_images()
                {
                    open_in_default_app(image);
                }
            }
        }
    }

    Ok(())
}

pub fn add_operations(
    args: &ArgMatches, order_args: &[String], workflow: &mut WorkFlow
) -> Result<(), String>
{
    if log_enabled!(Debug) && args.value_source("operations") == Some(CommandLine)
    {
        println!();
    }

    crate::cmd_parsers::operations::parse_options(workflow, order_args, args)?;
    crate::cmd_parsers::filters::parse_options(workflow, order_args, args)?;

    debug!("Arranging options as specified in cmd");

    if log_enabled!(Debug) && args.value_source("operations") == Some(CommandLine)
    {
        println!();
    }
    Ok(())
}
