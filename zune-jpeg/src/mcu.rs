use std::cmp::min;

use zune_core::colorspace::ColorSpace;

use crate::bitstream::BitStream;
use crate::components::{ComponentID, SampleRatios};
use crate::decoder::MAX_COMPONENTS;
use crate::errors::DecodeErrors;
use crate::marker::Marker;
use crate::misc::{calculate_padded_width, setup_component_params};
use crate::worker::{color_convert_no_sampling, upsample_and_color_convert};
use crate::JpegDecoder;

/// The size of a DC block for a MCU.

pub const DCT_BLOCK: usize = 64;

impl<'a> JpegDecoder<'a>
{
    /// Check for existence of DC and AC Huffman Tables
    pub(crate) fn check_tables(&self) -> Result<(), DecodeErrors>
    {
        // check that dc and AC tables exist outside the hot path
        for i in 0..self.input_colorspace.num_components()
        {
            let _ = &self
                .dc_huffman_tables
                .get(self.components[i].dc_huff_table)
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No Huffman DC table for component {:?} ",
                        self.components[i].component_id
                    ))
                })?
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No DC table for component {:?}",
                        self.components[i].component_id
                    ))
                })?;

            let _ = &self
                .ac_huffman_tables
                .get(self.components[i].ac_huff_table)
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No Huffman AC table for component {:?} ",
                        self.components[i].component_id
                    ))
                })?
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No AC table for component {:?}",
                        self.components[i].component_id
                    ))
                })?;
        }
        Ok(())
    }

    /// Decode MCUs and carry out post processing.
    ///
    /// This is the main decoder loop for the library, the hot path.
    ///
    /// Because of this, we pull in some very crazy optimization tricks hence readability is a pinch
    /// here.
    #[allow(
        clippy::similar_names,
        clippy::too_many_lines,
        clippy::cast_possible_truncation
    )]
    #[inline(never)]
    pub(crate) fn decode_mcu_ycbcr_baseline(&mut self) -> Result<Vec<u8>, DecodeErrors>
    {
        setup_component_params(self)?;

        self.check_component_dimensions()?;
        // check dc and AC tables
        self.check_tables()?;

        let (mut mcu_width, mut mcu_height);

        if self.is_interleaved
        {
            // set upsampling functions
            self.set_upsampling()?;

            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;
        }
        else
        {
            // For non-interleaved images( (1*1) subsampling)
            // number of MCU's are the widths (+7 to account for paddings) divided bu 8.
            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }

        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved
        {
            if self.options.get_strict_mode()
            {
                return Err(DecodeErrors::FormatStatic(
                    "[strict-mode]: Grayscale image with down-sampled component."
                ));
            }

            warn!("Grayscale image with down-sampled component, resetting component details");

            self.reset_params();

            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }
        // Size of our output image(width*height)
        let capacity = usize::from(self.info.width + 8) * usize::from(self.info.height + 8);
        let is_hv = usize::from(self.sub_sample_ratio == SampleRatios::HV);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let out_colorspace_components = self.options.get_out_colorspace().num_components();
        let width = usize::from(self.info.width);
        let chunks_size = width * out_colorspace_components * 8 * self.h_max * self.v_max;
        let padded_width = calculate_padded_width(width, self.sub_sample_ratio);

        let mut stream = BitStream::new();
        let mut pixels = vec![128; capacity * out_colorspace_components];
        let mut chunks = pixels.chunks_mut(chunks_size);
        let mut channels: [Vec<i16>; MAX_COMPONENTS] = [vec![], vec![], vec![], vec![]];
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        let mut tmp = [0_i32; DCT_BLOCK];
        let mut bias = 0;

        for (pos, comp) in self.components.iter_mut().enumerate()
        {
            // Allocate only needed components.
            //
            // For special colorspaces i.e YCCK and CYMK, just allocate all of the needed
            // components.
            if min(self.options.get_out_colorspace().num_components() - 1, pos) == pos
                || self.input_colorspace == ColorSpace::YCCK
                || self.input_colorspace == ColorSpace::CMYK
            {
                let mut len = comp.width_stride * DCT_BLOCK / 8;

                len *= comp.vertical_sample * comp.horizontal_sample;

                if self.is_interleaved && comp.component_id != ComponentID::Y
                {
                    len *= 2;
                }

                comp.needed = true;
                channels[pos] = vec![0; len];
            }
            else
            {
                comp.needed = false;
            }

            if comp.needed && self.is_interleaved && (comp.component_id != ComponentID::Y)
            {
                comp.setup_upsample_scanline(self.h_max, self.v_max);
            }
        }

        for i in 0..mcu_height
        {
            // Report if we have no more bytes
            // This may generate false negatives since we overread bytes
            // hence that why 37 is chosen(we assume if we overread more than 37 bytes, we have a problem)
            if stream.overread_by > 37
            // favourite number :)
            {
                if self.options.get_strict_mode()
                {
                    return Err(DecodeErrors::FormatStatic("Premature end of buffer"));
                };

                error!("Premature end of buffer");
                break;
            }
            for j in 0..mcu_width
            {
                // iterate over components
                for (pos, component) in self.components.iter_mut().enumerate()
                {
                    let dc_table = self.dc_huffman_tables[component.dc_huff_table]
                        .as_ref()
                        .ok_or(DecodeErrors::FormatStatic("No DC table for a component"))?;
                    let ac_table = self.ac_huffman_tables[component.ac_huff_table]
                        .as_ref()
                        .ok_or(DecodeErrors::FormatStatic("No AC table for component"))?;

                    let qt_table = &component.quantization_table;
                    let channel = &mut channels[pos];
                    // If image is interleaved iterate over scan  components,
                    // otherwise if it-s non-interleaved, these routines iterate in
                    // trivial scanline order(Y,Cb,Cr)
                    for v_samp in 0..component.vertical_sample
                    {
                        for h_samp in 0..component.horizontal_sample
                        {
                            // Fill the array with zeroes, decode_mcu_block expects
                            // a zero based array.
                            tmp.fill(0);

                            stream.decode_mcu_block(
                                &mut self.stream,
                                dc_table,
                                ac_table,
                                qt_table,
                                &mut tmp,
                                &mut component.dc_pred
                            )?;

                            if component.needed
                            {
                                let idct_position = {
                                    // derived from stb and rewritten for my tastes
                                    let c2 = ((bias * component.vertical_sample) + v_samp) * 8;
                                    let c3 = ((j * component.horizontal_sample) + h_samp) * 8;

                                    component.width_stride * c2 + c3
                                };

                                let idct_pos = channel.get_mut(idct_position..).unwrap();
                                //  call idct.
                                (self.idct_func)(&mut tmp, idct_pos, component.width_stride);
                            }
                        }
                    }
                }
                self.todo = self.todo.saturating_sub(1);
                // After all interleaved components, that's an MCU
                // handle stream markers
                //
                // In some corrupt images, it may occur that header markers occur in the stream.
                // The spec EXPLICITLY FORBIDS this, specifically, in
                // routine F.2.2.5  it says
                // `The only valid marker which may occur within the Huffman coded data is the RSTm marker.`
                //
                // But libjpeg-turbo allows it because of some weird reason. so I'll also
                // allow it because of some weird reason.
                if let Some(m) = stream.marker
                {
                    if m == Marker::EOI
                    {
                        // acknowledge and ignore EOI marker.
                        stream.marker.take();
                        info!("Found EOI marker");
                    }
                    else if let Marker::RST(_) = m
                    {
                        if self.todo == 0
                        {
                            self.handle_rst(&mut stream)?;
                        }
                    }
                    else
                    {
                        if self.options.get_strict_mode()
                        {
                            return Err(DecodeErrors::Format(format!(
                                "Marker {m:?} found where not expected"
                            )));
                        }
                        error!(
                            "Marker `{:?}` Found within Huffman Stream, possibly corrupt jpeg",
                            m
                        );

                        self.parse_marker_inner(m)?;
                    }
                }
            }
            bias += 1;

            if i == 0 && self.is_interleaved && self.sub_sample_ratio != SampleRatios::H
            {
                // copy first row of idct to the upsampler
                // Needed for HV and V upsampling.
                self.components.iter_mut().for_each(|x| {
                    if x.needed && x.component_id != ComponentID::Y
                    {
                        //copy
                        let length = x.upsample_scanline.len();

                        x.upsample_scanline.copy_from_slice(
                            &channels[usize::from(x.id.saturating_sub(1))][0..length]
                        );
                    }
                });
            }

            if self.is_interleaved
            {
                if i == mcu_height - 1 // take last row even if it doesn't evenly divide it
                    || (self.sub_sample_ratio == SampleRatios::H && i % 2 == 1)
                    || (self.sub_sample_ratio == SampleRatios::V)
                    || (self.sub_sample_ratio == SampleRatios::HV && i % 2 == 1)
                {
                    // We have done a complete mcu width, we can upsample.

                    // reset counter.
                    // The next iteration should re-use the temp storage
                    // overwriting whatever was previously there.(that this iteration wrote)
                    bias = 0;

                    upsample_and_color_convert(
                        &channels,
                        &mut self.components,
                        padded_width,
                        self.color_convert_16,
                        self.input_colorspace,
                        self.options.get_out_colorspace(),
                        chunks.next().unwrap(),
                        width,
                        &mut upsampler_scratch_space
                    )?;
                }
            }
            else
            {
                // color convert expects a reference to channel data, so convert
                // it here.
                let mut channels_ref: [&[i16]; MAX_COMPONENTS] = [&[]; MAX_COMPONENTS];

                channels
                    .iter()
                    .enumerate()
                    .for_each(|(pos, x)| channels_ref[pos] = x);

                // Color convert.
                color_convert_no_sampling(
                    &channels_ref,
                    self.color_convert_16,
                    self.input_colorspace,
                    self.options.get_out_colorspace(),
                    chunks.next().unwrap(),
                    width,
                    padded_width
                )?;

                bias = 0;
            }
        }
        info!("Finished decoding image");
        // remove excess allocation for images.
        let actual_dims = usize::from(self.width())
            * usize::from(self.height())
            * self.options.get_out_colorspace().num_components();

        pixels.truncate(actual_dims);

        return Ok(pixels);
    }
    // handle RST markers.
    // No-op if not using restarts
    // this routine is shared with mcu_prog
    #[cold]
    pub(crate) fn handle_rst(&mut self, stream: &mut BitStream) -> Result<(), DecodeErrors>
    {
        self.todo = self.restart_interval;

        if let Some(marker) = stream.marker
        {
            // Found a marker
            // Read stream and see what marker is stored there
            match marker
            {
                Marker::RST(_) =>
                {
                    // reset stream
                    stream.reset();
                    // Initialize dc predictions to zero for all components
                    self.components.iter_mut().for_each(|x| x.dc_pred = 0);
                    // Start iterating again. from position.
                }
                Marker::EOI =>
                {
                    // silent pass
                }
                _ =>
                {
                    return Err(DecodeErrors::MCUError(format!(
                        "Marker {marker:?} found in bitstream, possibly corrupt jpeg"
                    )));
                }
            }
        }
        Ok(())
    }
}
