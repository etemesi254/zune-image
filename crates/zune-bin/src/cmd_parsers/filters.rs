/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use log::debug;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::affine::AffineTransform;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::box_blur::BoxBlur;
use zune_imageprocs::color_transform::{ColorProfiles, ColorTransform};
use zune_imageprocs::convolve::Convolve;
use zune_imageprocs::blur::Blur;
use zune_imageprocs::hald_clut::HaldClut;
use zune_imageprocs::median::MedianBlur;
use zune_imageprocs::scharr::Scharr;
use zune_imageprocs::sharpen::Sharpen;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::spatial::SpatialOps;
use zune_imageprocs::spatial_ops::SpatialOperations;

#[allow(clippy::type_complexity)]
pub fn parse_options(
    argument: &str, args: &clap::ArgMatches,
) -> Result<Vec<(usize, Box<dyn OperationsTrait>)>, String> {
    let mut parsed_ops: Vec<(usize, Box<dyn OperationsTrait>)> = Vec::new();

    if argument == "box-blur" {
        let values = args.get_many::<usize>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (radius, idx) in values.zip(indices) {
            debug!("Parsed box blur filter with radius {radius} at {idx}");
            parsed_ops.push((idx, Box::new(BoxBlur::new(*radius))));
        }
    } else if argument == "blur" {
        let values = args.get_many::<f32>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (sigma, idx) in values.zip(indices) {
            debug!("Parsed gaussian blur filter with sigma {sigma} at {idx}");
            parsed_ops.push((idx, Box::new(Blur::new(*sigma))));
        }
    } else if argument == "sharpen" {
        let values: Vec<f32> = args.get_many::<f32>(argument).unwrap().copied().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(3).zip(indices.chunks(3)) {
            let sigma_f32 = chunk[0];
            let threshold_u16 = chunk[1];
            let percentage = chunk[2].clamp(0.0, 100.0) as u8;

            debug!("Parsed unsharpen filter with sigma={sigma_f32} and threshold={threshold_u16} at {}", idx_chunk[0]);
            parsed_ops.push((idx_chunk[0], Box::new(Sharpen::new(sigma_f32, threshold_u16 as u16, percentage))));
        }
    } else if argument == "mean-blur" {
        let values = args.get_many::<usize>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (radius, idx) in values.zip(indices) {
            debug!("Parsed mean blur filter with radius {radius} at {idx}");
            parsed_ops.push((idx, Box::new(SpatialOps::new(*radius, SpatialOperations::Mean))));
        }
    } else if argument == "sobel" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed sobel filter at {idx}");
                parsed_ops.push((idx, Box::new(Sobel::new())));
            }
        }
    } else if argument == "scharr" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed scharr filter at {idx}");
                parsed_ops.push((idx, Box::new(Scharr::new())));
            }
        }
    } else if argument == "convolve" {
        let values: Vec<f32> = args.get_many::<f32>(argument).unwrap().copied().collect();
        // Since convolve dynamically eats values, we just grab its first occurrence index
        if let Some(idx) = args.indices_of(argument).unwrap().next() {
            debug!("Parsed convolution filter at {idx}");
            parsed_ops.push((idx, Box::new(Convolve::new(values, 1.0))));
        }
    } else if argument == "median-blur" {
        let values = args.get_many::<usize>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (radius, idx) in values.zip(indices) {
            debug!("Parsed median blur with radius {radius} at {idx}");
            parsed_ops.push((idx, Box::new(MedianBlur::new(*radius))));
        }
    } else if argument == "color-transform" {
        let values = args.get_many::<String>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            let color_profile = match value.to_lowercase().as_str() {
                "rgb" => ColorProfiles::sRGB,
                "adobe-rgb" => ColorProfiles::AdobeRgb,
                "display-p3" => ColorProfiles::DisplayP3,
                "bt-2020" => ColorProfiles::DisplayP3,
                _ => return Err(format!("Unknown color profile: {value}")),
            };
            debug!("Parsed color transform operation at {idx}");
            parsed_ops.push((idx, Box::new(ColorTransform::new(color_profile))));
        }
    } else if argument == "affine-transform" {
        let values: Vec<&f32> = args.get_many::<f32>(argument).unwrap().collect();
        let indices: Vec<usize> = args.indices_of(argument).unwrap().collect();

        for (chunk, idx_chunk) in values.chunks(6).zip(indices.chunks(6)) {
            debug!("Parsed affine transform operation at {}", idx_chunk[0]);
            parsed_ops.push((
                idx_chunk[0],
                Box::new(AffineTransform::new(*chunk[0], *chunk[1], *chunk[2], *chunk[3], *chunk[4], *chunk[5]))
            ));
        }
    } else if argument == "bilateral" {
        let values = args.get_many::<String>(argument).unwrap();
        let indices = args.indices_of(argument).unwrap();
        for (value, idx) in values.zip(indices) {
            match parse_bilateral(value) {
                Ok(filter) => {
                    debug!("Parsed bilateral filter operation at {idx}");
                    parsed_ops.push((idx, Box::new(filter)));
                }
                Err(e) => return Err(e),
            }
        }
    } else if argument == "hald-clut" {
        if let Some(indices) = args.indices_of(argument) {
            for idx in indices {
                debug!("Parsed hald-clut filter at {idx}");
                parsed_ops.push((idx, Box::new(HaldClut::new())));
            }
        }
    }

    Ok(parsed_ops)
}

/// Parses a comma-separated string into a BilateralFilter.
/// Expected format: "d,sigma_color,sigma_space"
pub fn parse_bilateral(values: &str) -> Result<BilateralFilter, String> {
    let parts: Vec<&str> = values.split(',').collect();

    if parts.len() != 3 {
        return Err("Invalid format. Expected exactly 3 comma-separated values: d,sigma_color,sigma_space (e.g., '9,75.0,75.0').".to_string());
    }

    let d = parts[0].trim().parse::<i32>().map_err(|_| {
        format!(
            "Failed to parse 'd' (diameter) from '{}' as an integer.",
            parts[0]
        )
    })?;

    let sigma_color = parts[1].trim().parse::<f32>().map_err(|_| {
        format!(
            "Failed to parse 'sigma_color' from '{}' as a float.",
            parts[1]
        )
    })?;

    let sigma_space = parts[2].trim().parse::<f32>().map_err(|_| {
        format!(
            "Failed to parse 'sigma_space' from '{}' as a float.",
            parts[2]
        )
    })?;

    Ok(BilateralFilter::new(d, sigma_color, sigma_space))
}
