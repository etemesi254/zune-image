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

use crate::py_enums::PyImageDepth;
use crate::py_image::PyImage;

impl PyImage {
    pub(crate) fn to_numpy_generic<'py, T>(
        &self, py: Python<'py>, expected: PyImageDepth
    ) -> PyResult<&'py PyArray3<T>>
    where
        T: Copy + Default + 'static + numpy::Element + Send
    {
        let arr = unsafe {
            let colorspace = self.image.get_colorspace();
            //PyArray3::uget_raw()
            let arr = PyArray3::<T>::new(
                py,
                [self.height(), self.width(), colorspace.num_components()],
                false
            );

            //obtain first channel
            let channels = self.image.get_frames_ref()[0].get_channels_ref(colorspace, false);
            for chan in channels {
                if chan.reinterpret_as::<T>().is_err() {
                    return Err(PyErr::new::<PyException, _>(format!(
                        "The image depth {:?} is not u8 use image.convert_depth({:?}) to convert to 8 bit \nWe do not implicitly convert to desired depth", self.image.get_depth(), expected
                    )));
                }
            }
            let reinterprets: Vec<&[T]> = channels
                .iter()
                .map(|z| z.reinterpret_as::<T>().unwrap())
                .collect();

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
            let dims = dims.unwrap();
            // check that all reinterprets' length never passes dims
            // SAFETY CHECK: DO NOT REMOVE
            for chan in &reinterprets {
                if dims != chan.len() {
                    return Err(PyErr::new::<PyException, _>(format!(
                        "[INTERNAL-ERROR]: length of one channel doesn't match the expected len={},expected={}",
                        chan.len(), dims
                    )));
                }
            }
            // check that arr dims == length
            match arr.dims()[2] {
                1 => {
                    assert_eq!(reinterprets.len(), arr.dims()[2]);
                    // convert into u8
                    // get each pixel from each channel, so we iterate per row
                    for i in 0..arr.dims()[0] {
                        for j in 0..arr.dims()[1] {
                            let idx = (i * width) + j;
                            {
                                arr.uget_raw([i, j, 0])
                                    .write(*reinterprets.get_unchecked(0).get_unchecked(idx));
                            }
                        }
                    }
                }
                2 => {
                    // convert into T
                    // get each pixel from each channel, so we iterate per row
                    // optimized to use unsafe.
                    //
                    // # SAFETY
                    //  - Unchecked memory access
                    // - We have two memory accesses we care about,
                    //    1: uget_raw, that should never matter, since, we are iterating
                    //       over arr.dims[0] and arr.dims[1],
                    //       and we know arr_dims[2] is 2, (in this particular match)
                    //   2. reinterprets.get_unchecked(0), we assert below that the length is three
                    //   3. reinterprets.get_unchecked(0).get_unchecked(idx), we assert above(just above the match)
                    //      that the overflow doesn't happen

                    // safety check, do not remove...
                    assert_eq!(reinterprets.len(), 2);
                    for i in 0..arr.dims()[0] {
                        for j in 0..arr.dims()[1] {
                            let idx = (i * width) + j;
                            arr.uget_raw([i, j, 0])
                                .write(*reinterprets.get_unchecked(0).get_unchecked(idx));
                            arr.uget_raw([i, j, 1])
                                .write(*reinterprets.get_unchecked(1).get_unchecked(idx));
                        }
                    }
                }
                3 => {
                    // convert into T
                    // get each pixel from each channel, so we iterate per row
                    // optimized to use unsafe.
                    //
                    // # SAFETY
                    //  - Unchecked memory access
                    // - We have two memory accesses we care about,
                    //    1: uget_raw, that should never matter, since, we are iterating
                    //       over arr.dims[0] and arr.dims[1],
                    //       and we know arr_dims[2] is 3, (in this particular match)
                    //   2. reinterprets.get_unchecked(0), we assert below that the length is three
                    //   3. reinterprets.get_unchecked(0).get_unchecked(idx), we assert above(just above the match)
                    //      that the overflow doesn't happen

                    // safety check, do not remove...
                    assert_eq!(reinterprets.len(), 3);
                    for i in 0..arr.dims()[0] {
                        for j in 0..arr.dims()[1] {
                            let idx = (i * width) + j;
                            arr.uget_raw([i, j, 0])
                                .write(*reinterprets.get_unchecked(0).get_unchecked(idx));
                            arr.uget_raw([i, j, 1])
                                .write(*reinterprets.get_unchecked(1).get_unchecked(idx));
                            arr.uget_raw([i, j, 2])
                                .write(*reinterprets.get_unchecked(2).get_unchecked(idx));
                        }
                    }
                }
                4 => {
                    // convert into T
                    // get each pixel from each channel, so we iterate per row
                    // optimized to use unsafe.
                    //
                    // # SAFETY
                    //  - Unchecked memory access
                    // - We have two memory accesses we care about,
                    //    1: uget_raw, that should never matter, since, we are iterating
                    //       over arr.dims[0] and arr.dims[1],
                    //       and we know arr_dims[2] is 4, (in this particular match)
                    //   2. reinterprets.get_unchecked(0), we assert below that the length is three
                    //   3. reinterprets.get_unchecked(0).get_unchecked(idx), we assert above(just above the match)
                    //      that the overflow doesn't happen

                    // safety check, do not remove...
                    assert_eq!(reinterprets.len(), 4);
                    for i in 0..arr.dims()[0] {
                        for j in 0..arr.dims()[1] {
                            let idx = (i * width) + j;
                            arr.uget_raw([i, j, 0])
                                .write(*reinterprets.get_unchecked(0).get_unchecked(idx));
                            arr.uget_raw([i, j, 1])
                                .write(*reinterprets.get_unchecked(1).get_unchecked(idx));
                            arr.uget_raw([i, j, 2])
                                .write(*reinterprets.get_unchecked(2).get_unchecked(idx));
                            arr.uget_raw([i, j, 3])
                                .write(*reinterprets.get_unchecked(3).get_unchecked(idx));
                        }
                    }
                }
                _ => {
                    assert_eq!(reinterprets.len(), arr.dims()[2]);
                    // convert into u8
                    // get each pixel from each channel, so we iterate per row
                    for i in 0..arr.dims()[0] {
                        for j in 0..arr.dims()[1] {
                            let idx = (i * width) + j;
                            for k in 0..arr.dims()[2] {
                                arr.uget_raw([i, j, k])
                                    .write(*reinterprets.get_unchecked(k).get_unchecked(idx));
                            }
                        }
                    }
                }
            }

            arr
        };
        Ok(arr)
    }
}
