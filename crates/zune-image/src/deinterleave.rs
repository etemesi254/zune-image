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
//! post-processing for scenarios where processing is slow.

use alloc::vec::Vec;

use bytemuck::{Pod, Zeroable};
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

/// De-interleave generic channels
fn deinterleave_generic<T: Default + Clone + Copy + 'static + Zeroable + Pod>(
    interleaved_pixels: &[T], colorspace: ColorSpace
) -> Result<Vec<Channel>, ImageErrors> {
    if interleaved_pixels.len() % colorspace.num_components() != 0 {
        return Err(ImageErrors::OperationsError(
            ImageOperationsErrors::InvalidChannelLayout("Extra pixels in the colorspace")
        ));
    }
    let size = (interleaved_pixels.len() / colorspace.num_components()) * core::mem::size_of::<T>();

    if size == 0 {
        return Err(ImageErrors::GenericStr(
            "Too Small of an interleaved pixel count"
        ));
    }
    let mut channels = vec![Channel::new_with_length::<T>(size); colorspace.num_components()];

    let mut channel_converts = vec![];

    for c in channels.iter_mut() {
        channel_converts.push(c.reinterpret_as_mut::<T>().unwrap());
    }
    // then now convert them
    for (pos, pix_chunk) in interleaved_pixels
        .chunks_exact(colorspace.num_components())
        .enumerate()
    {
        for (single_channel, pix) in channel_converts.iter_mut().zip(pix_chunk) {
            single_channel[pos] = *pix;
        }
    }
    Ok(channels)
}

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

    if size == 0 {
        return Err(ImageErrors::GenericStr(
            "Too Small of an interleaved pixel count"
        ));
    }
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

    deinterleave_generic(interleaved_pixels, colorspace)
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

    if size == 0 {
        return Err(ImageErrors::GenericStr(
            "Too Small of an interleaved pixel count"
        ));
    }
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

    deinterleave_generic(interleaved_pixels, colorspace)
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

    if size == 0 {
        return Err(ImageErrors::GenericStr(
            "Too Small of an interleaved pixel count"
        ));
    }
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
    deinterleave_generic(interleaved_pixels, colorspace)
}

#[cfg(test)]
mod tests {
    use core::array;
    use core::num::NonZeroU32;

    use zune_core::colorspace::ColorSpace;

    use crate::deinterleave::{deinterleave_f32, deinterleave_u16, deinterleave_u8};
    #[test]
    pub fn deinterleave_multiband_u8() {
        let img = array::from_fn::<_, 256, _>(|c| c as u8).to_vec();
        let resp =
            deinterleave_u8(&img, ColorSpace::MultiBand(NonZeroU32::new(16).unwrap())).unwrap();
        assert_eq!(resp.len(), 16);
        assert_eq!(resp[0].reinterpret_as::<u8>().unwrap().len(), 256 / 16);
    }
    #[test]
    pub fn deinterleave_multiband_u16() {
        let img = array::from_fn::<_, 256, _>(|c| c as u16).to_vec();
        let resp =
            deinterleave_u16(&img, ColorSpace::MultiBand(NonZeroU32::new(16).unwrap())).unwrap();
        assert_eq!(resp.len(), 16);
        assert_eq!(resp[0].reinterpret_as::<u16>().unwrap().len(), 256 / 16);
    }

    #[test]
    pub fn deinterleave_multiband_f32() {
        let img = array::from_fn::<_, 256, _>(|c| c as f32).to_vec();
        let resp =
            deinterleave_f32(&img, ColorSpace::MultiBand(NonZeroU32::new(16).unwrap())).unwrap();
        assert_eq!(resp.len(), 16);
        assert_eq!(resp[0].reinterpret_as::<f32>().unwrap().len(), 256 / 16);
    }

    #[test]
    #[should_panic]
    pub fn deinterleave_multiband_extra_pixels() {
        let img = array::from_fn::<_, 102, _>(|c| c as f32).to_vec();
        deinterleave_f32(&img, ColorSpace::MultiBand(NonZeroU32::new(16).unwrap())).unwrap();
    }

    #[test]
    #[should_panic]
    pub fn deinterleave_multiband_zero_array() {
        let img = array::from_fn::<_, 0, _>(|c| c as f32).to_vec();
        deinterleave_f32(&img, ColorSpace::MultiBand(NonZeroU32::new(16).unwrap())).unwrap();
    }
}
