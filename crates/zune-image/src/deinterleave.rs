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
use zune_imageprocs::deinterleave::{
    de_interleave_four_channels_u16, de_interleave_three_channels_u16,
    de_interleave_three_channels_u8, deinterleave_four_channels_u8
};

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;

/// Separates image u8's into various components
pub fn deinterleave_u8(
    interleaved_pixels: &[u8], colorspace: ColorSpace
) -> Result<Vec<Channel>, ImgOperationsErrors>
{
    if interleaved_pixels.len() % colorspace.num_components() != 0
    {
        return Err(ImgOperationsErrors::InvalidChannelLayout(
            "Extra pixels in the colorspace"
        ));
    }
    let size = interleaved_pixels.len() / colorspace.num_components();

    if colorspace.num_components() == 1
    {
        let mut c1 = Channel::new_with_length(size);

        c1.extend(interleaved_pixels);

        return Ok(vec![c1]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3
    {
        let mut c1 = Channel::new_with_length(size);
        let mut c2 = Channel::new_with_length(size);
        let mut c3 = Channel::new_with_length(size);

        let c1_mut = c1.reinterpret_as_mut::<u8>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u8>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u8>().unwrap();

        de_interleave_three_channels_u8(interleaved_pixels, c1_mut, c2_mut, c3_mut);

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    }
    else if colorspace.num_components() == 4
    {
        let mut c1 = Channel::new_with_length(size);
        let mut c2 = Channel::new_with_length(size);
        let mut c3 = Channel::new_with_length(size);
        let mut c4 = Channel::new_with_length(size);

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
) -> Result<Vec<Channel>, ImgOperationsErrors>
{
    if interleaved_pixels.len() % colorspace.num_components() != 0
    {
        return Err(ImgOperationsErrors::InvalidChannelLayout(
            "Extra pixels in the colorspace"
        ));
    }
    let size = (interleaved_pixels.len() / colorspace.num_components()) * 2 /*Depth is two bytes*/;

    if colorspace.num_components() == 1
    {
        let mut c1 = Channel::new_with_length(size);

        c1.extend(interleaved_pixels);

        return Ok(vec![c1]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3
    {
        let mut c1 = Channel::new_with_length(size);
        let mut c2 = Channel::new_with_length(size);
        let mut c3 = Channel::new_with_length(size);

        let c1_mut = c1.reinterpret_as_mut::<u16>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u16>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u16>().unwrap();

        de_interleave_three_channels_u16(interleaved_pixels, c1_mut, c2_mut, c3_mut);

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    }
    else if colorspace.num_components() == 4
    {
        let mut c1 = Channel::new_with_length(size);
        let mut c2 = Channel::new_with_length(size);
        let mut c3 = Channel::new_with_length(size);
        let mut c4 = Channel::new_with_length(size);

        let c1_mut = c1.reinterpret_as_mut::<u16>().unwrap();
        let c2_mut = c2.reinterpret_as_mut::<u16>().unwrap();
        let c3_mut = c3.reinterpret_as_mut::<u16>().unwrap();
        let c4_mut = c4.reinterpret_as_mut::<u16>().unwrap();

        de_interleave_four_channels_u16(interleaved_pixels, c1_mut, c2_mut, c3_mut, c4_mut);

        return Ok(vec![c1, c2, c3, c4]);
    }

    todo!()
}
