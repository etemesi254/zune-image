use zune_core::colorspace::ColorSpace;
use zune_imageprocs::deinterleave::{de_interleave_four_channels, de_interleave_three_channels};

use crate::conversions::to_u16;
use crate::errors::ImgOperationsErrors;

pub fn deinterleave_u8(
    interleaved_pixels: &[u8], colorspace: ColorSpace,
) -> Result<Vec<Vec<u16>>, ImgOperationsErrors>
{
    if interleaved_pixels.len() % colorspace.num_components() != 0
    {
        return Err(ImgOperationsErrors::InvalidChannelLayout(
            "Extra pixels in the colorspace",
        ));
    }
    let size = interleaved_pixels.len() / colorspace.num_components();

    if colorspace.num_components() == 1
    {
        let mut c1 = vec![0; size];

        to_u16(interleaved_pixels, &mut c1);

        return Ok(vec![c1]);
    }
    // three component de-interleave
    else if colorspace.num_components() == 3
    {
        let mut c1 = vec![0; size];
        let mut c2 = vec![0; size];
        let mut c3 = vec![0; size];

        de_interleave_three_channels(interleaved_pixels, (&mut c1, &mut c2, &mut c3));

        // change the channel type to be uninitialized rgb8
        return Ok(vec![c1, c2, c3]);
    }
    else if colorspace.num_components() == 4
    {
        // four component deinterleave
        let mut c1 = vec![0; size];
        let mut c2 = vec![0; size];
        let mut c3 = vec![0; size];
        let mut c4 = vec![0; size];

        de_interleave_four_channels(interleaved_pixels, (&mut c1, &mut c2, &mut c3, &mut c4));

        return Ok(vec![c1, c2, c3, c4]);
    }

    todo!()
}
