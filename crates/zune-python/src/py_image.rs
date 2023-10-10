/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use zune_image::image::Image;
use zune_png::zune_core::options::DecoderOptions;

use crate::py_enums::{PyImageColorSpace, PyImageDepth, PyImageFormats};

#[pyclass]
pub struct PyImage {
    image: Image
}

impl PyImage {
    pub(crate) fn new(image: Image) -> PyImage {
        return PyImage { image };
    }
}

#[pymethods]
impl PyImage {
    pub fn to_u8(&self) -> Vec<u8> {
        let mut c = self.image.flatten_to_u8();
        let first_byte = core::mem::take(&mut c[0]);
        return first_byte;
    }
    pub fn to_u8_2d(&self) -> Vec<Vec<u8>> {
        return self.image.flatten_to_u8();
    }
    pub fn format(&self) -> PyImageFormats {
        match self.image.get_metadata().get_image_format() {
            Some(format) => PyImageFormats::from(format),
            None => PyImageFormats::Unknown
        }
    }
    pub fn dimensions(&self) -> (usize, usize) {
        self.image.get_dimensions()
    }
    pub fn width(&self) -> usize {
        self.image.get_dimensions().0
    }
    pub fn height(&self) -> usize {
        self.image.get_dimensions().1
    }
    pub fn colorspace(&self) -> PyImageColorSpace {
        PyImageColorSpace::from(self.image.get_colorspace())
    }
    pub fn depth(&self) -> PyImageDepth {
        PyImageDepth::from(self.image.get_depth())
    }
    pub fn save(&self, file: String, format: PyImageFormats) -> PyResult<()> {
        if let Err(e) = self.image.save_to(file, format.to_imageformat()) {
            return Err(PyErr::new::<PyException, _>(format!(
                "Error encoding: {:?}",
                e
            )));
        }
        Ok(())
    }
}

#[pyfunction]
pub fn decode_image(bytes: &[u8]) -> PyResult<PyImage> {
    let im_result = Image::read(bytes, DecoderOptions::new_fast());
    return match im_result {
        Ok(result) => Ok(PyImage::new(result)),
        Err(err) => Err(PyErr::new::<PyException, _>(format!(
            "Error decoding: {:?}",
            err
        )))
    };
}
