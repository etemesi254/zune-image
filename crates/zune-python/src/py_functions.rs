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
use zune_core::bit_depth::{BitDepth, ByteEndian};
use zune_core::bytestream::{ZCursor, ZReader};
use zune_core::colorspace::ColorSpace;
use zune_core::result::DecodingResult;
use zune_image::codecs::bmp::BmpDecoder;
use zune_image::codecs::farbfeld::FarbFeldDecoder;
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
pub fn guess_format(bytes: &[u8]) -> ImageFormat {
    match zune_image::codecs::guess_format(ZCursor::new(bytes)) {
        Some((format, _)) => ImageFormat::from(format),
        None => ImageFormat::Unknown
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
                "Unimplemented color components {e}"
            )))
        };
    }
}

fn decode_result(
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
/// - PNG files: Always supported, 8-bit and 16-bit images
/// - PPM: PPM, PFM, PNM: Always supported
/// - PSD files: Supported, the decoder extracts the raw pixels and doesn't attempt to do any layer blending
/// - FarbFeld: Always supported
/// - Qoi: Always supported
/// - HDR: Always supported
/// - JXL: Always supported,output is in f32 between, Library for decoding is  https://github.com/tirr-c/jxl-oxide
///
/// # Animated images
/// - Animated PNG (APNG): The decoder will only decode the first frame
/// - Animated JXL : The decoder will only decoded the first frame
///
/// To fully decode animated images,  `Image.open()` + `image.to_numpy()` which will return a 4D array of
///    `[frames,height,width,image_channels]`
///
/// # Additional notes
///
/// - The function determines the type of an image by content and not file extension
/// - In the case of color images, decoded images have channels in RGB order
/// - EXIF orientation is not taken into account, for that, use Image::decode() + image.auto_orient() function
#[pyfunction]
#[allow(clippy::too_many_lines)]
pub fn imread(py: Python<'_>, file: String) -> PyResult<&PyUntypedArray> {
    // some functions will allocate only once, e.g jpeg and qoi as they have
    // non-complicated decode_into apis
    //
    // others like psd and ppm have two allocations one for bytes and the other for numpy array
    // since it makes it easier to handle multiple return types
    return match read(file) {
        Ok(bytes) => {
            let format = guess_format(&bytes);

            match format {
                ImageFormat::PNG => {
                    // note: PNG has 8 bit and 16 bit images, it's a common format so we have to do some optimizations
                    //
                    // we don't strip 16 bit to 8 bit automatically, so we need to  handle that path
                    // but we have `decode_into` only taking &[u8] slices, and making it generic and sucks
                    //
                    // so we branch on the depth, cheat a bit on 16 bit and return whatever we can
                    //
                    let mut decoder = PngDecoder::new(ZCursor::new(&bytes));
                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    let (width, height) = decoder.dimensions().unwrap();
                    let color = decoder.colorspace().unwrap();

                    match decoder.depth().unwrap() {
                        BitDepth::Eight => {
                            let arr = PyArray3::<u8>::zeros(
                                py,
                                [height, width, color.num_components()],
                                false
                            );
                            let mut write_array = arr.try_readwrite()?;
                            let slice = write_array.as_slice_mut()?;

                            decoder
                                .decode_into(slice)
                                .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                            return Ok(arr.as_untyped());
                        }
                        BitDepth::Sixteen => {
                            let arr = PyArray3::<u16>::zeros(
                                py,
                                [height, width, color.num_components()],
                                false
                            );
                            let mut write_array = arr.try_readwrite()?;
                            let slice = write_array.as_slice_mut()?;
                            // safety:
                            // we can alias strong types to weak types, e.g u16->u8 works, we only care
                            // about alignment so it should be fine
                            //
                            // Reason:
                            // Saves us an unnecessary image allocation which is expensive
                            let (a, b, c) = unsafe { slice.align_to_mut::<u8>() };
                            assert_eq!(a.len(), 0);
                            assert_eq!(c.len(), 0);
                            assert_eq!(b.len(), width * height * color.num_components() * 2);
                            // set sample endianness to match platform
                            #[cfg(target_endian = "little")]
                            {
                                let options = decoder.options().set_byte_endian(ByteEndian::LE);
                                decoder.set_options(options);
                            }
                            #[cfg(target_endian = "big")]
                            {
                                let options = decoder.get_options().set_byte_endian(ByteEndian::BE);
                                decoder.set_options(options);
                            }

                            decoder
                                .decode_into(b)
                                .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                            return Ok(arr.as_untyped());
                        }
                        _ => unreachable!()
                    }
                }
                ImageFormat::JPEG => {
                    let mut decoder = JpegDecoder::new(ZCursor::new(&bytes));

                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    let (w, h) = decoder.dimensions().unwrap();
                    let color = decoder.output_colorspace().unwrap();

                    let arr = PyArray3::<u8>::zeros(py, [h, w, color.num_components()], false);
                    let mut write_array = arr.try_readwrite()?;

                    decoder
                        .decode_into(write_array.as_slice_mut()?)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    return Ok(arr.as_untyped());
                }
                ImageFormat::BMP => {
                    let mut decoder = BmpDecoder::new(ZCursor::new(&bytes));

                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;
                    let (w, h) = decoder.dimensions().unwrap();

                    let color = decoder.colorspace().unwrap();

                    let arr = PyArray3::<u8>::zeros(py, [h, w, color.num_components()], false);

                    let mut write_array = arr.try_readwrite()?;

                    decoder
                        .decode_into(write_array.as_slice_mut()?)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    return Ok(arr.as_untyped());
                }
                ImageFormat::PPM => {
                    let mut decoder = PPMDecoder::new(ZCursor::new(&bytes));
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;
                    let colorspace = decoder.colorspace().unwrap();
                    let (w, h) = decoder.dimensions().unwrap();

                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::PSD => {
                    // this will alloc twice:
                    //
                    // we can make it do it once, but it's an uncommon format so we
                    // should be okay with the speed hit
                    let mut decoder = PSDDecoder::new(ZCursor::new(&bytes));
                    let bytes = decoder
                        .decode()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;
                    let colorspace = decoder.colorspace().unwrap();
                    let (w, h) = decoder.dimensions().unwrap();

                    decode_result(py, bytes, w, h, colorspace)
                }
                ImageFormat::FarbFeld => {
                    let mut decoder = FarbFeldDecoder::new(ZCursor::new(&bytes));

                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    let (w, h) = decoder.dimensions().unwrap();
                    let color = decoder.colorspace();

                    let arr = PyArray3::<u16>::zeros(py, [h, w, color.num_components()], false);
                    let mut write_array = arr.try_readwrite()?;

                    decoder
                        .decode_into(write_array.as_slice_mut()?)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    return Ok(arr.as_untyped());
                }
                ImageFormat::Qoi => {
                    let mut decoder = QoiDecoder::new(ZCursor::new(&bytes));

                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    let (w, h) = decoder.dimensions().unwrap();
                    let color = decoder.colorspace().unwrap();

                    let arr = PyArray3::<u8>::zeros(py, [h, w, color.num_components()], false);

                    let mut write_array = arr.try_readwrite()?;

                    decoder
                        .decode_into(write_array.as_slice_mut()?)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    return Ok(arr.as_untyped());
                }
                ImageFormat::HDR => {
                    let mut decoder = HdrDecoder::new(ZCursor::new(&bytes));

                    decoder
                        .decode_headers()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;
                    let (w, h) = decoder.dimensions().unwrap();

                    let color = decoder.get_colorspace().unwrap();

                    let arr = PyArray3::<f32>::zeros(py, [h, w, color.num_components()], false);

                    let mut write_array = arr.try_readwrite()?;

                    decoder
                        .decode_into(write_array.as_slice_mut()?)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    return Ok(arr.as_untyped());
                }
                ImageFormat::JPEG_XL => {
                    let c = ZReader::new(ZCursor::new(&bytes));

                    let decoder = zune_image::codecs::jpeg_xl::jxl_oxide::JxlImage::builder()
                        .read(c)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                    let (w, h) = (decoder.width() as usize, decoder.height() as usize);
                    let color = decoder.pixel_format();

                    let render = decoder
                        .render_frame(0)
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x}")))?;

                    // get the images
                    let im_plannar = render.image();

                    if color.channels() == 1 {
                        let arr = PyArray2::zeros(py, [h, w], false);
                        arr.try_readwrite()?
                            .as_slice_mut()?
                            .copy_from_slice(im_plannar.buf());

                        Ok(arr.as_untyped())
                    } else {
                        let arr = PyArray3::zeros(py, [h, w, color.channels()], false);
                        arr.try_readwrite()?
                            .as_slice_mut()?
                            .copy_from_slice(im_plannar.buf());

                        Ok(arr.as_untyped())
                    }
                }
                d @ ImageFormat::Unknown => Err(PyErr::new::<PyException, _>(format!(
                    " No decoder for format {:?}",
                    d.to_imageformat()
                )))
            }
        }
        Err(e) => Err(PyErr::new::<PyException, _>(format!("{e}")))
    };
}
