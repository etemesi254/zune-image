/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use log::{debug, info};
use regex::Regex;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::core_filters::depth::Depth;
use zune_imageprocs::append::{Append, AppendDirection};
use zune_imageprocs::auto_orient::AutoOrient;
use zune_imageprocs::average::AverageSequence;
use zune_imageprocs::blend::Blend;
use zune_imageprocs::brighten::Brighten;
use zune_imageprocs::composite::{Composite, CompositeMethod};
use zune_imageprocs::contrast::Contrast;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::{Flip, FlipDirection};
use zune_imageprocs::fx::Fx;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::hsv_adjust::HsvAdjust;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::mirror::{Mirror, MirrorMode};
use zune_imageprocs::resize::{Resize, ResizeDimensions, ResizeMethod};
use zune_imageprocs::rotate::Rotate;
use zune_imageprocs::spatial::SpatialOps;
use zune_imageprocs::spatial_ops::SpatialOperations;
use zune_imageprocs::ssim::SsimDetection;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::swap::Swap;
use zune_imageprocs::threshold::{Threshold, ThresholdMethod};
use zune_imageprocs::transpose::Transpose;

use crate::cmd_args::arg_parsers::IResizeMethod;

use zune_image::traits::OperationsTrait;

pub fn parse_options(
    argument: &str, args: &clap::ArgMatches,
) -> Result<Vec<(usize, Box<dyn OperationsTrait>)>, String> {
    let mut parsed_ops: Vec<(usize, Box<dyn OperationsTrait>)> = Vec::new();

    if argument == "flip" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed flip operation at {idx}");
                parsed_ops.push((idx, Box::new(Flip::new(FlipDirection::MirrorXAxis))));
            }
        }
    } else if argument == "grayscale" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed grayscale operation at {idx}");
                parsed_ops.push((idx, Box::new(ColorspaceConv::new(ColorSpace::Luma))));
            }
        }
    } else if argument == "transpose" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed transpose operation at {idx}");
                parsed_ops.push((idx, Box::new(Transpose::new())));
            }
        }
    } else if argument == "flop" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed flop operation at {idx}");
                parsed_ops.push((idx, Box::new(Flip::new(FlipDirection::Horizontal))));
            }
        }
    } else if argument == "median" {
        // Just logging for median as in your original snippet
        debug!("Parsed Median operation");
    } else if argument == "statistic" {
        let values: Vec<&String> = args.get_many::<String>(argument).unwrap().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(2).zip(indices.chunks(2)) {
            let radius = str::parse::<usize>(chunk[0]).map_err(|x| x.to_string())?;
            let stats_mode = SpatialOperations::from_string_result(chunk[1])?;
            debug!("Parsed StatisticsOps operation at {}", idx_chunk[0]);
            parsed_ops.push((idx_chunk[0], Box::new(SpatialOps::new(radius, stats_mode))));
        }
    } else if argument == "mirror" {
        let values = args.get_many::<String>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();

        for (value, idx) in values.zip(indices) {
            let val = value.trim();
            let direction = match val {
                "north" => MirrorMode::North,
                "south" => MirrorMode::South,
                "east" => MirrorMode::East,
                "west" => MirrorMode::West,
                _ => return Err(format!("Unknown mirror mode {val:?}")),
            };
            debug!("Parsed mirror with direction {val:?} at {idx}");
            parsed_ops.push((idx, Box::new(Mirror::new(direction))));
        }
    } else if argument == "invert" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed invert operation at {idx}");
                parsed_ops.push((idx, Box::new(Invert::new())));
            }
        }
    } else if argument == "brighten" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed brighten operation with {value:?} at {idx}");
            parsed_ops.push((idx, Box::new(Brighten::new(*value))));
        }
    } else if argument == "crop" {
        let values: Vec<&usize> = args.get_many::<usize>(argument).unwrap().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(4).zip(indices.chunks(4)) {
            debug!(
                "Parsed crop with arguments width={} height={} x={} y={} at {}",
                chunk[0], chunk[1], chunk[2], chunk[3], idx_chunk[0]
            );
            parsed_ops.push((idx_chunk[0], Box::new(Crop::new(*chunk[0], *chunk[1], *chunk[2], *chunk[3]))));
        }
    } else if argument == "threshold" {
        let values: Vec<&String> = args.get_many::<String>(argument).unwrap().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(2).zip(indices.chunks(2)) {
            let radius = str::parse::<f32>(chunk[0]).map_err(|x| x.to_string())?;
            let thresh_mode = ThresholdMethod::from_string_result(chunk[1])?;
            debug!("Parsed threshold operation with mode {thresh_mode:?} and value {radius:?} at {}", idx_chunk[0]);
            parsed_ops.push((idx_chunk[0], Box::new(Threshold::new(radius, thresh_mode))));
        }
    } else if argument == "stretch-contrast" {
        let values: Vec<&f32> = args.get_many::<f32>(argument).unwrap().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(2).zip(indices.chunks(2)) {
            let lower = *chunk[0];
            let upper = *chunk[1];
            debug!("Parsed stretch contrast filter with lower={lower} and upper={upper} at {}", idx_chunk[0]);
            parsed_ops.push((idx_chunk[0], Box::new(StretchContrast::new(lower, upper))));
        }
    } else if argument == "gamma" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed gamma filter with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(Gamma::new(*value))));
        }
    } else if argument == "contrast" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed contrast filter with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(Contrast::new(*value))));
        }
    } else if argument == "resize" {
        let values = args.get_many::<String>("resize").unwrap();
        let indices = args.indices_of("resize").unwrap();

        let resizing_method = args
            .get_one::<IResizeMethod>("resize-method")
            .map(|c| c.to_resize_method())
            .unwrap_or(ResizeMethod::Bicubic);

        for (value, idx) in values.zip(indices) {
            match parse_geometry(value) {
                Ok(resize_dims) => {
                    debug!("Parsed resize op with params: {value}, method: {resizing_method:?} at {idx}");
                    parsed_ops.push((idx, Box::new(Resize::new(resize_dims, resizing_method))));
                }
                Err(e) => return Err(e),
            }
        }
    } else if argument == "depth" {
        let values = args.get_many::<u8>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            let depth = match value {
                8 => BitDepth::Eight,
                16 => BitDepth::Sixteen,
                32 => BitDepth::Float32,
                _ => return Err(format!("Unknown depth value {value}, supported depths are 8 and 16")),
            };
            debug!("Parsed depth operation with depth of {value} at {idx}");
            parsed_ops.push((idx, Box::new(Depth::new(depth))));
        }
    } else if argument == "colorspace" {
        let values = args.get_many::<String>("colorspace").unwrap();
        let indices = args.indices_of("colorspace").unwrap();
        for (colorspace_str, idx) in values.zip(indices) {
            let colorspace = parse_colorspace(colorspace_str)?;
            debug!("Parsed colorspace conversion to {colorspace:?} at {idx}");
            parsed_ops.push((idx, Box::new(ColorspaceConv::new(colorspace))));
        }
    } else if argument == "auto-orient" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed auto orient operation at {idx}");
                parsed_ops.push((idx, Box::new(AutoOrient)));
            }
        }
    } else if argument == "exposure" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (exposure, idx) in values.zip(indices) {
            debug!("Parsed exposure argument with value {exposure} at {idx}");
            parsed_ops.push((idx, Box::new(Exposure::new(*exposure, 0.))));
        }
    } else if argument == "v-flip" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed v-flip argument at {idx}");
                parsed_ops.push((idx, Box::new(Flip::new(FlipDirection::Vertical))));
            }
        }
    } else if argument == "huerotate" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed hue-rotate argument with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(HsvAdjust::new(*value, 1f32, 1f32))));
        }
    } else if argument == "saturate" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed saturate argument with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(HsvAdjust::new(0f32, *value, 1f32))));
        }
    } else if argument == "lightness" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed lightness argument with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(HsvAdjust::new(0f32, 1f32, *value))));
        }
    } else if argument == "rotate" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            debug!("Parsed rotate argument with value {value} at {idx}");
            parsed_ops.push((idx, Box::new(Rotate::new(*value))));
        }
    } else if argument == "composite" {
        if let Some(values) = args.get_many::<String>("composite") {
            let indices = args.indices_of("composite").unwrap();
            for (method_str, idx) in values.zip(indices) {
                let method = match method_str.as_str() {
                    "Over" => CompositeMethod::Over,
                    "Src" => CompositeMethod::Src,
                    "Dst" => CompositeMethod::Dst,
                    "DstIn" => CompositeMethod::DstIn,
                    "DstOut" => CompositeMethod::DstOut,
                    "Screen" => CompositeMethod::Screen,
                    "Xor" => CompositeMethod::Xor,
                    "Multiply" => CompositeMethod::Multiply,
                    "SrcIn" => CompositeMethod::SrcIn,
                    "SrcOut" => CompositeMethod::SrcOut,
                    _ => return Err("Unknown composite method".to_string()),
                };

                // NOTE: Grabs single global geometry for simplicity
                let position = if let Some(geo_str) = args.get_one::<String>("geometry") {
                    let parts: Vec<&str> = geo_str.split(',').collect();
                    if parts.len() == 2 {
                        let x = parts[0].parse().unwrap_or(0);
                        let y = parts[1].parse().unwrap_or(0);
                        (x, y)
                    } else {
                        return Err("Geometry must be in format x,y".to_string());
                    }
                } else {
                    (0, 0)
                };

                debug!("Parsed composite {method_str} at {idx}");
                parsed_ops.push((idx, Box::new(Composite::new(method, position))));
            }
        }
    } else if argument == "blend" {
        if let Some(values) = args.get_many::<f32>("blend") {
            let indices = args.indices_of("blend").unwrap();
            for (&alpha, idx) in values.zip(indices) {
                if !(0.0..=1.0).contains(&alpha) {
                    log::warn!("Blend alpha {} is outside the standard 0.0-1.0 range", alpha);
                }
                debug!("Parsed blend at {idx}");
                parsed_ops.push((idx, Box::new(Blend::new(alpha))));
            }
        }
    } else if argument == "append" {
        if let Some(values) = args.get_many::<String>("append") {
            let indices = args.indices_of("append").unwrap();
            for (direction_str, idx) in values.zip(indices) {
                let direction = match direction_str.as_str() {
                    "horizontal" => AppendDirection::Horizontal,
                    "vertical" => AppendDirection::Vertical,
                    _ => unreachable!(),
                };
                info!("Parsed append with direction {:?} at {}", direction, idx);
                parsed_ops.push((idx, Box::new(Append::new(direction))));
            }
        }
    } else if argument == "ssim" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                info!("Parsed ssim operation at {idx}");
                parsed_ops.push((idx, Box::new(SsimDetection::new())));
            }
        }
    } else if argument == "average" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                info!("Parsed average operation at {idx}");
                parsed_ops.push((idx, Box::new(AverageSequence::new())));
            }
        }
    } else if argument == "swap" {
        let values: Vec<&usize> = args.get_many::<usize>("swap").unwrap().collect();
        let indices: Vec<usize> = args.indices_of("swap").unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(2).zip(indices.chunks(2)) {
            info!("Parsed swap operation at {}", idx_chunk[0]);
            parsed_ops.push((idx_chunk[0], Box::new(Swap::new(*chunk[0], *chunk[1]))));
        }
    } else if argument == "fx" {
        let values = args.get_many::<String>("fx").unwrap();
        let indices = args.indices_of("fx").unwrap();
        for (expression, idx) in values.zip(indices) {
            info!("Parsed fx operation at {idx}");
            parsed_ops.push((idx, Box::new(Fx::new(expression))));
        }
    }

    Ok(parsed_ops)
}

/// Parses an ImageMagick-style geometry string into a ResizeDimensions enum.
pub fn parse_geometry(values: &str) -> Result<ResizeDimensions, String> {
    // 1. Trim whitespace or hidden newlines that CLI environments sometimes pass
    let values = values.trim();

    let re = Regex::new(r"^([0-9]+)?(%)?([xX])?([0-9]+)?(%)?([!><@\^])?$")
        .map_err(|e| format!("Failed to compile regex: {e}"))?;

    let caps = re.captures(values).ok_or_else(|| {
        format!("Invalid format: '{values}'. Use WxH, WxH^, WxH!, W, xH, P%, P%xP%, or Area@.")
    })?;

    // Safely extract capture groups (using .ok() to return None if parsing fails)
    let w = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok());
    let w_pct = caps.get(2).is_some();
    let has_x = caps.get(3).is_some();
    let h = caps.get(4).and_then(|m| m.as_str().parse::<usize>().ok());
    let h_pct = caps.get(5).is_some();
    let modifier = caps.get(6).map(|m| m.as_str());

    // Handle Percentages (e.g., "50%" or "50%x75%")
    if w_pct || h_pct {
        let width_pct = w.ok_or("Missing width percentage.")?;
        let height_pct = if has_x { h.ok_or("Missing height percentage.")? } else { width_pct };
        return Ok(ResizeDimensions::Percentage(width_pct, height_pct));
    }

    // Handle Area '@' (e.g., "40000@")
    if modifier == Some("@") {
        if let Some(area) = w {
            if !has_x && h.is_none() {
                return Ok(ResizeDimensions::Area(area));
            }
        }
        return Err("Area modifier '@' requires a single number (e.g., '40000@').".into());
    }

    // Handle standard dimensions and bounds
    match (w, has_x, h) {
        // Both Width and Height provided (e.g., "1920x1080", "800x600^")
        (Some(width), true, Some(height)) => match modifier {
            Some("!") => Ok(ResizeDimensions::IgnoreAspectRatio(width, height)),
            Some("^") => Ok(ResizeDimensions::Fill(width, height)),
            Some(">") => Ok(ResizeDimensions::ShrinkToFit(width, height)),
            Some("<") => Ok(ResizeDimensions::EnlargeToFit(width, height)),
            None => Ok(ResizeDimensions::FitWithin(width, height)),
            _ => Err(format!("Invalid modifier applied to WxH: {modifier:?}")),
        },
        // Width only without 'x' (e.g., "1920")
        (Some(width), false, None) => {
            if modifier.is_some() { return Err("Modifiers require both Width and Height.".into()); }
            Ok(ResizeDimensions::WidthOnly(width))
        }
        // Width only WITH trailing 'x' (e.g., "1920x" - ImageMagick supports this)
        (Some(width), true, None) => {
            if modifier.is_some() { return Err("Modifiers require both Width and Height.".into()); }
            Ok(ResizeDimensions::WidthOnly(width))
        }
        // Height only (e.g., "x1080")
        (None, true, Some(height)) => {
            if modifier.is_some() { return Err("Modifiers require both Width and Height.".into()); }
            Ok(ResizeDimensions::HeightOnly(height))
        }
        // Diagnostic fallback: If it falls through, tell the user exactly what variables caused it
        _ => Err(format!(
            "Invalid geometry format. Input: '{values}' | Extracted -> width:{w:?}, has_x:{has_x}, height:{h:?}, modifier:{modifier:?}"
        )),
    }
}

/// Parses a string directly into a zune_core ColorSpace.
fn parse_colorspace(s: &str) -> Result<ColorSpace, String> {
    use std::num::NonZeroU32;

    let lower = s.trim().to_lowercase();

    match lower.as_str() {
        "rgb" => Ok(ColorSpace::RGB),
        "rgba" => Ok(ColorSpace::RGBA),
        "argb" => Ok(ColorSpace::ARGB),
        "bgr" => Ok(ColorSpace::BGR),
        "bgra" => Ok(ColorSpace::BGRA),
        "hsl" => Ok(ColorSpace::HSL),
        "hsv" => Ok(ColorSpace::HSV),
        "cmyk" => Ok(ColorSpace::CMYK),
        "ycbcr" => Ok(ColorSpace::YCbCr),
        "ycck" => Ok(ColorSpace::YCCK),

        // Multiple aliases for grayscale to make it user-friendly
        "luma" | "grayscale" | "gray" => Ok(ColorSpace::Luma),
        "lumaa" | "graya" => Ok(ColorSpace::LumaA),

        _ => {
            // Dynamic parsing for MultiBand images (e.g., "multiband8" or "multiband-4")
            if let Some(num_str) = lower
                .strip_prefix("multiband")
                .map(|s| s.trim_start_matches('-'))
            {
                if let Ok(n) = num_str.parse::<u32>() {
                    if let Some(nz) = NonZeroU32::new(n) {
                        return Ok(ColorSpace::MultiBand(nz));
                    }
                }
            }

            Err(format!("Unknown or unsupported colorspace: '{s}'"))
        }
    }
}
