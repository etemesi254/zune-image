/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use py_functions::*;
use py_image::*;
use pyo3::prelude::*;

use crate::py_enums::{ZImageColorSpace, ZImageDepth, ZImageFormats, ZImageThresholdType};

mod py_enums;
mod py_functions;
mod py_image;

/// A Python module implemented in Rust.
#[pymodule]
#[pyo3(name = "zil")]
fn zune_image(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<ZImageFormats>()?;
    m.add_class::<ZImageColorSpace>()?;
    m.add_class::<ZImage>()?;
    m.add_class::<ZImageDepth>()?;
    m.add_class::<ZImageThresholdType>()?;

    m.add_function(wrap_pyfunction!(guess_format, m)?)?;
    m.add_function(wrap_pyfunction!(decode_image, m)?)?;
    m.add_function(wrap_pyfunction!(decode_file, m)?)?;

    Ok(())
}
