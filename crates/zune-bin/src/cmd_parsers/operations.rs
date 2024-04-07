/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use clap::ArgMatches;
use log::debug;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::core_filters::depth::Depth;
use zune_image::pipelines::Pipeline;
use zune_image::traits::IntoImage;
use zune_imageprocs::brighten::Brighten;
use zune_imageprocs::contrast::Contrast;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::{Flip, VerticalFlip};
use zune_imageprocs::flop::Flop;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::hsv_adjust::HsvAdjust;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::mirror::{Mirror, MirrorMode};
use zune_imageprocs::resize::{Resize, ResizeMethod};
use zune_imageprocs::spatial::SpatialOps;
use zune_imageprocs::spatial_ops::SpatialOperations;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::threshold::{Threshold, ThresholdMethod};
use zune_imageprocs::transpose::Transpose;

use crate::cmd_args::arg_parsers::IColorSpace;

pub fn parse_options<T: IntoImage>(
    workflow: &mut Pipeline<T>, argument: &str, args: &ArgMatches
) -> Result<(), String> {
    if argument == "flip" {
        debug!("Added flip operation");
        workflow.chain_operations(Box::new(Flip::new()));
    } else if argument == "grayscale" {
        debug!("Added grayscale operation");
        workflow.chain_operations(Box::new(ColorspaceConv::new(ColorSpace::Luma)));
    } else if argument == "transpose" {
        debug!("Added transpose operation");
        workflow.chain_operations(Box::new(Transpose::new()));
    } else if argument == "flop" {
        debug!("Added flop operation");
        workflow.chain_operations(Box::new(Flop::new()));
    } else if argument == "median" {
        //let radius = *args.get_one::<usize>("median").unwrap();
        // workflow.add_operation(Box::new(Median::new(radius)));
        debug!("Added Median operation");
    } else if argument == "statistic" {
        let val: Vec<&String> = args.get_many::<String>(argument).unwrap().collect();

        // parse first one as radius
        let radius = str::parse::<usize>(val[0]).map_err(|x| x.to_string())?;
        let stats_mode = SpatialOperations::from_string_result(val[1])?;

        workflow.chain_operations(Box::new(SpatialOps::new(radius, stats_mode)));
        debug!("Added StatisticsOps operation");
    } else if argument == "mirror" {
        let value = args.get_one::<String>("mirror").unwrap().trim();
        let direction;

        if value == "north" {
            direction = MirrorMode::North;
        } else if value == "south" {
            direction = MirrorMode::South;
        } else if value == "east" {
            direction = MirrorMode::East;
        } else if value == "west" {
            direction = MirrorMode::West;
        } else {
            return Err(format!("Unknown mirror mode {value:?}"));
        }

        debug!("Added mirror with direction {:?}", value);
        workflow.chain_operations(Box::new(Mirror::new(direction)));
    } else if argument == "invert" {
        debug!("Added invert operation");
        workflow.chain_operations(Box::new(Invert::new()));
    } else if argument == "brighten" {
        let value = *args.get_one::<f32>(argument).unwrap();
        debug!("Added brighten operation with {:?}", value);
        workflow.chain_operations(Box::new(Brighten::new(value)));
    } else if argument == "crop" {
        let crop_args = args
            .get_many::<usize>(argument)
            .unwrap()
            .collect::<Vec<&usize>>();

        let crop = Crop::new(*crop_args[0], *crop_args[1], *crop_args[2], *crop_args[3]);

        debug!(
            "Added crop with arguments width={} height={} x={} y={}",
            crop_args[0], crop_args[1], crop_args[2], crop_args[3]
        );

        workflow.chain_operations(Box::new(crop));
    } else if argument == "threshold" {
        let val: Vec<&String> = args.get_many::<String>(argument).unwrap().collect();

        // parse first one as radius
        let radius = str::parse::<f32>(val[0]).map_err(|x| x.to_string())?;
        let thresh_mode = ThresholdMethod::from_string_result(val[1])?;
        let threshold = Threshold::new(radius, thresh_mode);

        workflow.chain_operations(Box::new(threshold));

        debug!(
            "Added threshold operation with mode {:?}  and value {:?}",
            thresh_mode, radius
        )
    } else if argument == "stretch_contrast" {
        let values = args
            .get_many::<f32>(argument)
            .unwrap()
            .collect::<Vec<&f32>>();

        let lower = *values[0];

        let upper = *values[1];

        debug!(
            "Added stretch contrast filter with lower={} and upper={}",
            lower, upper
        );
        let stretch_contrast = StretchContrast::new(lower, upper);
        workflow.chain_operations(Box::new(stretch_contrast));
    } else if argument == "gamma" {
        let value = *args.get_one::<f32>(argument).unwrap();
        debug!("Added gamma filter with value {}", value);
        workflow.chain_operations(Box::new(Gamma::new(value)));
    } else if argument == "contrast" {
        let value = *args.get_one::<f32>(argument).unwrap();
        debug!("Added contrast filter with value {},", value);
        workflow.chain_operations(Box::new(Contrast::new(value)));
    } else if argument == "resize" {
        let values = args
            .get_many::<usize>(argument)
            .unwrap()
            .collect::<Vec<&usize>>();

        let width = *values[0];

        let height = *values[1];

        let func = Resize::new(width, height, ResizeMethod::Bilinear);

        debug!(
            "Added resize operation with width:{}, height:{}",
            width, height
        );
        workflow.chain_operations(Box::new(func));
    } else if argument == "depth" {
        let value = *args.get_one::<u8>(argument).unwrap();
        let depth = match value {
            8 => BitDepth::Eight,
            16 => BitDepth::Sixteen,
            _ => {
                return Err(format!(
                    "Unknown depth value {value}, supported depths are 8 and 16"
                ))
            }
        };
        debug!("Added depth operation with depth of {value}");

        workflow.chain_operations(Box::new(Depth::new(depth)));
    } else if argument == "colorspace" {
        let colorspace = args
            .get_one::<IColorSpace>("colorspace")
            .unwrap()
            .to_colorspace();

        debug!("Added colorspace conversion from source colorspace to {colorspace:?}");

        workflow.chain_operations(Box::new(ColorspaceConv::new(colorspace)));
    } else if argument == "auto-orient" {
        debug!("Add auto orient operation");
        //workflow.add_operation(Box::new(AutoOrient))
    } else if argument == "exposure" {
        let exposure = *args.get_one::<f32>(argument).unwrap();

        workflow.chain_operations(Box::new(Exposure::new(exposure, 0.)));
        debug!("Adding exposure argument with value {}", exposure);
    } else if argument == "v-flip" {
        debug!("Added v-flip argument");
        workflow.chain_operations(Box::new(VerticalFlip::new()));
    } else if argument == "huerotate" {
        let value = *args.get_one::<f32>(argument).unwrap();
        workflow.chain_operations(Box::new(HsvAdjust::new(value, 1f32, 1f32)));
        debug!("Added hue-rotate argument with value {}", value);
    } else if argument == "saturate" {
        let value = *args.get_one::<f32>(argument).unwrap();
        workflow.chain_operations(Box::new(HsvAdjust::new(0f32, value, 1f32)));
        debug!("Added saturate argument with value {}", value);
    } else if argument == "lightness" {
        let value = *args.get_one::<f32>(argument).unwrap();
        workflow.chain_operations(Box::new(HsvAdjust::new(0f32, 1f32, value)));
        debug!("Added lightness argument with value {}", value);
    }

    Ok(())
}
