/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
#![warn(
    clippy::correctness,
    clippy::perf,
    clippy::pedantic,
    clippy::inline_always,
    clippy::missing_errors_doc,
    clippy::panic
)]
#![allow(
    clippy::needless_return,
    clippy::similar_names,
    clippy::inline_always,
    clippy::similar_names,
    clippy::doc_markdown,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::wildcard_imports
)]
use py_functions::*;
use py_image::*;
use pyo3::prelude::*;

use crate::py_enums::{ColorSpace, ImageDepth, ImageFormat, ImageThresholdType};

mod py_enums;
mod py_functions;
mod py_image;

/// Initialize the logger.
///
/// This allows the library to print information about operations
/// status in the library
#[pyfunction]
#[allow(unused_results)]
pub fn init_logger() {
    let _ = pyo3_log::try_init();
}

#[pymodule]
#[pyo3(name = "zil")]
fn zune_image(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<ImageFormat>()?;
    m.add_class::<ColorSpace>()?;
    m.add_class::<Image>()?;
    m.add_class::<ImageDepth>()?;
    m.add_class::<ImageThresholdType>()?;
    m.add_function(wrap_pyfunction!(init_logger, m)?)?;

    m.add_function(wrap_pyfunction!(guess_format, m)?)?;
    m.add_function(wrap_pyfunction!(imread, m)?)?;

    Ok(())
}
