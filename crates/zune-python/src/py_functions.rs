/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::fs::read;

use numpy::{PyArray2, PyArray3, PyUntypedArray};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
use zune_image::codecs::bmp::BmpDecoder;
use zune_image::codecs::hdr::HdrDecoder;
use zune_image::codecs::jpeg::JpegDecoder;
use zune_image::codecs::png::PngDecoder;
use zune_image::codecs::ppm::PPMDecoder;
use zune_image::codecs::psd::PSDDecoder;
use zune_image::codecs::qoi::QoiDecoder;

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

fn to_numpy_from_bytes<'py, T: Copy + Default + 'static + numpy::Element + Send>(
    py: Python<'py>, data: &[T], width: usize, height: usize, color: ColorSpace
) -> PyResult<&'py PyUntypedArray> {
    debug_assert_eq!(data.len(), width * height * color.num_components());

    unsafe {
        return match color.num_components() {
            1 => {
                let arr = PyArray2::<T>::new(py, [height, width], false);
                let mut write_array = arr.try_readwrite()?;
                write_array.as_slice_mut()?.copy_from_slice(data);

                Ok(arr.as_untyped())
            }
            2 => {
                let arr = PyArray3::<T>::new(py, [height, width, color.num_components()], false);
                //
                let mut write_array = arr.try_readwrite()?;
                write_array.as_slice_mut()?.copy_from_slice(data);
                Ok(arr.as_untyped())
            }
            3 => {
                let arr = PyArray3::<T>::new(py, [height, width, color.num_components()], false);
                //
                let mut write_array = arr.try_readwrite()?;
                write_array.as_slice_mut()?.copy_from_slice(data);
                Ok(arr.as_untyped())
            }
            4 => {
                let arr = PyArray3::<T>::new(py, [height, width, color.num_components()], false);

                let mut write_array = arr.try_readwrite()?;
                write_array.as_slice_mut()?.copy_from_slice(data);
                Ok(arr.as_untyped())
            }
            e => Err(PyErr::new::<PyException, _>(format!(
                "Unimplemented color components {}",
                e
            )))
        };
    }
}

pub fn decode_result(
    py: Python, result: DecodingResult, width: usize, height: usize, colorspace: ColorSpace
) -> PyResult<&PyUntypedArray> {
    return match result {
        DecodingResult::U8(b) => to_numpy_from_bytes(py, &b, width, height, colorspace),
        DecodingResult::U16(b) => to_numpy_from_bytes(py, &b, width, height, colorspace),
        DecodingResult::F32(b) => to_numpy_from_bytes(py, &b, width, height, colorspace),
        _ => Err(PyErr::new::<PyException, _>(
            "Unimplemented color result type"
        ))
    };
}

/// Loads an image from a file returning a numpy array containing pixels
///
///
/// If the function cannot decode pixels due to an error or an unsupported image, it
/// returns an error.
///
/// The return type of this is determined by the pixel types, e.g 16 bit png will
/// return numpy.dtype=uint16, HDR will always return float32 and jpeg will always return uint8
///
/// # Supported formats
///
/// The decoder doesn't depend on external libraries to decode, so support for below
/// formats should not vary depending on the current platform configuration
///
/// - Windows Bitmaps: (*.bmp) Always supported, currently no support for .dib files
/// - JPEG files: Always supported
/// - PNG files: Always supported
/// - PPM: PPM, PFM, PNM: Always supported
/// - PSD files: Supported, the decoder extracts the raw pixels and doesn't attempt to do any layer blending
/// - FarbFeld: Always supported
/// - Qoi: Always supported
/// - HDR: Always supported
///
/// # Additional notes
///
/// - The function determines the type of an image by content and not file extension
/// - In the case of color images, decoded images have channels in RGB order
/// - EXIF orientation is not taken into account, for that, use Image::decode() + image.auto_orient() function
#[pyfunction]
pub fn imread<'py>(py: Python<'py>, file: String) -> PyResult<&'py PyUntypedArray> {
    return match read(file) {
        Ok(bytes) => {
            let format = guess_format(&bytes)?;

            match format {
                ImageFormat::PNG => {
                    let mut decoder = PngDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();
                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::JPEG => {
                    let mut decoder = JpegDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_output_colorspace().unwrap();
                    let (w, h) = decoder.dimensions().unwrap();
                    return to_numpy_from_bytes(py, &bytes, w, h, colorspace);
                }
                ImageFormat::BMP => {
                    let mut decoder = BmpDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();
                    return to_numpy_from_bytes(py, &bytes, w, h, colorspace);
                }
                ImageFormat::PPM => {
                    let mut decoder = PPMDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();

                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::PSD => {
                    let mut decoder = PSDDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();

                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::FarbFeld => {
                    let mut decoder = PSDDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();

                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::Qoi => {
                    let mut decoder = QoiDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();

                    return to_numpy_from_bytes(py, &bytes, w, h, colorspace);
                }
                ImageFormat::HDR => {
                    let mut decoder = HdrDecoder::new(&bytes);
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;
                    let colorspace = decoder.get_colorspace().unwrap();
                    let (w, h) = decoder.get_dimensions().unwrap();

                    return to_numpy_from_bytes(py, &bytes, w, h, colorspace);
                }
                d => Err(PyErr::new::<PyException, _>(format!(
                    " No decoder for format {:?}",
                    d.to_imageformat()
                )))
            }
        }
        Err(e) => Err(PyErr::new::<PyException, _>(format!("{}", e)))
    };
}
