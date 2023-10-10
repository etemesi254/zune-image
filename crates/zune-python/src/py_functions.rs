/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use zune_core::result::DecodingResult;

use crate::py_enums::PyImageFormats;

#[pyfunction]
pub fn decode_jpeg(bytes: &[u8]) -> PyResult<Vec<u8>> {
    zune_jpeg::JpegDecoder::new(bytes)
        .decode()
        .map_err(|x| PyErr::new::<PyException, _>(x.to_string()))
}

#[pyfunction]
pub fn decode_png(bytes: &[u8]) -> PyResult<Vec<u8>> {
    return match zune_png::PngDecoder::new(bytes).decode() {
        Ok(result) => match result {
            DecodingResult::U8(b) => Ok(b),
            DecodingResult::U16(b) => {
                // for u16,we will convert it to u8, causing another allocation
                // I'm sorrt
                Ok(b.iter().map(|x| *x as u8).collect::<Vec<u8>>())
            }
            _ => unreachable!()
        },
        Err(e) => Err(PyErr::new::<PyException, _>(format!("{:?}", e)))
    };
}

#[pyfunction]
pub fn guess_format(bytes: &[u8]) -> PyResult<PyImageFormats> {
    match zune_image::codecs::guess_format(bytes) {
        Some((format, _)) => Ok(PyImageFormats::from(format)),
        None => Ok(PyImageFormats::Unknown)
    }
}
