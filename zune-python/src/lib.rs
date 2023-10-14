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

use crate::py_enums::{PyImageColorSpace, PyImageDepth, PyImageFormats, PyImageThresholdType};

mod py_enums;
mod py_functions;
mod py_image;

/// A Python module implemented in Rust.
#[pymodule]
fn zune_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyImageFormats>()?;
    m.add_class::<PyImageColorSpace>()?;
    m.add_class::<PyImage>()?;
    m.add_class::<PyImageDepth>()?;
    m.add_class::<PyImageThresholdType>()?;

    m.add_function(wrap_pyfunction!(guess_format, m)?)?;
    m.add_function(wrap_pyfunction!(decode_image, m)?)?;
    m.add_function(wrap_pyfunction!(decode_file, m)?)?;

    Ok(())
}
