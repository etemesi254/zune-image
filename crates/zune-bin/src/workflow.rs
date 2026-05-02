/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, IsTerminal, Read, Write};
use std::path::Path;
use std::string::String;
use std::time::Instant;

use crate::cmd_args::CmdImageFormats;
use crate::cmd_parsers::global_options::CmdOptions;
use crate::cmd_parsers::{decoder_options, encoder_options};
use crate::file_io::{ZuneFile, ZuneMem, ZuneWeb};
use crate::probe_files::probe_input_files;
use crate::show_gui::open_in_default_app;
use clap::parser::ValueSource::CommandLine;
use clap::ArgMatches;
use log::{error, info, log_enabled, trace};
use zune_image::codecs::ImageFormat;
use zune_image::errors::ImageErrors;
use zune_image::pipelines::Pipeline;
use zune_image::traits::OperationsTrait;

struct CmdPipeline {
    inner: Pipeline,
    formats: Vec<(ImageFormat, std::ffi::OsString)>,
}
impl CmdPipeline {
    pub fn new() -> CmdPipeline {
        CmdPipeline {
            inner: Pipeline::new(),
            formats: vec![],
        }
    }
}

#[allow(unused_variables)]
#[allow(clippy::unused_io_amount)] // yes it's what I want
pub(crate) fn create_and_exec_workflow_from_cmd(
    args: &ArgMatches, cmd_opts: &CmdOptions,
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

    // Initialize a single workflow for the entire batch of images
    let mut workflow: CmdPipeline = CmdPipeline::new();
    let options = encoder_options(args);

    for in_file in args.get_raw("in").unwrap() {
        if in_file == "-" {
            // Handle stdin completely in memory
            let mut data = Vec::new();
            std::io::stdin().read_to_end(&mut data)?;

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

            let in_file_str = in_file.to_string_lossy();
            let is_web = in_file_str.starts_with("http://") || in_file_str.starts_with("https://");

            let mut header = [0u8; 32];
            let bytes_read;

            let mut web_parts = None;
            let mut local_file = None;
            if is_web {
                {
                    // 1. Make the request and grab the Content-Length
                    let resp = ureq::get(&in_file_str.to_string())
                        .call()
                        .map_err(|e| ImageErrors::GenericString(format!("Network error: {}", e)))?;

                    let content_length = resp
                        .headers()
                        .get("Content-Length")
                        .and_then(|s| s.to_str().ok()?.parse::<u64>().ok());

                    let mut stream =
                        Box::new(resp.into_body().into_reader()) as Box<dyn Read + Send + Sync>;

                    // 2. Read the header for format guessing
                    bytes_read = stream.read(&mut header)?;

                    // 3. Save the active stream and the read bytes for later
                    let initial_data = header[..bytes_read].to_vec();
                    web_parts = Some((stream, initial_data, content_length));
                }
            } else {
                let mut file = File::open(in_file)?;
                bytes_read = file.read(&mut header)?;

                // Store the file handle to pass to ZuneFile
                local_file = Some(file);
            }

            // 4. Guess format and chain
            if let Some((format, _)) =
                ImageFormat::guess_format(std::io::Cursor::new(&header[..bytes_read]))
            {
                if format.has_decoder() {
                    if is_web {
                        if let Some((stream, initial_data, content_length)) = web_parts {
                            // Construct the WebBuffer right here with the live connection
                            let web_buf = zune_image::web::WebBuffer::from_parts(
                                stream,
                                initial_data,
                                content_length,
                            );
                            workflow
                                .inner
                                .chain_decoder(Box::new(ZuneWeb::new(web_buf, decoder_options)));
                        }
                    } else {
                        if let Some(file) = local_file {
                            // Pass the open file directly; no more re-opening from the path string
                            workflow.inner.chain_decoder(Box::new(ZuneFile::new(
                                file,
                                decoder_options
                            )));
                        }
                    }
                } else {
                    return Err(ImageErrors::ImageDecoderNotImplemented(format));
                }
            } else {
                return Err(ImageErrors::ImageDecoderNotIncluded(ImageFormat::Unknown));
            }
        }
    }
    add_operations(args, &mut workflow.inner)?;
    workflow.inner.advance_to_end()?;

    if let Some(source) = args.value_source("out") {
        if source == clap::parser::ValueSource::CommandLine {
            for out_file in args.get_raw("out").unwrap() {
                let path = Path::new(out_file);

                // Check for file overwrite safely
                if path.exists() && !cmd_opts.override_files && path.is_file() {
                    // IMPORTANT: Only prompt if stdin is a terminal.
                    // If they piped an image in, `read_line` will try to read the image!
                    if std::io::stdin().is_terminal() {
                        eprint!("File {path:?} already exists. Overwrite? [y/N]: ");
                        let _ = std::io::stdout().flush();
                        let mut response = String::new();
                        std::io::stdin()
                            .read_line(&mut response)
                            .expect("Unable to read from stdin");

                        if !response.to_lowercase().starts_with('y') {
                            return Err(ImageErrors::GenericStr("Aborting due to file existence"));
                        }
                    } else {
                        return Err(ImageErrors::GenericStr("File exists and cannot prompt for overwrite in non-interactive mode. Use --force."));
                    }
                }

                // Determine output format
                if out_file == "-" {
                    if let Some(cmd_format) = args.get_one::<CmdImageFormats>("output-format") {
                        let CmdImageFormats::Format(format) = cmd_format;
                        workflow.formats.push((*format, out_file.to_os_string()));
                    } else {
                        return Err(ImageErrors::GenericStr("You must specify the image format via --output-format when outputting to stdout ('-')"));
                    }
                } else if let Some(ext) = path.extension() {
                    if let Some(encode_type) =
                        ImageFormat::encoder_for_extension(ext.to_str().unwrap())
                    {
                        info!("Treating {out_file:?} as a {encode_type:?} format");
                        workflow
                            .formats
                            .push((encode_type, out_file.to_os_string()));
                    } else {
                        error!("Unknown or unsupported format for {out_file:?}");
                        return Err(ImageErrors::GenericStr("Unsupported output format"));
                    }
                } else {
                    error!("Could not determine extension from {out_file:?}");
                    return Err(ImageErrors::GenericStr("Output file missing extension"));
                }
            }
        }
    }

    // Write generated images to output formats
    for (format, out_file) in &workflow.formats {
        if format.has_encoder() {
            if log_enabled!(log::Level::Trace) {
                println!();
                trace!("Encoding to format {format:?} to file {out_file:?}");
            }

            for image in workflow.inner.images() {
                // Write to stdout or file
                let mut writer: Box<dyn Write> = if out_file == "-" {
                    Box::new(BufWriter::new(std::io::stdout()))
                } else {
                    let file = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(out_file)
                        .map_err(|e| {
                            error!("Cannot encode to file, error opening {e:?}");
                            ImageErrors::GenericStr("File open error")
                        })?;
                    Box::new(BufWriter::new(file))
                };

                let start = Instant::now();
                let bytes = format.encode(image, options, &mut writer)?;
                let end = Instant::now();

                trace!(
                    "Took {:?} to encode {} bytes to {:?}",
                    end - start,
                    bytes,
                    out_file
                );
            }
        }
    }
    // View if requested
    if let Some(view) = args.value_source("view") {
        if view == clap::parser::ValueSource::CommandLine {
            for image in workflow.inner.images() {
                open_in_default_app(image, options);
            }
        }
    }

    Ok(())
}

pub fn add_operations(args: &ArgMatches, workflow: &mut Pipeline) -> Result<(), String> {
    // A master list to hold every operation and its CLI position
    let mut all_operations: Vec<(usize, Box<dyn OperationsTrait>)> = Vec::new();

    for id in args.ids() {
        let id_str = id.as_str();

        if args.try_get_many::<clap::Id>(id_str).is_ok() {
            // ignore groups
            continue;
        }

        let value_source = args.value_source(id_str).expect("id came from matches");

        if value_source != clap::parser::ValueSource::CommandLine {
            // ignore things not passed via command line
            continue;
        }

        // Collect operations from your parsers
        // (Assuming you update cmd_parsers::filters::parse_options to match the new signature too)

        let mut ops = crate::cmd_parsers::operations::parse_options(id_str, args)?;
        all_operations.append(&mut ops);

        let mut filters = crate::cmd_parsers::filters::parse_options(id_str, args)?;
        all_operations.append(&mut filters);
    }

    // THE MAGIC STEP: Sort all operations chronologically by their command line index
    all_operations.sort_by_key(|(index, _)| *index);

    // Chain them into the workflow in the exact order the user typed them
    for (_, operation) in all_operations {
        workflow.chain_operations(operation);
    }

    Ok(())
}
