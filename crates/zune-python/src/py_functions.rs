/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use pyo3::prelude::*;

use crate::py_enums::ImageFormat;

/// Guess an image format from bytes
///
/// # Arguments
/// bytes: An array of bytes consisting of an encoded image
#[pyfunction]
pub fn guess_format(bytes: &[u8]) -> PyResult<ImageFormat> {
    match zune_image::codecs::guess_format(bytes) {
        Some((format, _)) => Ok(ImageFormat::from(format)),
        None => Ok(ImageFormat::Unknown)
    }
}
