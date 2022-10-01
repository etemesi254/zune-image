use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read};
use std::ops::Deref;

use clap::parser::ValueSource;
use clap::ArgMatches;
use log::{debug, info};
use memmap2::Mmap;
use zune_image::codecs::ppm::SPPMEncoder;
use zune_image::codecs::{guess_format, Codecs};
use zune_image::errors::ImgErrors;
use zune_image::impls::deinterleave::DeInterleaveThreeChannels;
use zune_image::impls::grayscale::RgbToGrayScale;
use zune_image::workflow::WorkFlow;

use crate::{CmdOptions, MmapOptions};

pub(crate) fn create_and_exec_workflow_from_cmd(
    args: &ArgMatches, cmd_opts: &CmdOptions,
) -> Result<(), ImgErrors>
{
    info!("Creating workflows from input");

    let mut buf = Vec::with_capacity(1 << 20);
    for (in_file, out_file) in args
        .get_many::<String>("in")
        .unwrap()
        .zip(args.get_many::<String>("out").unwrap())
    {
        let mut workflow = WorkFlow::new();

        let mut fd = File::open(in_file).unwrap();

        let mmap = unsafe { Mmap::map(&fd).unwrap() };

        let mmap_opt = cmd_opts.get_mmap_options();
        let data = if mmap_opt == MmapOptions::Auto || mmap_opt == MmapOptions::Always
        {
            info!("Reading file via memory maps");
            mmap.deref()
        }
        else
        {
            info!("Reading file to memory");
            fd.read_to_end(&mut buf).unwrap();
            &buf
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
                debug!("Treating {} as a jpg file", in_file);

                workflow.add_decoder(Box::new(zune_jpeg::JpegDecoder::new()));
            }
        }
        if let Some(value) = args.value_source("operations")
        {
            if value == ValueSource::CommandLine
            {
                workflow.add_operation(Box::new(DeInterleaveThreeChannels::new()));
            }
        }

        if let Some(value) = args.value_source("grayscale")
        {
            if value == ValueSource::CommandLine
            {
                workflow.add_operation(Box::new(RgbToGrayScale::new()));
            }
        }

        if out_file.ends_with(".ppm")
        {
            let encoder = SPPMEncoder::new(buf_writer);
            workflow.add_encoder(Box::new(encoder));
        }

        workflow.advance_to_end()?;
    }
    Ok(())
}
