/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use numpy::PyArray3;
use pyo3::exceptions::PyException;
use pyo3::{PyErr, PyResult, Python};

use crate::py_enums::ImageDepth;
use crate::py_image::Image;
use crate::utils::channels_to_linear;

impl Image {
    pub(crate) fn to_numpy_generic<'py, T>(
        &self, py: Python<'py>, expected: ImageDepth
    ) -> PyResult<&'py PyArray3<T>>
    where
        T: Copy + Default + 'static + numpy::Element + Send
    {
        let arr = {
            let colorspace = self.image.colorspace();
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
                    "width * height overflowed to big of dimensions ({},{})",
                    width, height
                )));
            }
            let mut arr_v = arr
                .try_readwrite()
                .expect("This should be safe as we own the array and haven't exposed it");
            let pix_values = arr_v.as_slice_mut().unwrap();

            channels_to_linear(channels, pix_values)
                .map_err(|x| PyErr::new::<PyException, _>(format!("{:?}", x)))?;

            arr
        };
        return Ok(arr);
    }
}
