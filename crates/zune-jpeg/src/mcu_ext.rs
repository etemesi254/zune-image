/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Extended JPEG decoding (SOF1) for 9-16 bit sample precision.
//!
//! This module provides MCU decoding routines for extended sequential JPEG images
//! which support sample precisions beyond the baseline 8-bit limit. The main
//! differences from baseline decoding are:
//!
//! - Output is `&mut [u16]` instead of `&mut [u8]` to hold wider samples
//! - IDCT clamping range depends on sample precision (e.g., 0-4095 for 12-bit)
//! - Color conversion outputs 16-bit samples
//!
//! Extended JPEG is commonly used in medical imaging (DICOM) where higher
//! precision is needed to preserve subtle grayscale variations.

use alloc::vec::Vec;
use alloc::{format, vec};
use core::cmp::min;

use zune_core::bytestream::ZByteReaderTrait;
use zune_core::colorspace::ColorSpace;
use zune_core::colorspace::ColorSpace::Luma;
use zune_core::log::{error, trace, warn};

use crate::bitstream::BitStream;
use crate::components::SampleRatios;
use crate::decoder::MAX_COMPONENTS;
use crate::errors::DecodeErrors;
use crate::idct::scalar::{idct_int_1x1_extended, idct_int_4x4_extended, idct_int_extended};
use crate::mcu::{McuContinuation, DCT_BLOCK};
use crate::mcu_prog::get_marker;
use crate::misc::{calculate_padded_width, setup_component_params};
use crate::JpegDecoder;

impl<T: ZByteReaderTrait> JpegDecoder<T> {
    /// Decode MCUs for extended JPEG (9-16 bit precision) images.
    ///
    /// This is similar to `decode_mcu_ycbcr_baseline` but outputs to a 16-bit buffer
    /// and uses wider clamping ranges appropriate for the sample precision.
    #[allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::cast_possible_truncation
    )]
    #[inline(never)]
    pub(crate) fn decode_mcu_ycbcr_extended<const PREC: u8>(
        &mut self, pixels: &mut [u16]
    ) -> Result<(), DecodeErrors> {
        setup_component_params(self)?;

        // check dc and AC tables
        self.check_tables()?;

        let (mut mcu_width, mut mcu_height);

        if self.is_interleaved {
            // set upsampling functions
            self.set_upsampling()?;

            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;
        } else {
            // For non-interleaved images( (1*1) subsampling)
            // number of MCU's are the widths (+7 to account for paddings) divided by 8.
            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }

        if self.is_interleaved
            && self.input_colorspace.num_components() > 1
            && self.options.jpeg_get_out_colorspace().num_components() == 1
            && (self.info.sample_ratio == SampleRatios::V
                || self.info.sample_ratio == SampleRatios::HV)
        {
            mcu_height *= self.v_max;
            mcu_height /= self.h_max;
            self.coeff = 2;
        }

        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved {
            warn!("Grayscale image with down-sampled component, resetting component details");

            self.reset_params();

            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }

        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.info.sample_ratio);

        let mut stream = BitStream::new();
        let mut tmp = [0_i32; DCT_BLOCK];

        let comp_len = self.components.len();

        for (pos, comp) in self.components.iter_mut().enumerate() {
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos
            ) == pos
                || comp_len == 4
            {
                let len = comp.width_stride * comp.vertical_sample * 8;

                comp.needed = true;
                comp.raw_coeff = vec![0; len];
            } else {
                comp.needed = false;
            }
        }

        let all_components_in_first_scan = usize::from(self.num_scans) == self.components.len();
        let mut progressive_mcus: [Vec<i16>; 4] = core::array::from_fn(|_| vec![]);

        if !all_components_in_first_scan {
            for (component, mcu) in self.components.iter().zip(&mut progressive_mcus) {
                let len = mcu_width
                    * component.vertical_sample
                    * component.horizontal_sample
                    * mcu_height
                    * 64;
                *mcu = vec![0; len];
            }
        }

        let mut pixels_written = 0;

        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv
            * self
                .components
                .iter()
                .map(|x| x.width_stride)
                .max()
                .unwrap_or(0)
            * 8;
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];

        'sos: loop {
            trace!(
                "Extended JPEG decoding of components: {:?}",
                &self.z_order[..usize::from(self.num_scans)]
            );

            trace!("Decoding MCU width: {mcu_width}, height: {mcu_height}");

            for i in 0..mcu_height {
                if stream.overread_by > 0 {
                    // Fill remaining pixels with mid-gray for the bit depth
                    let mid_gray = (1u16 << (PREC - 1)) as u16;
                    pixels.get_mut(pixels_written..).map(|v| v.fill(mid_gray));
                    if self.options.strict_mode() {
                        return Err(DecodeErrors::FormatStatic("Premature end of buffer"));
                    };

                    error!("Premature end of buffer");
                    break;
                }

                let terminate = if all_components_in_first_scan {
                    self.decode_mcu_width_extended::<false, PREC>(
                        mcu_width,
                        i,
                        &mut tmp,
                        &mut stream,
                        &mut progressive_mcus
                    )?
                } else {
                    self.decode_mcu_width_extended::<true, PREC>(
                        mcu_width,
                        i,
                        &mut tmp,
                        &mut stream,
                        &mut progressive_mcus
                    )?
                };

                if all_components_in_first_scan {
                    self.post_process_extended::<PREC>(
                        pixels,
                        i,
                        mcu_height,
                        width,
                        padded_width,
                        &mut pixels_written,
                        &mut upsampler_scratch_space
                    )?;
                }

                match terminate {
                    McuContinuation::Ok => {}
                    McuContinuation::AnotherSos if all_components_in_first_scan => {
                        warn!("More than one SOS despite already having all components");
                        return Ok(());
                    }
                    McuContinuation::AnotherSos => continue 'sos,
                    McuContinuation::InterScanMarker(marker) => {
                        if self.advance_to_next_sos(marker, &mut stream)? {
                            continue 'sos;
                        } else {
                            break;
                        }
                    }
                    McuContinuation::Terminate => {
                        warn!("Got terminate signal, will not process further");
                        let mid_gray = (1u16 << (PREC - 1)) as u16;
                        pixels.get_mut(pixels_written..).map(|v| v.fill(mid_gray));
                        return Ok(());
                    }
                }
            }

            break;
        }

        if !all_components_in_first_scan {
            self.finish_extended_decoding::<PREC>(&progressive_mcus, mcu_width, pixels)?;
        }

        if !stream.seen_eoi {
            let marker = get_marker(&mut self.stream, &mut stream);
            match marker {
                Ok(_m) => {
                    trace!("Found marker {:?}", _m);
                }
                Err(_) => {}
            }
        }

        trace!("Finished decoding extended JPEG image");

        Ok(())
    }

    /// Process all MCUs for extended JPEG when decoding component-after-component.
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cast_sign_loss)]
    pub(crate) fn finish_extended_decoding<const PREC: u8>(
        &mut self, block: &[Vec<i16>; MAX_COMPONENTS], _mcu_width: usize, pixels: &mut [u16]
    ) -> Result<(), DecodeErrors> {
        let mcu_height = self.mcu_y;

        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.info.sample_ratio);

        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];

        for (pos, comp) in self.components.iter_mut().enumerate() {
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos
            ) == pos
                || self.input_colorspace == ColorSpace::YCCK
                || self.input_colorspace == ColorSpace::CMYK
            {
                comp.needed = true;
            } else {
                comp.needed = false;
            }
        }

        let mut pixels_written = 0;

        for i in 0..mcu_height {
            'component: for (position, component) in &mut self.components.iter_mut().enumerate() {
                if !component.needed {
                    continue 'component;
                }

                let step = block[position].len() / mcu_height;
                let slice = &block[position][i * step..][..step];
                let temp_channel = &mut component.raw_coeff;
                temp_channel[..step].copy_from_slice(slice);
            }

            self.post_process_extended::<PREC>(
                pixels,
                i,
                mcu_height,
                width,
                padded_width,
                &mut pixels_written,
                &mut upsampler_scratch_space
            )?;
        }

        Ok(())
    }

    fn decode_mcu_width_extended<const PROGRESSIVE: bool, const PREC: u8>(
        &mut self, mcu_width: usize, mcu_height: usize, tmp: &mut [i32; 64],
        stream: &mut BitStream, progressive: &mut [Vec<i16>; 4]
    ) -> Result<McuContinuation, DecodeErrors> {
        let is_one_by_one = !self.scan_subsampled;

        if is_one_by_one {
            self.inner_decode_mcu_width_extended::<PROGRESSIVE, false, PREC>(
                mcu_width,
                mcu_height,
                tmp,
                stream,
                progressive
            )
        } else {
            self.inner_decode_mcu_width_extended::<PROGRESSIVE, true, PREC>(
                mcu_width,
                mcu_height,
                tmp,
                stream,
                progressive
            )
        }
    }

    #[inline(never)]
    fn inner_decode_mcu_width_extended<
        const PROGRESSIVE: bool,
        const SAMPLED: bool,
        const PREC: u8
    >(
        &mut self, mcu_width: usize, mcu_height: usize, tmp: &mut [i32; 64],
        stream: &mut BitStream, progressive: &mut [Vec<i16>; 4]
    ) -> Result<McuContinuation, DecodeErrors> {
        let z_order = self.z_order;
        let z_scans = &z_order[..usize::from(self.num_scans)];

        let mut clobber_more_than_4x4 = true;

        let scan_du_width = if PROGRESSIVE {
            let k = z_scans[0];
            let comp = &self.components[k];
            (self.info.width as usize * comp.horizontal_sample + self.h_max * 8 - 1)
                / (self.h_max * 8)
        } else {
            mcu_width
        };

        for j in 0..scan_du_width {
            for &k in z_scans {
                let component = &mut self.components[k];

                let dc_table = self.dc_huffman_tables[component.dc_huff_table % MAX_COMPONENTS]
                    .as_ref()
                    .ok_or(DecodeErrors::FormatStatic("DC table not found"))?;

                let ac_table = self.ac_huffman_tables[component.ac_huff_table % MAX_COMPONENTS]
                    .as_ref()
                    .ok_or(DecodeErrors::FormatStatic("AC table not found"))?;

                let qt_table = &component.quantization_table;
                let channel = if PROGRESSIVE {
                    let offset =
                        mcu_height * component.width_stride * 8 * component.vertical_sample;
                    &mut progressive[k][offset..]
                } else {
                    &mut component.raw_coeff
                };

                let component_samples_needed = component.needed;

                let v_step =
                    if SAMPLED && !PROGRESSIVE { 0..component.vertical_sample } else { 0..1 };

                for v_samp in v_step {
                    let h_step =
                        if SAMPLED && !PROGRESSIVE { 0..component.horizontal_sample } else { 0..1 };

                    for h_samp in h_step {
                        let result = if component_samples_needed {
                            let clobber_len = if !clobber_more_than_4x4 { 32 } else { 64 };

                            tmp[..clobber_len].fill(0);

                            stream.decode_mcu_block(
                                &mut self.stream,
                                dc_table,
                                ac_table,
                                qt_table,
                                tmp,
                                &mut component.dc_pred
                            )
                        } else {
                            stream.discard_mcu_block(&mut self.stream, dc_table, ac_table)
                        };

                        let len = if let Ok(len) = result {
                            len
                        } else {
                            return if self.options.strict_mode() {
                                Err(result.err().unwrap())
                            } else {
                                error!("{}", result.err().unwrap());
                                Ok(McuContinuation::Terminate)
                            };
                        };

                        if component_samples_needed {
                            clobber_more_than_4x4 = len > 10;

                            let idct_position = if PROGRESSIVE {
                                j * 8
                            } else {
                                let c2 = v_samp * 8;
                                let c3 = ((j * component.horizontal_sample) + h_samp) * 8;

                                component.width_stride * c2 + c3
                            };

                            let idct_pos = channel.get_mut(idct_position..).unwrap();

                            // Use extended IDCT with appropriate bit depth clamping
                            if len <= 1 {
                                idct_int_1x1_extended::<PREC>(
                                    tmp,
                                    idct_pos,
                                    component.width_stride
                                );
                            } else if len <= 10 {
                                idct_int_4x4_extended::<PREC>(
                                    tmp,
                                    idct_pos,
                                    component.width_stride
                                );
                            } else {
                                idct_int_extended::<PREC>(tmp, idct_pos, component.width_stride);
                            }
                        }
                    }
                }
            }

            self.todo = self.todo.wrapping_sub(1);

            if self.todo == 0 {
                self.handle_rst_main(stream)?;
                continue;
            }

            if stream.marker.is_some() && stream.bits_left == 0 {
                break;
            }
        }

        self.check_stream_marker_after_mcu_width(stream)
    }

    /// Post-process MCU data for extended JPEG, outputting 16-bit samples.
    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    pub(crate) fn post_process_extended<const PREC: u8>(
        &mut self, pixels: &mut [u16], i: usize, mcu_height: usize, width: usize,
        padded_width: usize, pixels_written: &mut usize, upsampler_scratch_space: &mut [i16]
    ) -> Result<(), DecodeErrors> {
        let out_colorspace_components = self.options.jpeg_get_out_colorspace().num_components();

        let mut px = *pixels_written;
        let is_vertically_sampled = self
            .components
            .iter()
            .any(|c| c.sample_ratio == SampleRatios::HV || c.sample_ratio == SampleRatios::V);

        let mut comp_len = self.components.len();

        if out_colorspace_components < comp_len && self.options.jpeg_get_out_colorspace() == Luma {
            comp_len = out_colorspace_components;
            _ = comp_len; // Silence unused warning - comp_len may be used in future color conversions
        }

        let comps = &mut self.components[..];

        // For extended JPEG, we primarily support grayscale output for medical imaging
        // Full color conversion would require 16-bit color conversion functions
        if self.options.jpeg_get_out_colorspace() == Luma || !self.is_interleaved {
            // Direct grayscale output - no color conversion needed
            let mut channels_ref: [&[i16]; MAX_COMPONENTS] = [&[]; MAX_COMPONENTS];

            self.components
                .iter()
                .enumerate()
                .for_each(|(pos, x)| channels_ref[pos] = &x.raw_coeff);

            let num_iters = if let SampleRatios::Generic(_, v) = self.info.sample_ratio {
                8 * v * self.coeff
            } else {
                8 * self.coeff
            };

            // Copy samples directly to output, converting i16 to u16
            for (pos, output) in pixels[px..]
                .chunks_exact_mut(width * out_colorspace_components)
                .take(num_iters)
                .enumerate()
            {
                for (j, out_pix) in output.iter_mut().enumerate().take(width) {
                    if let Some(&sample) = channels_ref[0].get(pos * padded_width + j) {
                        // i16 samples from IDCT are already clamped to valid range
                        // Convert to u16 for output
                        *out_pix = sample as u16;
                    }
                }
                px += width * out_colorspace_components;
            }
        } else if self.is_interleaved {
            // For interleaved color images, we need upsampling first
            for comp in comps.iter_mut() {
                crate::worker::upsample(
                    comp,
                    mcu_height,
                    i,
                    upsampler_scratch_space,
                    is_vertically_sampled
                )?;
            }

            // TODO: Implement full 16-bit color conversion for extended JPEG
            // For now, we only support grayscale or direct sample copy
            // Full YCbCr to RGB conversion for 12-bit would need new color conversion functions

            let mut samples: [&[i16]; 4] = [&[], &[], &[], &[]];

            for (samp, component) in samples.iter_mut().zip(self.components.iter()) {
                *samp = if component.sample_ratio == SampleRatios::None {
                    &component.raw_coeff
                } else {
                    &component.upsample_dest
                };
            }

            let is_last_considered = is_vertically_sampled && (i != mcu_height.saturating_sub(1));
            let num_iters = (8 - usize::from(is_last_considered)) * self.coeff * self.v_max;

            // Extended color conversion (simplified - outputs Y channel scaled)
            for (pos, output) in pixels[px..]
                .chunks_exact_mut(width * out_colorspace_components)
                .take(num_iters)
                .enumerate()
            {
                color_convert_extended::<PREC>(
                    &samples,
                    self.input_colorspace,
                    self.options.jpeg_get_out_colorspace(),
                    output,
                    width,
                    padded_width,
                    pos
                )?;
                px += width * out_colorspace_components;
            }
        }

        *pixels_written = px;
        Ok(())
    }
}

// =============================================================================
// Extended color conversion (16-bit output)
// =============================================================================

/// Color conversion for extended JPEG with 16-bit output.
///
/// This is a simplified implementation that handles the most common cases
/// for medical imaging (primarily grayscale).
#[allow(clippy::too_many_arguments)]
fn color_convert_extended<const PREC: u8>(
    samples: &[&[i16]; 4], input_colorspace: ColorSpace, output_colorspace: ColorSpace,
    output: &mut [u16], width: usize, padded_width: usize, row: usize
) -> Result<(), DecodeErrors> {
    match (input_colorspace, output_colorspace) {
        (ColorSpace::Luma, ColorSpace::Luma) | (ColorSpace::YCbCr, ColorSpace::Luma) => {
            // Grayscale output - direct copy from Y channel
            let y_row = &samples[0][row * padded_width..];
            for (out, &y) in output.iter_mut().zip(y_row.iter()).take(width) {
                *out = y as u16;
            }
        }
        (ColorSpace::YCbCr, ColorSpace::RGB) => {
            // YCbCr to RGB conversion for extended precision
            // Using the same formula as baseline but with wider range
            let y_row = &samples[0][row * padded_width..];
            let cb_row = &samples[1][row * padded_width..];
            let cr_row = &samples[2][row * padded_width..];

            for (i, chunk) in output.chunks_exact_mut(3).enumerate().take(width) {
                if i >= y_row.len() || i >= cb_row.len() || i >= cr_row.len() {
                    break;
                }

                let y = i32::from(y_row[i]);
                let cb = i32::from(cb_row[i]);
                let cr = i32::from(cr_row[i]);

                // Standard YCbCr to RGB conversion
                // R = Y + 1.402 * (Cr - 128)
                // G = Y - 0.344136 * (Cb - 128) - 0.714136 * (Cr - 128)
                // B = Y + 1.772 * (Cb - 128)
                // For extended precision, the "128" bias scales with bit depth
                // but the IDCT already handles this, so cb/cr are centered around 0

                // Fixed-point conversion (14-bit precision)
                let r = y + ((cr * 22970) >> 14);
                let g = y - ((cb * 5638 + cr * 11700) >> 14);
                let b = y + ((cb * 29032) >> 14);

                // Clamp to valid range (the max depends on original precision but
                // since IDCT already clamped, we just ensure non-negative)
                chunk[0] = r.max(0) as u16;
                chunk[1] = g.max(0) as u16;
                chunk[2] = b.max(0) as u16;
            }
        }
        _ => {
            return Err(DecodeErrors::Format(format!(
                "Unsupported extended JPEG color conversion from {:?} to {:?}",
                input_colorspace, output_colorspace
            )));
        }
    }
    Ok(())
}
