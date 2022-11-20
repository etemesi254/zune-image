use std::convert::TryInto;

use zune_core::colorspace::ColorSpace;

use crate::color_convert::ycbcr_to_grayscale;
use crate::components::{ComponentID, Components};
use crate::decoder::ColorConvert16Ptr;

pub(crate) fn color_convert_no_sampling(
    unprocessed: &[&[i16]; 3], color_convert_16: ColorConvert16Ptr, input_colorspace: ColorSpace,
    output_colorspace: ColorSpace, output: &mut [u8], width: usize
) // so many parameters..
{
    // maximum sampling factors are in Y-channel, no need to pass them.

    // color convert
    match (input_colorspace, output_colorspace)
    {
        (ColorSpace::YCbCr | ColorSpace::Luma, ColorSpace::Luma) =>
        {
            ycbcr_to_grayscale(unprocessed[0], width, output);
        }
        (
            ColorSpace::YCbCr,
            ColorSpace::YCbCr | ColorSpace::RGB | ColorSpace::RGBA | ColorSpace::RGBX
        ) =>
        {
            color_convert_ycbcr(
                unprocessed,
                width,
                output_colorspace,
                color_convert_16,
                output
            );
        }
        // For the other components we do nothing(currently)
        _ =>
        {}
    }
}

/// Do color-conversion for interleaved MCU
#[allow(
    clippy::similar_names,
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::unwrap_used
)]
fn color_convert_ycbcr(
    mcu_block: &[&[i16]; 3], width: usize, output_colorspace: ColorSpace,
    color_convert_16: ColorConvert16Ptr, output: &mut [u8]
)
{
    // Width of image which takes into account fill bytes(it may be larger than actual width).
    let width_chunk = ((width + 7) >> 3) * 8;
    let num_components = output_colorspace.num_components();

    let stride = width * num_components;

    // Allocate temporary buffer for small widths less than  16.
    let mut temp = [0; 64];
    // We need to chunk per width to ensure we can discard extra values at the end of the width.
    // Since the encoder may pad bits to ensure the width is a multiple of 8.
    for (((y_width, cb_width), cr_width), out) in mcu_block[0]
        .chunks_exact(width_chunk)
        .zip(mcu_block[1].chunks_exact(width_chunk))
        .zip(mcu_block[2].chunks_exact(width_chunk))
        .zip(output.chunks_exact_mut(stride))
    {
        if width < 16
        {
            // allocate temporary buffers for the values received from idct
            let mut y_out = [0; 16];
            let mut cb_out = [0; 16];
            let mut cr_out = [0; 16];
            // copy those small widths to that buffer
            y_out[0..y_width.len()].copy_from_slice(y_width);
            cb_out[0..cb_width.len()].copy_from_slice(cb_width);
            cr_out[0..cr_width.len()].copy_from_slice(cr_width);
            // we handle widths less than 16 a bit differently, allocating a temporary
            // buffer and writing to that and then flushing to the out buffer
            // because of the optimizations applied below,
            (color_convert_16)(&y_out, &cb_out, &cr_out, &mut temp, &mut 0);
            // copy to stride
            out[0..width * num_components].copy_from_slice(&temp[0..width * num_components]);
            // next
            continue;
        }

        // Chunk in outputs of 16 to pass to color_convert as an array of 16 i16's.
        for (((y, cb), cr), out_c) in y_width
            .chunks_exact(16)
            .zip(cb_width.chunks_exact(16))
            .zip(cr_width.chunks_exact(16))
            .zip(out.chunks_exact_mut(16 * num_components))
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                out_c,
                &mut 0
            );
        }
        //we have more pixels in the end that can't be handled by the main loop.
        //move pointer back a little bit to get last 16 bytes,
        //color convert, and overwrite
        //This means some values will be color converted twice.

        //redo the last one

        // handles widths not divisible by 16
        for ((y, cb), cr) in y_width
            .rchunks_exact(16)
            .zip(cb_width.rchunks_exact(16))
            .zip(cr_width.rchunks_exact(16))
            .take(1)
        {
            (color_convert_16)(
                y.try_into().unwrap(),
                cb.try_into().unwrap(),
                cr.try_into().unwrap(),
                &mut temp,
                &mut 0
            );
        }
        let rem = out.chunks_exact_mut(16 * num_components).into_remainder();

        rem.copy_from_slice(&temp[0..rem.len()]);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsample_and_color_convert(
    unprocessed: &[Vec<i16>; 3], component_data: &mut [Components],
    color_convert_16: ColorConvert16Ptr, input_colorspace: ColorSpace,
    output_colorspace: ColorSpace, output: &mut [u8], width: usize, scratch_space: &mut [i16]
)
{
    let v_samp = component_data[0].vertical_sample;
    let out_stride = width * output_colorspace.num_components() * v_samp;
    // Width of image which takes into account fill bytes
    let width_stride = component_data[0].width_stride * v_samp;

    for ((pos, out), y_stride) in output
        .chunks_mut(out_stride)
        .enumerate()
        .zip(unprocessed[0].chunks(width_stride))
    {
        for component in component_data.iter_mut()
        {
            if component.component_id == ComponentID::Y || !component.needed
            {
                continue;
            }
            // read a down-sampled stride and upsample it
            let raw_data = &unprocessed[usize::from(component.id.saturating_sub(1))];
            let comp_stride_start = pos * component.width_stride;
            let comp_stride_stop = comp_stride_start + component.width_stride;
            let comp_stride = &raw_data[comp_stride_start..comp_stride_stop];
            let out_ref = &mut component.upsample_scanline;
            let out_stride = &mut component.upsample_dest;

            // upsample using the fn pointer, can either be h,v or hv upsampling.
            (component.up_sampler)(comp_stride, out_ref, scratch_space, out_stride);
        }

        // by here, each component has been up-sampled, so let's color convert a row(s)
        let cb_stride = &component_data[1].upsample_dest;
        let cr_stride = &component_data[2].upsample_dest;

        color_convert_no_sampling(
            &[y_stride, cb_stride, cr_stride],
            color_convert_16,
            input_colorspace,
            output_colorspace,
            out,
            width
        );
    }
}
