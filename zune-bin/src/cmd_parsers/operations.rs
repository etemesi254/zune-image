use clap::ArgMatches;
use log::debug;
use zune_core::bit_depth::BitDepth;
use zune_image::impls::brighten::Brighten;
use zune_image::impls::colorspace::ColorspaceConv;
use zune_image::impls::contrast::Contrast;
use zune_image::impls::crop::Crop;
use zune_image::impls::depth::Depth;
use zune_image::impls::flip::Flip;
use zune_image::impls::flop::Flop;
use zune_image::impls::gamma::Gamma;
use zune_image::impls::grayscale::RgbToGrayScale;
use zune_image::impls::invert::Invert;
use zune_image::impls::median::Median;
use zune_image::impls::mirror::{Mirror, MirrorMode};
use zune_image::impls::orientation::AutoOrient;
use zune_image::impls::resize::{Resize, ResizeMethod};
use zune_image::impls::statistics::{StatisticOperations, StatisticsOps};
use zune_image::impls::stretch_contrast::StretchContrast;
use zune_image::impls::threshold::{Threshold, ThresholdMethod};
use zune_image::impls::transpose::Transpose;
use zune_image::traits::IntoImage;
use zune_image::workflow::WorkFlow;

use crate::cmd_args::arg_parsers::{get_four_pair_args, IColorSpace};

pub fn parse_options<T: IntoImage>(
    workflow: &mut WorkFlow<T>, order_args: &[String], args: &ArgMatches
) -> Result<(), String>
{
    for argument in order_args
    {
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
            debug!("Added flop operation");
            workflow.add_operation(Box::new(Flop::new()))
        }
        else if argument == "median"
        {
            let radius = *args.get_one::<usize>("median").unwrap();
            workflow.add_operation(Box::new(Median::new(radius)));
            debug!("Added Median operation");
        }
        else if argument == "statistic"
        {
            let val = args.get_one::<String>(argument).unwrap();
            let split_args: Vec<&str> = val.split(':').collect();

            if split_args.len() != 2
            {
                return Err(format!("Statistic operation expected 2 arguments separated by `:` in the command line,got {}", split_args.len()));
            }
            // parse first one as radius
            let thresh_string = split_args[0];
            let radius = str::parse::<usize>(thresh_string).map_err(|x| x.to_string())?;
            let stats_mode = StatisticOperations::from_string_result(split_args[1])?;

            workflow.add_operation(Box::new(StatisticsOps::new(radius, stats_mode)));
            debug!("Added StatisticsOps operation");
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
            else if value == "west"
            {
                direction = MirrorMode::West;
            }
            else
            {
                return Err(format!("Unknown mirror mode {value:?}"));
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
            let value = *args.get_one::<f32>(argument).unwrap();
            debug!("Added brighten operation with {:?}", value);
            workflow.add_operation(Box::new(Brighten::new(value)))
        }
        else if argument == "crop"
        {
            let val = args.get_one::<String>(argument).unwrap();
            let crop_args = get_four_pair_args(val)?;
            let crop = Crop::new(crop_args[0], crop_args[1], crop_args[2], crop_args[3]);

            debug!(
                "Added crop with arguments width={} height={} x={} y={}",
                crop_args[0], crop_args[1], crop_args[2], crop_args[3]
            );

            workflow.add_operation(Box::new(crop));
        }
        else if argument == "threshold"
        {
            let val = args.get_one::<String>(argument).unwrap();
            let split_args: Vec<&str> = val.split(':').collect();

            if split_args.len() != 2
            {
                return Err(format!("Threshold operation expected 2 arguments separated by `:` in the command line,got {}", split_args.len()));
            }
            // parse first one as threshold
            let thresh_string = split_args[0];
            let thresh_int = str::parse::<u16>(thresh_string).map_err(|x| x.to_string())?;
            let thresh_mode = ThresholdMethod::from_string_result(split_args[1])?;

            let threshold = Threshold::new(thresh_int, thresh_mode);
            workflow.add_operation(Box::new(threshold));

            debug!(
                "Added threshold operation with mode {:?}  and value {:?}",
                thresh_mode, thresh_int
            )
        }
        else if argument == "stretch_contrast"
        {
            let values = args
                .get_many::<u16>(argument)
                .unwrap()
                .collect::<Vec<&u16>>();

            let lower = *values[0];

            let upper = *values[1];

            debug!(
                "Added stretch contrast filter with lower={} and upper={}",
                lower, upper
            );
            let stretch_contrast = StretchContrast::new(lower, upper);
            workflow.add_operation(Box::new(stretch_contrast));
        }
        else if argument == "gamma"
        {
            let value = *args.get_one::<f32>(argument).unwrap();
            debug!("Added gamma filter with value {}", value);
            workflow.add_operation(Box::new(Gamma::new(value)));
        }
        else if argument == "contrast"
        {
            let value = *args.get_one::<f32>(argument).unwrap();
            debug!("Added contrast filter with value {},", value);
            workflow.add_operation(Box::new(Contrast::new(value)));
        }
        else if argument == "resize"
        {
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
            workflow.add_operation(Box::new(func));
        }
        else if argument == "depth"
        {
            let value = *args.get_one::<u8>(argument).unwrap();
            let depth = match value
            {
                8 => BitDepth::Eight,
                16 => BitDepth::Sixteen,
                _ =>
                {
                    return Err(format!(
                        "Unknown depth value {value}, supported depths are 8 and 16"
                    ))
                }
            };
            debug!("Added depth operation with depth of {value}");

            workflow.add_operation(Box::new(Depth::new(depth)));
        }
        else if argument == "colorspace"
        {
            let colorspace = args
                .get_one::<IColorSpace>("colorspace")
                .unwrap()
                .to_colorspace();

            debug!("Added colorspace conversion from source colorspace to {colorspace:?}");

            workflow.add_operation(Box::new(ColorspaceConv::new(colorspace)))
        }
        else if argument == "auto-orient"
        {
            debug!("Add auto orient operation");
            workflow.add_operation(Box::new(AutoOrient))
        }
    }
    Ok(())
}
