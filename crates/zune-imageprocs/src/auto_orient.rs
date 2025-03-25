/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Perform auto orientation of the image
//!
//! This uses the exif orientation tag of an image if it has
//! it requires the `metadata` feature in order to read exif tags, otherwise
//! this is a no-op.
#![allow(unused_variables, unused_imports)]
use zune_core::bit_depth::BitType;
use zune_core::log::warn;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::flip::{Flip, FlipDirection};
use crate::rotate::Rotate;
use crate::transpose::Transpose;

/// Auto orient the image based on the exif metadata
///
/// This operation is a no-op if `metadata` feature is not specified
/// in the crate level docs
///
/// This operation is also a no-op if the image does not have
/// exif metadata
///
/// If orientation is applied, it will also modify the exif tag to indicate
/// the image was oriented
pub struct AutoOrient;

impl OperationsTrait for AutoOrient {
    fn name(&self) -> &'static str {
        "Auto orient"
    }

    #[allow(unused_variables)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        // check if we have exif orientation metadata and transform it
        // to be this orientation

        #[cfg(feature = "exif")]
        {

            use exif::{Tag, Value};

            if let Some(data) = image.metadata().clone().exif() {
                'rotator:for field in data {
                    // look for the orientation tag
                    if field.tag == Tag::Orientation {
                        match &field.value {
                            Value::Short(bytes) => {
                                if bytes.is_empty() {
                                    warn!("The exif value is empty, cannot orient");
                                    return Ok(());
                                }
                                let byte = bytes[0];
                                match byte {
                                    1 => (), // orientation is okay
                                    2 => {
                                        Flip::new(FlipDirection::Horizontal).execute(image)?;
                                    }

                                    3 => {
                                        Rotate::new(180.0).execute(image)?;
                                    }
                                    4 => {
                                        Flip::new(FlipDirection::Vertical).execute(image)?;
                                    }
                                    5 => {
                                        Transpose::new().execute_impl(image)?;
                                    }
                                    6 => {
                                        Rotate::new(90.0).execute(image)?;
                                    }
                                    7 => {
                                        Rotate::new(270.0).execute(image)?;
                                        Flip::new(FlipDirection::Horizontal).execute(image)?;
                                    }
                                    8 => {
                                        Rotate::new(270.0).execute(image)?;
                                    }

                                    _ => {
                                        warn!(
                                            "Unknown exif orientation tag {:?}, ignoring it",
                                            &field.value
                                        );
                                    }
                                }
                                break 'rotator;
                            }
                            _ => {
                                warn!("Invalid exif orientation type, ignoring it");
                            }
                        }
                    }
                }
            }
            // update exif
            if let Some(data) = image.metadata_mut().exif_mut() {
                for field in data {
                    // set orientation to do nothing
                    if field.tag == Tag::Orientation {
                        field.value = Value::Byte(vec![1]);
                    }
                }
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U16, BitType::U8, BitType::F32]
    }
}
