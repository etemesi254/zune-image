/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//#![no_std]
#![macro_use]
extern crate alloc;

pub use crate::decoder::{probe_bmp, BmpDecoder};
pub use crate::errors::BmpDecoderErrors;

mod common;
mod decoder;
mod errors;
