/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use numpy::{PyArray2, PyArray3, PyArray4, PyUntypedArray};
use pyo3::exceptions::PyException;
use pyo3::{PyErr, PyResult, Python};
use zune_image::utils::swizzle_channels;

use crate::py_enums::ImageDepth;
use crate::py_image::Image;

impl Image {
    pub(crate) fn to_numpy_generic<'py, T>(
        &self, py: Python<'py>, expected: ImageDepth
    ) -> PyResult<&'py PyUntypedArray>
    where
        T: Copy + Default + 'static + numpy::Element + Send
    {
        let colorspace = self.image.colorspace();

        // handle anumated images
        return if self.image.is_animated() {
            // create a 4D array
            let arr = unsafe {
                PyArray4::<T>::new(
                    py,
                    [
                        self.image.frames_len(),
                        self.height(),
                        self.width(),
                        colorspace.num_components()
                    ],
                    false
                )
            };
            let single_im_frame_dims = self.height() * self.width() * colorspace.num_components();

            let mut arr_v = arr
                .try_readwrite()
                .expect("This should be safe as we own the array and haven't exposed it");
            let pix_values = arr_v.as_slice_mut().unwrap();

            for (im_chunk, frame) in pix_values
                .chunks_exact_mut(single_im_frame_dims)
                .zip(self.image.frames_ref())
            {
                swizzle_channels(frame.channels_ref(colorspace, false), im_chunk)
                    .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;
            }

            Ok(arr.as_untyped())
        } else {
            if colorspace.num_components() == 1 {
                // just one component
                let arr = unsafe { PyArray2::<T>::new(py, [self.height(), self.width()], false) };
                let mut arr_v = arr
                    .try_readwrite()
                    .expect("This should be safe as we own the array and haven't exposed it");
                let pix_values = arr_v.as_slice_mut().unwrap();

                let channels = self.image.frames_ref()[0].channels_ref(colorspace, false);
                pix_values.copy_from_slice(
                    channels[0]
                        .reinterpret_as()
                        .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?
                );

                return Ok(arr.as_untyped());
            }
            let arr = {
                //PyArray3::uget_raw()
                let arr = unsafe {
                    PyArray3::<T>::new(
                        py,
                        [self.height(), self.width(), colorspace.num_components()],
                        false
                    )
                };

                //obtain first channel
                let channels = self.image.frames_ref()[0].channels_ref(colorspace, false);
                for chan in channels {
                    if chan.reinterpret_as::<T>().is_err() {
                        return Err(PyErr::new::<PyException, _>(format!(
                            "The image depth {:?} is not u8 use image.convert_depth({:?}) to convert to 8 bit \nWe do not implicitly convert to desired depth", self.image.depth(), expected
                        )));
                    }
                }

                let width = self.width();
                let height = self.height();

                let dims = height.checked_mul(width);
                // check for overflow
                if dims.is_none() {
                    return Err(PyErr::new::<PyException, _>(format!(
                        "width * height overflowed to big of dimensions ({width},{height})"
                    )));
                }
                let mut arr_v = arr
                    .try_readwrite()
                    .expect("This should be safe as we own the array and haven't exposed it");
                let pix_values = arr_v.as_slice_mut().unwrap();

                swizzle_channels(channels, pix_values)
                    .map_err(|x| PyErr::new::<PyException, _>(format!("{x:?}")))?;

                arr
            };
            Ok(arr.as_untyped())
        };
    }
}
