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
use zune_image::traits::{OperationColorValues, OperationsTrait};

use crate::flip::{Flip, FlipDirection};
use crate::rotate::Rotate;
use crate::transpose::Transpose;


/// Automatically orients an image based on its EXIF metadata.
///
/// Digital cameras and smartphones often write an `Orientation` tag into the EXIF metadata
/// to indicate how the device was held when the photo was taken. Instead of altering the
/// pixel data immediately, they save the image sideways and append this tag.
///
/// This operation reads that tag and applies the necessary physical rotations and flips
/// to the underlying pixel data so that the image is correctly oriented (i.e., "right-side up").
///
/// # Behavior
///
/// * Checks the EXIF `Orientation` tag (values 1 through 8).
/// * Applies the corresponding `Rotate`, `Flip`, or `Transpose` operations.
/// * Mutates the EXIF metadata, resetting the `Orientation` tag to `1` (Normal) so that
///   subsequent saves or reads do not mistakenly double-apply the orientation.
///
/// # Feature Flags
///
/// This operation requires the `exif` feature to be enabled. If the feature is disabled,
/// if the image lacks EXIF metadata, or if the EXIF tag is missing/malformed, this
/// operation safely acts as a **no-op** and returns the image unmodified.
pub struct AutoOrient;
impl OperationsTrait for AutoOrient {
    fn name(&self) -> &'static str {
        "Auto orient"
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Any
    }
    #[allow(unused_variables)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        // check if we have exif orientation metadata and transform it
        // to be this orientation

        #[cfg(feature = "exif")]
        {
            use exif::{Tag, Value};

            if let Some(data) = image.metadata().clone().exif() {
                'rotator: for field in data {
                    // look for the orientation tag
                    if field.tag == Tag::Orientation {
                        if let Value::Short(bytes) = &field.value {
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
