/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use clap::ArgMatches;
use log::debug;
use zune_image::pipelines::Pipeline;
use zune_imageprocs::affine::AffineTransform;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::box_blur::BoxBlur;
use zune_imageprocs::color_transform::{ColorProfiles, ColorTransform};
use zune_imageprocs::convolve::Convolve;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::hald_clut::HaldClut;
use zune_imageprocs::median::Median;
use zune_imageprocs::scharr::Scharr;
use zune_imageprocs::sharpen::Sharpen;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::spatial::SpatialOps;
use zune_imageprocs::spatial_ops::SpatialOperations;
//use zune_opencl::ocl_sobel::OclSobel;

pub fn parse_options(
    workflow: &mut Pipeline, argument: &str, args: &ArgMatches,
) -> Result<(), String> {
    if argument == "box-blur" {
        let radius = *args.get_one::<usize>(argument).unwrap();
        debug!("Added box blur filter with radius {radius}");

        let box_blur = BoxBlur::new(radius);
        workflow.chain_operations(Box::new(box_blur));
    } else if argument == "blur" {
        let sigma = *args.get_one::<f32>(argument).unwrap();
        debug!("Added gaussian blur filter with radius {sigma}");

        let gaussian_blur = GaussianBlur::new(sigma);
        workflow.chain_operations(Box::new(gaussian_blur));
    } else if argument == "sharpen" {
        // parse first one as threshold
        let values: Vec<f32> = args.get_many::<f32>(argument).unwrap().copied().collect();
        let sigma_f32 = values[0];
        let threshold_u16 = values[1];
        let percentage = values[2].clamp(0.0, 100.0) as u8;

        debug!("Added unsharpen filter with sigma={sigma_f32} and threshold={threshold_u16}");

        let sharpen = Sharpen::new(sigma_f32, threshold_u16 as u16, percentage);
        workflow.chain_operations(Box::new(sharpen));
    } else if argument == "mean-blur" {
        let radius = *args.get_one::<usize>(argument).unwrap();
        debug!("Added mean blur filter with radius {radius}");

        let mean_blur = SpatialOps::new(radius, SpatialOperations::Mean);
        workflow.chain_operations(Box::new(mean_blur));
    } else if argument == "sobel" {
        debug!("Added sobel filter");
        workflow.chain_operations(Box::new(Sobel::new()));
    } else if argument == "scharr" {
        debug!("Added scharr filter");
        workflow.chain_operations(Box::new(Scharr::new()));
    } else if argument == "convolve" {
        debug!("Adding convolution filter");

        let values: Vec<f32> = args
            .get_many::<f32>(argument)
            .unwrap()
            .collect::<Vec<&f32>>()
            .iter()
            .map(|x| **x)
            .collect();

        workflow.chain_operations(Box::new(Convolve::new(values, 1.0)));
    } else if argument == "median-blur" {
        let radius = *args.get_one::<usize>(argument).unwrap();

        let blur = Median::new(radius);
        debug!("Added median blur with  radius of {radius}");
        workflow.chain_operations(Box::new(blur));
    } else if argument == "color-transform" {
        let value = args.get_one::<String>(argument).unwrap();

        let color_profile = match value.to_lowercase().as_str() {
            "rgb" => ColorProfiles::sRGB,
            "adobe-rgb" => ColorProfiles::AdobeRgb,
            "display-p3" => ColorProfiles::DisplayP3,
            "bt-2020" => ColorProfiles::DisplayP3,
            _ => Err(format!("Unknown color profile: {value}"))?,
        };
        debug!("Added color transform operation");

        let transform = ColorTransform::new(color_profile);
        workflow.chain_operations(Box::new(transform));
    } else if argument == "affine-transform" {
        let value = args
            .get_many::<f32>(argument)
            .unwrap()
            .collect::<Vec<&f32>>();
        debug!("Added affine transform operation");
        if value.len() != 6 {
            return Err(format!("Invalid transform length: {}", value.len()));
        }
        let transform = AffineTransform::new(
            *value[0], *value[1], *value[2], *value[3], *value[4], *value[5],
        );
        workflow.chain_operations(Box::new(transform));
    } else if argument == "bilateral" {
        let values = args.get_one::<String>(argument).unwrap();
        match parse_bilateral(values) {
            Ok(filter) => {
                debug!("Added bilateral filter operation",);
                workflow.chain_operations(Box::new(filter));
            }
            Err(e) => {
                return Err(e);
            }
        }
    } else if argument == "hald-clut" {
        workflow.chain_operations(Box::new(HaldClut::new()));
    }

    Ok(())
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
