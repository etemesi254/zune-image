use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{stdin, BufRead, BufWriter, Read};
use std::ops::Deref;
use std::path::Path;
use std::string::String;

use clap::parser::ValueSource;
use clap::ArgMatches;
use log::Level::Debug;
use log::{debug, info, log_enabled};
use memmap2::Mmap;
use zune_image::codecs::ppm::SPPMEncoder;
use zune_image::codecs::{guess_format, Codecs};
use zune_image::errors::ImgErrors;
use zune_image::impls::brighten::Brighten;
use zune_image::impls::crop::Crop;
use zune_image::impls::deinterleave::DeInterleaveChannels;
use zune_image::impls::flip::Flip;
use zune_image::impls::flop::Flop;
use zune_image::impls::grayscale::RgbToGrayScale;
use zune_image::impls::invert::Invert;
use zune_image::impls::mirror::{Mirror, MirrorMode};
use zune_image::impls::transpose::Transpose;
use zune_image::workflow::WorkFlow;

use crate::cmd_args::arg_parsers::{get_four_pair_args, parse_options_to_jpeg};
use crate::{CmdOptions, MmapOptions};

pub(crate) fn create_and_exec_workflow_from_cmd(
    args: &ArgMatches, cmd_opts: &CmdOptions,
) -> Result<(), ImgErrors>
{
    info!("Creating workflows from input");

    let mut buf = Vec::with_capacity(1 << 20);
    for (in_file, out_file) in args
        .get_raw("in")
        .unwrap()
        .zip(args.get_raw("out").unwrap())
    {
        let mut workflow = WorkFlow::new();

        {
            add_operations(args, &mut workflow)?;
            verify_file_paths(in_file, out_file, args)?;
        }

        let mut fd = File::open(in_file).unwrap();
        let mmap = unsafe { Mmap::map(&fd).unwrap() };
        let mmap_opt = cmd_opts.get_mmap_options();

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

        workflow.add_buffer(data);

        let file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(out_file)
            .unwrap();

        let buf_writer = BufWriter::new(file);

        if let Some(format) = guess_format(data)
        {
            if format == Codecs::Jpeg
            {
                debug!("Treating {:?} as a jpg file", in_file);

                let options = parse_options_to_jpeg(args);
                let decoder = zune_jpeg::JpegDecoder::new_with_options(options);

                workflow.add_decoder(Box::new(decoder));
            }
        }

        if let Some(ext) = Path::new(out_file).extension()
        {
            if ext == OsStr::new("ppm")
            {
                debug!("Treating {:?} as a ppm file", out_file);
                let encoder = SPPMEncoder::new(buf_writer);
                workflow.add_encoder(Box::new(encoder));
            }
        }

        workflow.advance_to_end()?;
    }
    Ok(())
}

fn verify_file_paths(p0: &OsStr, p1: &OsStr, args: &ArgMatches) -> Result<(), ImgErrors>
{
    if p0 == p1
    {
        return Err(ImgErrors::GenericString(format!(
            "Cannot use {:?} as both input and output",
            p0
        )));
    }
    let in_path = Path::new(p0);
    let out_path = Path::new(p1);

    if !in_path.exists()
    {
        return Err(ImgErrors::GenericString(format!(
            "Path {:?}, does not exist",
            in_path
        )));
    }

    if !in_path.is_file()
    {
        return Err(ImgErrors::GenericString(format!(
            "Path {:?} is not a file",
            in_path
        )));
    }

    if out_path.exists()
    {
        if args.value_source("all-yes") == Some(ValueSource::CommandLine)
        {
            info!("Overwriting path {:?} ", p1);
        }
        else
        {
            println!("File {:?} exists, overwrite [y/N]", out_path);
            let mut result = String::new();

            stdin().lock().read_line(&mut result).unwrap();

            if result.trim() != "y"
            {
                return Err(ImgErrors::GenericString(format!(
                    "Not overwriting file {:?}",
                    out_path
                )));
            }
        }
    }
    Ok(())
}

pub fn add_operations(args: &ArgMatches, workflow: &mut WorkFlow) -> Result<(), String>
{
    if args.value_source("operations") == Some(ValueSource::CommandLine)
    {
        workflow.add_operation(Box::new(DeInterleaveChannels::new()));
    }
    if log_enabled!(Debug) && args.value_source("operations") == Some(ValueSource::CommandLine)
    {
        println!();
    }

    for id in args.ids()
    {
        if args.try_get_many::<clap::Id>(id.as_str()).is_ok()
        {
            // ignore groups
            continue;
        }

        let value_source = args
            .value_source(id.as_str())
            .expect("id came from matches");

        if value_source != clap::parser::ValueSource::CommandLine
        {
            continue;
        }
        let argument = id.to_string();

        if argument == "flip"
        {
            debug!("Added flip operation");
            workflow.add_operation(Box::new(Flip::new()));
        }
        else if argument == "grayscale"
        {
            debug!("Added grayscale operation");
            workflow.add_operation(Box::new(RgbToGrayScale::new()));
        }
        else if argument == "transpose"
        {
            debug!("Added transpose operation");
            workflow.add_operation(Box::new(Transpose::new()));
        }
        else if argument == "flop"
        {
            debug!("Added transpose operation");
            workflow.add_operation(Box::new(Flop::new()))
        }
        else if argument == "mirror"
        {
            let value = args.get_one::<String>("mirror").unwrap().trim();
            let direction;

            if value == "north"
            {
                direction = MirrorMode::North;
            }
            else if value == "south"
            {
                direction = MirrorMode::South;
            }
            else if value == "east"
            {
                direction = MirrorMode::East;
            }
            else
            {
                direction = MirrorMode::West;
            }
            debug!("Added mirror with direction {:?}", value);
            workflow.add_operation(Box::new(Mirror::new(direction)))
        }
        else if argument == "invert"
        {
            debug!("Added invert operation");
            workflow.add_operation(Box::new(Invert::new()))
        }
        else if argument == "brighten"
        {
            let value = *args.get_one::<i16>(&argument).unwrap();
            debug!("Added brighten operation with {:?}", value);
            workflow.add_operation(Box::new(Brighten::new(value)))
        }
        else if argument == "crop"
        {
            let val = args.get_one::<String>(&argument).unwrap();
            let crop_args = get_four_pair_args(val)?;
            let crop = Crop::new(crop_args[0], crop_args[1], crop_args[2], crop_args[3]);

            debug!(
                "Added crop with arguments width={} height={} x={} y={}",
                crop_args[0], crop_args[1], crop_args[2], crop_args[3]
            );

            workflow.add_operation(Box::new(crop));
        }
    }
    if log_enabled!(Debug) && args.value_source("operations") == Some(ValueSource::CommandLine)
    {
        println!();
    }
    Ok(())
}
