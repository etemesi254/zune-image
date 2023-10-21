/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_image::errors::ImageErrors;

mod ocl_img;
pub mod ocl_sobel;

fn propagate_ocl_error(error: ocl::Error) -> ImageErrors {
    let message = format!("OCL_ERROR:\n{}", error);
    ImageErrors::GenericString(message)
}
