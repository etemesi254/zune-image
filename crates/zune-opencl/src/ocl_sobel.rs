/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use std::sync::Mutex;

use ocl::{OclPrm, ProQue};
use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;

use crate::propagate_ocl_error;

unsafe fn ocl_deriv_generic<T: OclPrm + Copy + bytemuck::Pod>(
    ocl_pq: &ocl::ProQue, name: &'static str, ref_channel: &Channel, mut_channel: &mut Channel,
    dims: (usize, usize)
) -> Result<(), ImageErrors> {
    // Create input and output buffers
    let input_image: ocl::Buffer<T> = ocl_pq
        .buffer_builder()
        .flags(ocl::MemFlags::READ_ONLY)
        .build()
        .map_err(propagate_ocl_error)?;

    input_image
        .write(ref_channel.reinterpret_as()?)
        .enq()
        .map_err(propagate_ocl_error)?;
    // define output image
    let output_image: ocl::Buffer<T> = ocl_pq
        .buffer_builder()
        .flags(ocl::MemFlags::WRITE_ONLY)
        .build()
        .map_err(propagate_ocl_error)?;

    // Define the constant value
    // Enqueue the kernel
    ocl_pq
        .kernel_builder(name)
        .arg(&input_image)
        .arg(&output_image)
        .arg(dims.0 as i32)
        .arg(dims.1 as i32)
        .build()
        .map_err(propagate_ocl_error)?
        .enq()
        .map_err(propagate_ocl_error)?;

    output_image
        .read(mut_channel.reinterpret_as_mut()?)
        .enq()
        .map_err(propagate_ocl_error)?;

    Ok(())
}

/// Sobel OpenCL filter.
///
/// # Warning
/// This filter may be slower than the normal `Sobel` filter
/// please and it may not work on all
/// platforms.
///
/// Please test/benchmark before using this filter
///
///
/// # Example
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::traits::OperationsTrait;
/// use zune_opencl::ocl_sobel::OclSobel;
/// // create an image with color type  RGB 100x1000
/// let mut image = zune_image::image::Image::fill(100_u8,
///     ColorSpace::RGB, 100, 100).unwrap();
/// // execute
/// OclSobel::try_new().unwrap().execute(&mut image).unwrap();
///
/// ```
pub struct OclSobel {
    // protect by mutex in oder to get
    // interior mutability, we need to set_dims
    // in execute_impl which takes an immutable
    // reference
    pq: std::sync::Mutex<ocl::ProQue>
}

impl OclSobel {
    /// Try to create a new sobel filter.
    ///
    /// This invokes the opencl compiler and it's done
    /// outside init to allow OclSobel to be reused on multiple images
    /// without recompiling the kernel.
    ///
    /// # Returns.
    /// - Ok(OclSobel): OpenCL sobel kernel runner.
    /// - Err(e):  Compiling opencl kernel raised an error. or for some reason
    /// we can't build
    pub fn try_new() -> Result<Self, ImageErrors> {
        let ocl_pq = ProQue::builder()
            .src(include_str!("./open_cl/ocl_sobel.cl"))
            .build()
            .map_err(propagate_ocl_error)?;

        Ok(OclSobel {
            pq: Mutex::new(ocl_pq)
        })
    }
}

impl zune_image::traits::OperationsTrait for OclSobel {
    fn name(&self) -> &'static str {
        "OCL Sobel"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let depth = image.depth();
        let dims = image.dimensions();
        let b_type = image.depth();

        let mut ocl_pq = self.pq.lock().map_err(|x| {
            let message = format!("Could not unlock mutex:\n{}", x);
            ImageErrors::GenericString(message)
        })?;

        ocl_pq.set_dims(dims);

        for channel in image.channels_mut(true) {
            let mut mut_channel = Channel::new_with_bit_type(channel.len(), b_type.bit_type());
            unsafe {
                match depth.bit_type() {
                    BitType::U8 => {
                        ocl_deriv_generic::<u8>(
                            &ocl_pq,
                            "SobelFilterU8",
                            channel,
                            &mut mut_channel,
                            dims
                        )?;
                    }
                    BitType::U16 => {
                        ocl_deriv_generic::<u16>(
                            &ocl_pq,
                            "SobelFilterU16",
                            channel,
                            &mut mut_channel,
                            dims
                        )?;
                    }
                    BitType::F32 => {
                        ocl_deriv_generic::<f32>(
                            &ocl_pq,
                            "SobelFilterF32",
                            channel,
                            &mut mut_channel,
                            dims
                        )?;
                    }

                    d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
                }
            }
            *channel = mut_channel;
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

#[test]
#[cfg(feature = "tests")]
fn test_ocr_sobel() {
    use zune_core::colorspace::ColorSpace;
    use zune_image::traits::OperationsTrait;
    // test for all supported bit types and whether they work.
    let mut image = Image::fill(100_u8, ColorSpace::RGB, 100, 100).unwrap();
    let ocl_sobel = OclSobel::try_new().unwrap();

    for d_type in ocl_sobel.supported_types() {
        image.convert_depth(d_type.to_depth()).unwrap();
        ocl_sobel.clone_and_execute(&image).unwrap();
    }
}
