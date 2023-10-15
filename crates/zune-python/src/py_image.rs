/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
use std::fs::read;

use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use zune_image::filters::box_blur::BoxBlur;
use zune_image::filters::crop::Crop;
use zune_image::filters::exposure::Exposure;
use zune_image::filters::flip::Flip;
use zune_image::filters::flop::Flop;
use zune_image::filters::gamma::Gamma;
use zune_image::filters::gaussian_blur::GaussianBlur;
use zune_image::filters::invert::Invert;
use zune_image::filters::orientation::AutoOrient;
use zune_image::filters::sobel::Sobel;
use zune_image::filters::stretch_contrast::StretchContrast;
use zune_image::filters::threshold::Threshold;
use zune_image::filters::transpose::Transpose;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;
use zune_png::zune_core::options::DecoderOptions;

use crate::py_enums::{
    PyImageColorSpace, PyImageDepth, PyImageErrors, PyImageFormats, PyImageThresholdType
};

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
    /// # Arguments
    /// - to: The new colorspace to convert to
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (to, in_place = false))]
    pub fn convert_colorspace(
        &mut self, to: PyImageColorSpace, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        let color = to.to_colorspace();

        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = image.image.convert_color(color) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

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
        let exec = |image: &mut PyImage| -> PyResult<()> {
            Crop::new(width, height, x, y)
                .execute(&mut image.image)
                .map_err(|x| PyImageErrors::from(x))?;
            Ok(())
        };
        return if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;
            Ok(Some(im_clone))
        };
    }
    /// Transpose the image.
    ///
    /// This rewrites pixels into `dst(i,j)=src(j,i)`
    ///
    /// # Arguments
    /// - inplace: Whether to transpose the image in place or generate a clone
    /// and transpose the new clone
    #[pyo3(signature = (in_place = false))]
    pub fn transpose(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            Transpose::new()
                .execute(&mut image.image)
                .map_err(|x| PyImageErrors::from(x))?;
            Ok(())
        };
        return if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;
            Ok(Some(im_clone))
        };
    }

    /// Convert from one depth to another
    ///
    /// # Arguments
    /// - to: The new depth to convert to
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (to, in_place = false))]
    pub fn convert_depth(&mut self, to: PyImageDepth, in_place: bool) -> PyResult<Option<PyImage>> {
        let color = to.to_depth();

        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = image.image.convert_depth(color) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }
    /// Applies a fixed-level threshold to each array element.
    ///
    /// Thresholding works best for grayscale images, passing a colored image
    /// does not implicitly convert it to grayscale, you need to do that explicitly
    ///
    /// # Arguments
    ///  - value: Non-zero value assigned to the pixels for which the condition is satisfied
    ///  - method: The thresholding method used, defaults to binary which generates a black
    /// and white image from a grayscale image
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred

    #[pyo3(signature = (value, method = PyImageThresholdType::Binary, in_place = false))]
    pub fn threshold(
        &mut self, value: f32, method: PyImageThresholdType, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Threshold::new(value, method.to_threshold()).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }
    /// Invert (negate) an image
    ///
    /// # Arguments
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn invert(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Invert::new().execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Blur the image using a box blur operation
    ///
    /// # Arguments
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (radius, in_place = false))]
    pub fn box_blur(&mut self, radius: usize, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = BoxBlur::new(radius).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Adjust exposure of image filter
    ///
    ///#  Arguments
    /// - exposure: Set the exposure correction, allowed range is from -3.0 to 3.0. Default should be zero
    /// - black: Set black level correction: Allowed range from -1.0 to 1.0. Default is zero.
    ///
    /// For 8 bit and 16 bit images, values are clamped to their limits,
    /// for floating point, no clamping occurs
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (exposure, black_point = 0.0, in_place = false))]
    pub fn exposure(
        &mut self, exposure: f32, black_point: f32, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Exposure::new(exposure, black_point).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Creates a vertical mirror image by reflecting
    /// the pixels around the central x-axis.
    ///
    ///
    /// ```text
    ///
    ///old image     new image
    /// ┌─────────┐   ┌──────────┐
    /// │a b c d e│   │j i h g f │
    /// │f g h i j│   │e d c b a │
    /// └─────────┘   └──────────┘
    /// ```
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn flip(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Flip.execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Creates a horizontal mirror image by
    /// reflecting the pixels around the central y-axis
    ///
    ///```text
    ///old image     new image
    ///┌─────────┐   ┌──────────┐
    ///│a b c d e│   │e d b c a │
    ///│f g h i j│   │j i h g f │
    ///└─────────┘   └──────────┘
    ///```
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn flop(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Flop.execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }
    /// Gamma adjust an image
    ///
    /// This currently only supports 8 and 16 bit depth images since it applies an optimization
    /// that works for those depths.
    ///
    /// # Arguments
    /// - gamma: Ranges typical range is from 0.8-2.3
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (gamma, in_place = false))]
    pub fn gamma(&mut self, gamma: f32, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Gamma::new(gamma).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Blur the image using a gaussian blur filter
    ///
    /// # Arguments
    ///   - sigma: Strength of blur
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (sigma, in_place = false))]
    pub fn gaussian_blur(&mut self, sigma: f32, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = GaussianBlur::new(sigma).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Auto orient the image based on the exif metadata
    ///
    ///
    /// This operation is also a no-op if the image does not have
    /// exif metadata
    #[pyo3(signature = (in_place = false))]
    pub fn auto_orient(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = AutoOrient.execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Calculate the sobel derivative of an image
    ///
    /// This uses the standard 3x3 [Gx and Gy matrix](https://en.wikipedia.org/wiki/Sobel_operator)
    ///
    /// Gx matrix
    /// ```text
    ///   -1, 0, 1,
    ///   -2, 0, 2,
    ///   -1, 0, 1
    /// ```
    /// Gy matrix
    /// ```text
    /// -1,-2,-1,
    ///  0, 0, 0,
    ///  1, 2, 1
    /// ```
    ///
    ///  # Arguments
    /// - in-place: Whether to carry the operation in place or clone and operate on the copy
    #[pyo3(signature = (in_place = false))]
    pub fn sobel(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Sobel.execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }
    /// Calculate the scharr derivative of an image
    ///
    /// The image is convolved with the following 3x3 matrix
    ///
    ///
    /// Gx matrix
    /// ```text
    ///   -3, 0,  3,
    ///  -10, 0, 10,
    ///   -3, 0,  3
    /// ```
    /// Gy matrix
    /// ```text
    /// -3,-10,-3,
    ///  0,  0, 0,
    ///  3, 10, 3
    /// ```
    ///
    ///  # Arguments
    /// - in-place: Whether to carry the operation in place or clone and operate on the copy
    #[pyo3(signature = (in_place = false))]
    pub fn scharr(&mut self, in_place: bool) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = Sobel.execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
    }

    /// Linearly stretches the contrast in an image in place,
    /// sending lower to image minimum and upper to image maximum.
    ///
    /// # Arguments
    /// - lower: Lower minimum value for which pixels below this are clamped to the value
    /// - upper: Upper maximum value for which pixels above are clamped to the value
    ///
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (lower, upper, in_place = false))]
    pub fn stretch_contrast(
        &mut self, lower: u16, upper: u16, in_place: bool
    ) -> PyResult<Option<PyImage>> {
        let exec = |image: &mut PyImage| -> PyResult<()> {
            if let Err(e) = StretchContrast::new(lower, upper).execute(&mut image.image) {
                return Err(PyErr::new::<PyException, _>(format!(
                    "Error converting: {:?}",
                    e
                )));
            }
            Ok(())
        };

        if in_place {
            exec(self)?;
            Ok(None)
        } else {
            let mut im_clone = self.clone();
            exec(&mut im_clone)?;

            Ok(Some(im_clone))
        }
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

/// Decode a file path containing an image
#[pyfunction]
pub fn decode_file(file: String) -> PyResult<PyImage> {
    return match read(file) {
        Ok(bytes) => Ok(PyImage::new(
            Image::read(bytes, DecoderOptions::new_fast()).map_err(|x| PyImageErrors::from(x))?
        )),
        Err(e) => Err(PyErr::new::<PyException, _>(format!("{}", e)))
    };
}
