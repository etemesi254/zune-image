/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::string::String;
use std::time::Instant;

use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use log::{error, info, log_enabled, trace};
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;
use zune_image::pipelines::Pipeline;

use crate::cmd_args::CmdImageFormats;
use crate::cmd_parsers::global_options::CmdOptions;
use crate::cmd_parsers::{decoder_options, encoder_options};
use crate::file_io::{ZuneFile, ZuneMem};
use crate::probe_files::probe_input_files;
use crate::show_gui::open_in_default_app;

struct CmdPipeline {
    inner:   Pipeline,
    formats: Vec<(ImageFormat, std::ffi::OsString)>
}
impl CmdPipeline {
    pub fn new() -> CmdPipeline {
        CmdPipeline {
            inner:   Pipeline::new(),
            formats: vec![]
        }
    }
}

#[allow(unused_variables)]
#[allow(clippy::unused_io_amount)] // yes it's what I want
pub(crate) fn create_and_exec_workflow_from_cmd(
    args: &ArgMatches, cmd_opts: &CmdOptions
) -> Result<(), ImageErrors> {
    if let Some(view) = args.value_source("probe") {
        if view == CommandLine {
            probe_input_files(args);
            return Ok(());
        }
    }

    info!("Creating workflows from input");

    if log_enabled!(log::Level::Trace) {
        println!()
    }
    let decoder_options = decoder_options(args);
    let mut buf = [0; 30];

    for in_file in args.get_raw("in").unwrap() {
        let mut workflow: CmdPipeline = CmdPipeline::new();

        if in_file == "-" {
            // handle stdin
            let mut data = Vec::new();
            let bytes_read = std::io::stdin().read_to_end(&mut data)?;
            if let Some((format, _)) = ImageFormat::guess_format(std::io::Cursor::new(&data)) {
                if format.has_decoder() {
                    workflow
                        .inner
                        .chain_decoder(Box::new(ZuneMem::new(data, decoder_options)));
                } else {
                    return Err(ImageErrors::ImageDecoderNotImplemented(format));
                }
            } else {
                return Err(ImageErrors::ImageDecoderNotIncluded(ImageFormat::Unknown));
            }
        } else {
            File::open(in_file)?.read(&mut buf)?;

            add_operations(args, &mut workflow.inner)?;

            if let Some((format, _)) = ImageFormat::guess_format(std::io::Cursor::new(&buf)) {
                if format.has_decoder() {
                    workflow.inner.chain_decoder(Box::new(ZuneFile::new(
                        in_file.to_os_string(),
                        decoder_options
                    )));
                } else {
                    return Err(ImageErrors::ImageDecoderNotImplemented(format));
                }
            } else {
                return Err(ImageErrors::ImageDecoderNotIncluded(ImageFormat::Unknown));
            }
        }

        let options = encoder_options(args);

        if let Some(source) = args.value_source("out") {
            if source == CommandLine {
                for out_file in args.get_raw("out").unwrap() {
                    let path = Path::new(out_file);
                    if path.exists() && !cmd_opts.override_files && path.is_file() {
                        let msg = format!("File {path:?} already exists overwrite [y/N]?");
                        eprintln!("{msg}");
                        let _ = std::io::stdout().flush();
                        let mut response = String::new();
                        let _ = std::io::stdin()
                            .read_line(&mut response)
                            .expect("Unable to read from stdin");

                        if !response.to_lowercase().starts_with("y") {
                            return Err(ImageErrors::GenericStr(
                                "Aborting due to file existence"
                            ));
                        }
                    }

                    if let Some(ext) = path.extension() {
                        if let Some(encode_type) =
                            ImageFormat::encoder_for_extension(ext.to_str().unwrap())
                        {
                            info!("Treating {out_file:?} as a {encode_type:?} format");
                            workflow
                                .formats
                                .push((encode_type, out_file.to_os_string()));
                        } else {
                            error!("Unknown or unsupported format {out_file:?}")
                        }
                        // check for path details before even carrying out operations
                        // this
                    } else if out_file == "-" {
                        if let Some(cmd_format) = args.get_one::<CmdImageFormats>("output-format") {
                            let CmdImageFormats::Format(format) = cmd_format;

                            workflow.formats.push((*format, out_file.to_os_string()))
                        } else {
                            return Err(ImageErrors::GenericStr("You must specify the image format to be used while using output as '-` via the --output-format flag "));
                        }
                        error!("Could not determine extension from {out_file:?}");
                    }
                }
            }
        }

        workflow.inner.advance_to_end()?;

        // write to output

        //  We support multiple format writes per invocation
        // i.e it's perfectly valid to do -o a.ppm , -o a.png
        for (format, out_file) in &workflow.formats {
            if format.has_encoder() {
                if log_enabled!(log::Level::Trace) {
                    println!();
                    trace!("Encoding to format {format:?} to file {out_file:?} ");
                }
                for image in workflow.inner.images() {
                    let fd = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(out_file);
                    match fd {
                        Ok(file) => {
                            let mut file_c = BufWriter::new(file);
                            let start = Instant::now();
                            let bytes = format.encode(image, options, &mut file_c)?;
                            let end = Instant::now();
                            trace!(
                                "Took {:?} to encode {} bytes to {:?}",
                                end - start,
                                bytes,
                                out_file
                            );
                        }
                        Err(e) => {
                            error!("Cannot encode to file, error opening {e:?}");
                        }
                    }
                }
            }
        }
        if let Some(view) = args.value_source("view") {
            if view == CommandLine {
                for image in workflow.inner.images() {
                    open_in_default_app(image,options);
                }
            }
        }
    }

    Ok(())
}

pub fn add_operations(args: &ArgMatches, workflow: &mut Pipeline) -> Result<(), String> {
    for id in args.ids() {
        if args.try_get_many::<clap::Id>(id.as_str()).is_ok() {
            // ignore groups
            continue;
        }

        let value_source = args
            .value_source(id.as_str())
            .expect("id came from matches");

        if value_source != clap::parser::ValueSource::CommandLine {
            // ignore things not passed via command line
            continue;
        }

        crate::cmd_parsers::operations::parse_options(workflow, id.as_str(), args)?;
        crate::cmd_parsers::filters::parse_options(workflow, id.as_str(), args)?;
    }

    Ok(())
}
