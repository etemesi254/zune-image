/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Spatial operations on images
//!
//! spatial goes through each pixel on an image collecting its neighbors and picking one
//! based on the function provided.
//!
//! The resulting image is then returned.
//! The parameter radius corresponds to the radius of the neighbor area to be searched,
//! for example a radius of R will result in a search window length of 2R+1 for each dimension.
//!
//!
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::spatial_ops::{spatial_ops, SpatialOperations};
use crate::utils::execute_on;

/// Spatial operations on images.
///
/// The parameter radius corresponds to the radius of the neighbor area the statistic is applied,
/// larger radius means more compute time.
///
/// for example a radius of R will result in a search window length of 2R+1 for each dimension.
pub struct SpatialOps {
    radius: usize,
    operation: SpatialOperations,
}

impl SpatialOps {
    #[must_use]
    pub fn new(radius: usize, operation: SpatialOperations) -> SpatialOps {
        SpatialOps { radius, operation }
    }
}

impl OperationsTrait for SpatialOps {
    fn name(&self) -> &'static str {
        "StatisticsOps Filter"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let (width, height) = image.dimensions();

        let depth = image.depth();

        let spatial_fn = |channel: &mut Channel| -> Result<(), ImageErrors> {
            let mut new_channel = Channel::new_with_bit_type(channel.len(), depth.bit_type());

            match depth.bit_type() {
                BitType::U16 => spatial_ops(
                    channel.reinterpret_as::<u16>()?,
                    new_channel.reinterpret_as_mut::<u16>()?,
                    self.radius,
                    width,
                    height,
                    self.operation,
                ),
                BitType::U8 => spatial_ops(
                    channel.reinterpret_as::<u8>()?,
                    new_channel.reinterpret_as_mut::<u8>()?,
                    self.radius,
                    width,
                    height,
                    self.operation,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
            *channel = new_channel;
            Ok(())
        };

        execute_on(spatial_fn, image, true)
    }
    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16]
    }
}
