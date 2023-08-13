/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Deinterleave de-interleaves image channels into separate channels
//!
//! Multiple image formats store  images in `R`,`G`,`B`,`R`,`G`,`B`...
//! format, while this is good for viewing, it's not good for image processing,
//! this library aims to be a processing library,we usually have to separate
//! pixels into different channels/planes
//!
//! i.e `RGBRGBRGBRGBRGB` becomes `RRRR`,`GGGG`,`BBBB`.
//!
//!The latter representation makes it easier for processing and it allows multi-threaded
//! post processing for scenarios where processing is slow.
use zune_core::colorspace::ColorSpace;

use crate::channel::Channel;
use crate::deinterleave::deinterleave_impls::{
    de_interleave_four_channels_f32, de_interleave_four_channels_u16,
    de_interleave_three_channels_f32, de_interleave_three_channels_u16,
    de_interleave_three_channels_u8, deinterleave_four_channels_u8
};
use crate::errors::{ImageErrors, ImageOperationsErrors};

mod avx2;
mod scalar;
mod sse2;
mod sse41;

mod deinterleave_impls;

/// Separates image u8's into various components
pub fn deinterleave_u8(
    interleaved_pixels: &[u8], colorspace: ColorSpace
) -> Result<Vec<Channel>, ImageErrors> {
    if interleaved_pixels.len() % colorspace.num_components() != 0 {
        return Err(ImageErrors::OperationsError(
            ImageOperationsErrors::InvalidChannelLayout("Extra pixels in the colorspace")
        ));
    }
    let size = interleaved_pixels.len() / colorspace.num_components();

    if colorspace.num_components() == 1 {
        let mut c1 = Channel::new_with_capacity::<u8>(size);

        c1.extend(interleaved_pixels);

        return Ok(vec![c1]);
    } else if colorspace.num_components() == 2 {
        let mut c1 = Channel::new_with_length::<u8>(size);
        let mut c2 = Channel::new_with_length::<u8>(size);

        let c1_mut = c1.reinterpret_as_mut::<u8>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u8>().unwrap();

        for ((chunk, c1), c2) in interleaved_pixels.chunks_exact(2).zip(c1_mut).zip(c2_mut) {
            *c1 = chunk[0];
            *c2 = chunk[1];
        }

        return Ok(vec![c1, c2]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3 {
        let mut c1 = Channel::new_with_length::<u8>(size);
        let mut c2 = Channel::new_with_length::<u8>(size);
        let mut c3 = Channel::new_with_length::<u8>(size);

        let c1_mut = c1.reinterpret_as_mut::<u8>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u8>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u8>().unwrap();

        de_interleave_three_channels_u8(interleaved_pixels, c1_mut, c2_mut, c3_mut);

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    } else if colorspace.num_components() == 4 {
        let mut c1 = Channel::new_with_length::<u8>(size);
        let mut c2 = Channel::new_with_length::<u8>(size);
        let mut c3 = Channel::new_with_length::<u8>(size);
        let mut c4 = Channel::new_with_length::<u8>(size);

        let c1_mut = c1.reinterpret_as_mut::<u8>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u8>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u8>().unwrap();
        let c4_mut = c4.reinterpret_as_mut::<u8>().unwrap();

        deinterleave_four_channels_u8(interleaved_pixels, c1_mut, c2_mut, c3_mut, c4_mut);

        return Ok(vec![c1, c2, c3, c4]);
    }

    todo!()
}

/// Separates u16's into various components
pub fn deinterleave_u16(
    interleaved_pixels: &[u16], colorspace: ColorSpace
) -> Result<Vec<Channel>, ImageErrors> {
    if interleaved_pixels.len() % colorspace.num_components() != 0 {
        return Err(ImageErrors::OperationsError(
            ImageOperationsErrors::InvalidChannelLayout("Extra pixels in the colorspace")
        ));
    }

    let size = (interleaved_pixels.len() / colorspace.num_components()) * 2 /*Depth is two bytes*/;

    if colorspace.num_components() == 1 {
        let mut c1 = Channel::new_with_capacity::<u16>(size);

        c1.extend(interleaved_pixels);

        return Ok(vec![c1]);
    } else if colorspace.num_components() == 2 {
        let mut c1 = Channel::new_with_length::<u16>(size);
        let mut c2 = Channel::new_with_length::<u16>(size);

        let c1_mut = c1.reinterpret_as_mut::<u16>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u16>().unwrap();

        for ((chunk, c1), c2) in interleaved_pixels.chunks_exact(2).zip(c1_mut).zip(c2_mut) {
            *c1 = chunk[0];
            *c2 = chunk[1];
        }

        return Ok(vec![c1, c2]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3 {
        let mut c1 = Channel::new_with_length::<u16>(size);
        let mut c2 = Channel::new_with_length::<u16>(size);
        let mut c3 = Channel::new_with_length::<u16>(size);

        let c1_mut = c1.reinterpret_as_mut::<u16>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u16>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u16>().unwrap();

        de_interleave_three_channels_u16(interleaved_pixels, c1_mut, c2_mut, c3_mut);

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    } else if colorspace.num_components() == 4 {
        let mut c1 = Channel::new_with_length::<u16>(size);
        let mut c2 = Channel::new_with_length::<u16>(size);
        let mut c3 = Channel::new_with_length::<u16>(size);
        let mut c4 = Channel::new_with_length::<u16>(size);

        let c1_mut = c1.reinterpret_as_mut::<u16>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u16>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u16>().unwrap();
        let c4_mut = c4.reinterpret_as_mut::<u16>().unwrap();

        de_interleave_four_channels_u16(interleaved_pixels, c1_mut, c2_mut, c3_mut, c4_mut);

        return Ok(vec![c1, c2, c3, c4]);
    }

    todo!()
}

pub fn deinterleave_f32(
    interleaved_pixels: &[f32], colorspace: ColorSpace
) -> Result<Vec<Channel>, ImageErrors> {
    if interleaved_pixels.len() % colorspace.num_components() != 0 {
        return Err(ImageErrors::OperationsError(
            ImageOperationsErrors::InvalidChannelLayout("Extra pixels in the colorspace")
        ));
    }
    let size = (interleaved_pixels.len() / colorspace.num_components()) * 4 /*Depth 4  bytes*/;

    if colorspace.num_components() == 1 {
        let mut c1 = Channel::new_with_capacity::<f32>(size);

        c1.extend(interleaved_pixels);

        return Ok(vec![c1]);
    } else if colorspace.num_components() == 2 {
        let mut c1 = Channel::new_with_length::<f32>(size);
        let mut c2 = Channel::new_with_length::<f32>(size);

        let c1_mut = c1.reinterpret_as_mut::<f32>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<f32>().unwrap();

        for ((chunk, c1), c2) in interleaved_pixels.chunks_exact(2).zip(c1_mut).zip(c2_mut) {
            *c1 = chunk[0];
            *c2 = chunk[1];
        }

        return Ok(vec![c1, c2]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3 {
        let mut c1 = Channel::new_with_length::<f32>(size);
        let mut c2 = Channel::new_with_length::<f32>(size);
        let mut c3 = Channel::new_with_length::<f32>(size);

        let c1_mut = c1.reinterpret_as_mut::<f32>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<f32>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<f32>().unwrap();

        de_interleave_three_channels_f32(interleaved_pixels, c1_mut, c2_mut, c3_mut);

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    } else if colorspace.num_components() == 4 {
        let mut c1 = Channel::new_with_length::<f32>(size);
        let mut c2 = Channel::new_with_length::<f32>(size);
        let mut c3 = Channel::new_with_length::<f32>(size);
        let mut c4 = Channel::new_with_length::<f32>(size);

        let c1_mut = c1.reinterpret_as_mut::<f32>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<f32>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<f32>().unwrap();
        let c4_mut = c4.reinterpret_as_mut::<f32>().unwrap();

        de_interleave_four_channels_f32(interleaved_pixels, c1_mut, c2_mut, c3_mut, c4_mut);

        return Ok(vec![c1, c2, c3, c4]);
    }
    todo!()
}
