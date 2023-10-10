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
use zune_image::traits::OperationsTrait;
use zune_png::zune_core::options::DecoderOptions;

use crate::py_enums::PyImageErrors;

#[pyclass]
#[derive(Clone)]
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
    /// Get the image or the first frame of an image
    /// as a Python list.
    ///
    /// # Returns
    /// An array of size `width * height *colorspace` containing
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
    /// Get the image dimensions as a tuple of width and height
    ///
    /// # Returns
    /// -  A tuple in the format `(width,height)`
    pub fn dimensions(&self) -> (usize, usize) {
        self.image.get_dimensions()
    }
    /// Get the image width
    ///
    /// # Returns
    /// Image width
    pub fn width(&self) -> usize {
        self.image.get_dimensions().0
    }
    /// Get the image height
    ///
    /// # Returns
    /// Image height
    pub fn height(&self) -> usize {
        self.image.get_dimensions().1
    }
    /// Get the image colorspace
    ///
    /// # Returns
    /// - The current image colorspace
    ///
    /// # See
    /// - [convert_colorspace](PyImage::convert_colorspace) : Convert from one colorspace to another
    pub fn colorspace(&self) -> PyImageColorSpace {
        PyImageColorSpace::from(self.image.get_colorspace())
    }
    /// Convert from one colorspace to another
    ///
    /// This operation modifies the image in place
    ///
    /// # Arguments
    /// - to: The new colorspace to convert to
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    /// - Nothing on success, on error returns error that occurred
    #[pyo3(signature = (to, in_place = false))]
    pub fn convert_colorspace(
        &mut self, to: PyImageColorSpace, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        let color = to.to_colorspace();

        if in_place {
            if let Err(e) = self.image.convert_color(color) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error encoding: {:?}",
                    e
                )));
            }
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            if let Err(e) = im_clone.image.convert_color(color) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error encoding: {:?}",
                    e
                )));
            }
            Ok(Some(im_clone))
        }
    }
    /// Return the image depth
    ///
    /// # Returns
    /// - The image depth
    ///
    /// This also gives you the internal representation of an image
    ///  - Eight: u8 (1 byte per pixel)
    ///  - Sixteen: u16 (2 bytes per pixel)
    ///  - F32: Float f32 (4 bytes per pixel, float type)
    ///  
    pub fn depth(&self) -> PyImageDepth {
        PyImageDepth::from(self.image.get_depth())
    }
    /// Save an image to a format
    ///
    /// Not all image formats have encoders enabled
    /// so check by calling `PyImageFormat.has_encoder()` which returns a boolean
    /// indicating if the image format has an encoder
    ///
    /// # Arguments
    ///  - file: Filename to save the file to
    ///  - format:  The format to save the file in
    ///
    /// # Returns
    ///  - Nothing on success, or Exception  on error
    pub fn save(&self, file: String, format: PyImageFormats) -> PyResult<()> {
        if let Err(e) = self.image.save_to(file, format.to_imageformat()) {
            return Err(PyErr::new::<PyException, _>(format!(
                "Error encoding: {:?}",
                e
            )));
        }
        Ok(())
    }

    /// Crop an image
    ///
    /// # Arguments
    /// - width: Out width, how wide the new image should be
    /// - height: Out height, how tall the new image should be
    /// - x : How many pixels horizontally from the origin should the cropping start from
    /// - y : How many pixels vertically from the origin should the cropping start from.
    ///
    ///  - in_place: Whether to carry out the crop in place or create a clone for which to crop
    ///
    /// Origin is defined from the top left of the image.
    ///
    #[pyo3(signature = (width, height, x, y, in_place = false))]
    pub fn crop(
        &mut self, width: usize, height: usize, x: usize, y: usize, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        return if in_place {
            Crop::new(width, height, x, y)
                .execute(&mut self.image)
                .map_err(|x| PyImageErrors::from(x))?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            Crop::new(width, height, x, y)
                .execute(&mut im_clone.image)
                .map_err(|x| PyImageErrors::from(x))?;
            Ok(Some(im_clone))
        };
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

impl From<PyImageErrors> for pyo3::PyErr {
    fn from(value: PyImageErrors) -> Self {
        PyErr::new::<PyException, _>(format!("{:?}", value.error))
    }
}
