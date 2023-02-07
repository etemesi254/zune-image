use alloc::format;
use alloc::vec::Vec;
use core::convert::TryInto;

use zune_core::colorspace::ColorSpace;

use crate::color_convert::ycbcr_to_grayscale;
use crate::components::{ComponentID, Components};
use crate::decoder::{ColorConvert16Ptr, MAX_COMPONENTS};
use crate::errors::DecodeErrors;

/// fast 0..255 * 0..255 => 0..255 rounded multiplication
///
/// Borrowed from stb
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn blinn_8x8(in_val: u8, y: u8) -> u8
{
    let t = i32::from(in_val) * i32::from(y) + 128;
    return ((t + (t >> 8)) >> 8) as u8;
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub(crate) fn color_convert_no_sampling(
    unprocessed: &[&[i16]; MAX_COMPONENTS], color_convert_16: ColorConvert16Ptr,
    input_colorspace: ColorSpace, output_colorspace: ColorSpace, output: &mut [u8], width: usize,
    padded_width: usize
) -> Result<(), DecodeErrors> // so many parameters..
{
    // maximum sampling factors are in Y-channel, no need to pass them.

    if input_colorspace.num_components() == 3 && input_colorspace == output_colorspace
    {
        // sort things like RGB to RGB conversion
        copy_removing_padding(unprocessed, width, padded_width, output);
        return Ok(());
    }
    // color convert
    match (input_colorspace, output_colorspace)
    {
        (ColorSpace::YCbCr | ColorSpace::Luma, ColorSpace::Luma) =>
        {
            ycbcr_to_grayscale(unprocessed[0], width, padded_width, output);
        }
        (ColorSpace::YCbCr, ColorSpace::RGB | ColorSpace::RGBA) =>
        {
            color_convert_ycbcr(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }
        (ColorSpace::YCCK, ColorSpace::RGB) =>
        {
            color_convert_ycck_to_rgb::<3>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }

        (ColorSpace::YCCK, ColorSpace::RGBA) =>
        {
            color_convert_ycck_to_rgb::<4>(
                unprocessed,
                width,
                padded_width,
                output_colorspace,
                color_convert_16,
                output
            );
        }
        (ColorSpace::CMYK, ColorSpace::RGB) =>
        {
            color_convert_cymk_to_rgb::<3>(unprocessed, width, padded_width, output);
        }
        (ColorSpace::CMYK, ColorSpace::RGBA) =>
        {
            color_convert_cymk_to_rgb::<4>(unprocessed, width, padded_width, output);
        }
        // For the other components we do nothing(currently)
        _ =>
        {
            let msg = format!(
                    "Unimplemented colorspace mapping from {input_colorspace:?} to {output_colorspace:?}");

            return Err(DecodeErrors::Format(msg));
        }
    }
    Ok(())
}

/// Copy a block to output removing padding bytes from input
/// if necessary
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn copy_removing_padding(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize, output: &mut [u8]
)
{
    for (((pix_w, c_w), m_w), y_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
    {
        for (((pix, c), y), m) in pix_w.chunks_exact_mut(3).zip(c_w).zip(m_w).zip(y_w)
        {
            pix[0] = *c as u8;
            pix[1] = *y as u8;
            pix[2] = *m as u8;
        }
    }
}

/// Convert YCCK image to rgb
#[allow(clippy::cast_possible_truncation)]
fn color_convert_ycck_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize,
    output_colorspace: ColorSpace, color_convert_16: ColorConvert16Ptr, output: &mut [u8]
)
{
    color_convert_ycbcr(
        mcu_block,
        width,
        padded_width,
        output_colorspace,
        color_convert_16,
        output
    );
    for (pix_w, m_w) in output
        .chunks_exact_mut(width * 3)
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for (pix, m) in pix_w.chunks_exact_mut(NUM_COMPONENTS).zip(m_w)
        {
            let m = (*m) as u8;
            pix[0] = blinn_8x8(255 - pix[0], m);
            pix[1] = blinn_8x8(255 - pix[1], m);
            pix[2] = blinn_8x8(255 - pix[2], m);
        }
    }
}

#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
fn color_convert_cymk_to_rgb<const NUM_COMPONENTS: usize>(
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize, output: &mut [u8]
)
{
    for ((((pix_w, c_w), m_w), y_w), k_w) in output
        .chunks_exact_mut(width * NUM_COMPONENTS)
        .zip(mcu_block[0].chunks_exact(padded_width))
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
        .zip(mcu_block[3].chunks_exact(padded_width))
    {
        for ((((pix, c), m), y), k) in pix_w
            .chunks_exact_mut(3)
            .zip(c_w)
            .zip(m_w)
            .zip(y_w)
            .zip(k_w)
        {
            let c = *c as u8;
            let m = *m as u8;
            let y = *y as u8;
            let k = *k as u8;

            pix[0] = blinn_8x8(c, k);
            pix[1] = blinn_8x8(m, k);
            pix[2] = blinn_8x8(y, k);
        }
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
    mcu_block: &[&[i16]; MAX_COMPONENTS], width: usize, padded_width: usize,
    output_colorspace: ColorSpace, color_convert_16: ColorConvert16Ptr, output: &mut [u8]
)
{
    let num_components = output_colorspace.num_components();

    let stride = width * num_components;

    // Allocate temporary buffer for small widths less than  16.
    let mut temp = [0; 64];
    // We need to chunk per width to ensure we can discard extra values at the end of the width.
    // Since the encoder may pad bits to ensure the width is a multiple of 8.
    for (((y_width, cb_width), cr_width), out) in mcu_block[0]
        .chunks_exact(padded_width)
        .zip(mcu_block[1].chunks_exact(padded_width))
        .zip(mcu_block[2].chunks_exact(padded_width))
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
        for ((y, cb), cr) in y_width[width - 16..]
            .chunks_exact(16)
            .zip(cb_width[width - 16..].chunks_exact(16))
            .zip(cr_width[width - 16..].chunks_exact(16))
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

        let rem = out[(width - 16) * num_components..]
            .chunks_exact_mut(16 * num_components)
            .next()
            .unwrap();

        rem.copy_from_slice(&temp[0..rem.len()]);
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn upsample_and_color_convert(
    unprocessed: &[Vec<i16>; MAX_COMPONENTS], component_data: &mut [Components],
    padded_width: usize, color_convert_16: ColorConvert16Ptr, input_colorspace: ColorSpace,
    output_colorspace: ColorSpace, output: &mut [u8], width: usize, height: usize,
    scratch_space: &mut [i16]
) -> Result<(), DecodeErrors>
{
    let v_samp = component_data[0].vertical_sample;

    let out_stride = width * output_colorspace.num_components() * v_samp;
    // Width of image which takes into account fill bytes
    let width_stride = component_data[0].width_stride * v_samp;

    let last_row = height / v_samp;

    for ((pos, out), y_stride) in output
        .chunks_mut(out_stride)
        .enumerate()
        .zip(unprocessed[0].chunks(width_stride))
    {
        for (component_position, component) in component_data.iter_mut().enumerate()
        {
            if component.component_id == ComponentID::Y || !component.needed
            {
                continue;
            }
            // read a down-sampled stride and upsample it
            // we need to also take the nearest row and the furthest

            let raw_data = &unprocessed[component_position];

            let comp_stride_start = pos * component.width_stride;
            let comp_stride_stop = comp_stride_start + component.width_stride;

            // take a slice of the row above and the row below
            let row_up = if pos == 0
            {
                &raw_data[0..component.width_stride]
            }
            else
            {
                &raw_data[comp_stride_start - component.width_stride..comp_stride_start]
            };

            let row_down = if pos + 1 >= last_row
            {
                // last row, the raw data is the same as the input
                &raw_data[comp_stride_start..comp_stride_stop]
            }
            else
            {
                &raw_data[comp_stride_stop..comp_stride_stop + component.width_stride]
            };

            let comp_stride = &raw_data[comp_stride_start..comp_stride_stop];
            let out_stride = &mut component.upsample_dest;

            // upsample using the fn pointer, can either be h,v or hv upsampling.
            (component.up_sampler)(comp_stride, row_up, row_down, scratch_space, out_stride);
        }

        // by here, each component has been up-sampled, so let's color convert a row(s)
        let cb_stride = &component_data[1].upsample_dest;
        let cr_stride = &component_data[2].upsample_dest;

        let iq_stride: &[i16] = if let Some(component) = component_data.get(3)
        {
            &component.upsample_dest
        }
        else
        {
            &[]
        };

        color_convert_no_sampling(
            &[y_stride, cb_stride, cr_stride, iq_stride],
            color_convert_16,
            input_colorspace,
            output_colorspace,
            out,
            width,
            padded_width
        )?;
    }
    Ok(())
}

#[test]
fn test_jpg()
{
    use crate::JpegDecoder;

    let data = std::fs::read("/home/caleb/jpeg/milad.jpg").unwrap();
    let _ = JpegDecoder::new(&data).decode().unwrap();
}
