/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! A Portable Pixmap and Portable FloatMap Format Decoder and Encoder
//!
//! This crate supports decoding and encoding of the following ppm formats
//!
//! # Specification
//! [here](https://netpbm.sourceforge.net/doc/ppm.html)
//!
//!|Format | Decoder | Encoder |
//!|-------|--------|--------|
//!|P1-P3 | No     | No      |
//!| P5   | Yes    | Yes     |
//!| P6   | Yes    | Yes     |
//!| P7   | Yes    | Yes     |
//!| [PFM]  | Yes    | No     |
//!
//!
//![PFM]:https://www.pauldebevec.com/Research/HDR/PFM/
//!
//! # Example
//! - Decoding PPM
//!```no_run
//! use zune_ppm::PPMDecoder;
//! use zune_ppm::PPMDecodeErrors;
//! use zune_core::result::DecodingResult;
//!
//! fn main()->Result<(),PPMDecodeErrors>{
//!    use zune_core::bytestream::ZCursor;
//! let mut decoder = PPMDecoder::new(ZCursor::new(&[]));
//!    let pix = decoder.decode()?;
//!    match pix {  
//!        DecodingResult::U8(_) => {
//!            // deal with 8 bit images
//!        }
//!        DecodingResult::U16(_) => {
//!            // deal with 16 bit images
//!        }
//!        DecodingResult::F32(_) => {
//!            // deal with 32 bit images (PFM)
//!        }
//!        _=>unreachable!()};
//! Ok(())
//! }
//! ```
#![forbid(unsafe_code)]
#![no_std]
extern crate alloc;

pub use zune_core;

pub use crate::decoder::*;
pub use crate::encoder::*;

mod decoder;
mod encoder;
