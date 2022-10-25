//!Routines for progressive decoding
//!
use std::cmp::min;

use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;

use crate::bitstream::BitStream;
use crate::components::{ComponentID, SubSampRatios};
use crate::decoder::{JpegDecoder, MAX_COMPONENTS};
use crate::errors::DecodeErrors;
use crate::errors::DecodeErrors::Format;
use crate::headers::{parse_huffman, parse_sos};
use crate::marker::Marker;
use crate::mcu::DCT_BLOCK;
use crate::worker::{color_convert_no_sampling, upsample_and_color_convert};

impl<'a> JpegDecoder<'a>
{
    /// Decode a progressive image
    ///
    /// This routine decodes a progressive image, stopping if it finds any error.
    #[rustfmt::skip]
    #[allow(clippy::needless_range_loop,clippy::cast_sign_loss)]
    pub(crate) fn decode_mcu_ycbcr_progressive(
        &mut self
    ) -> Result<Vec<u8>, DecodeErrors>
    {
        self.check_component_dimensions()?;
        let mcu_height;

        // memory location for decoded pixels for components
        let mut block = [vec![], vec![], vec![]];
        let mut mcu_width;

        let mut seen_scans = 1;

        if self.is_interleaved
        {
            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;
        } else {
            mcu_width = (self.info.width as usize + 7) / 8;
            mcu_height = (self.info.height as usize + 7) / 8;
        }

        mcu_width *= 64;

        for i in 0..self.input_colorspace.num_components()
        {
            let comp = &self.components[i];
            let len = mcu_width * comp.vertical_sample * comp.horizontal_sample * mcu_height;

            block[i] = vec![0; len];
        }

        let mut stream = BitStream::new_progressive(self.succ_high, self.succ_low,
                                                    self.spec_start, self.spec_end);

        // there are multiple scans in the stream, this should resolve the first scan
        self.parse_entropy_coded_data(&mut stream, &mut block)?;

        // extract marker
        let mut marker = stream.marker.take().ok_or(DecodeErrors::FormatStatic("Marker missing where expected"))?;
        // if marker is EOI, we are done, otherwise continue scanning.
        'eoi: while marker != Marker::EOI
        {

            match marker
            {
                Marker::DHT => {
                    parse_huffman(self)?;
                }
                Marker::SOS =>
                    {
                        parse_sos(self)?;

                        stream.update_progressive_params(self.succ_high, self.succ_low,
                                                         self.spec_start, self.spec_end);

                        // after every SOS, marker, parse data for that scan.
                        self.parse_entropy_coded_data(&mut stream, &mut block)?;
                        // extract marker, might either indicate end of image or we continue
                        // scanning(hence the continue statement to determine).
                        marker = get_marker(&mut self.stream, &mut stream)?;
                        seen_scans+=1;

                        if seen_scans >  self.options.get_max_scans(){
                            return Err(DecodeErrors::Format(format!("Too many scans, exceeded limit of {}", self.options.get_max_scans())))
                        }

                        stream.reset();
                        continue 'eoi;
                    }
                _ =>
                    {
                        break 'eoi;
                    }
            }

            marker = get_marker(&mut self.stream, &mut stream)?;
        }

        self.finish_progressive_decoding(&block, mcu_width)
    }

    #[allow(clippy::too_many_lines)]
    #[allow(clippy::needless_range_loop, clippy::cast_sign_loss)]
    fn finish_progressive_decoding(
        &mut self, block: &[Vec<i16>; 3], _mcu_width: usize,
    ) -> Result<Vec<u8>, DecodeErrors>
    {
        let (mut mcu_width, mut mcu_height);

        let mut bias = 0;

        let mut v_counter = 1;
        if self.is_interleaved
        {
            // set upsampling functions
            self.set_upsampling()?;

            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;

            if self.sub_sample_ratio == SubSampRatios::V
            {
                mcu_width = self.mcu_x / 2;
                v_counter = 2;
            }
        }
        else
        {
            // For non-interleaved images( (1*1) subsampling)
            // number of MCU's are the widths (+7 to account for paddings) divided by 8.
            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }

        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved
        {
            /*
            Apparently, grayscale images which can be down sampled exists, which is weird in the sense
            that it has one component Y, which is not usually down sampled.

            This means some calculations will be wrong, so for that we explicitly reset params
            for such occurrences, warn and reset the image info to appear as if it were
            a non-sampled image to ensure decoding works
            */
            if self.options.get_strict_mode()
            {
                return Err(DecodeErrors::FormatStatic(
                    "[strict-mode]: Grayscale image with down-sampled component.",
                ));
            }

            warn!("Grayscale image with down-sampled component, resetting component details");

            mcu_width = ((self.info.width + 7) / 8) as usize;
            self.h_max = 1;
            self.options = self.options.set_out_colorspace(ColorSpace::Luma);
            self.v_max = 1;
            self.sub_sample_ratio = SubSampRatios::None;
            self.components[0].vertical_sample = 1;
            self.components[0].width_stride = mcu_width * 8;
            self.components[0].horizontal_sample = 1;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }
        // Size of our output image(width*height)
        let capacity = usize::from(self.info.width + 8) * usize::from(self.info.height + 8);
        let is_hv = usize::from(self.sub_sample_ratio == SubSampRatios::HV);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let out_colorspace_components = self.options.get_out_colorspace().num_components();
        let width = usize::from(self.info.width);
        let chunks_size = width * out_colorspace_components * 8 * self.h_max * self.v_max;

        let mut pixels = vec![255; capacity * out_colorspace_components];
        let mut chunks = pixels.chunks_exact_mut(chunks_size);
        let mut temporary = [vec![], vec![], vec![]];
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        let mut tmp = [0_i32; DCT_BLOCK];

        for (pos, comp) in self.components.iter_mut().enumerate()
        {
            // Allocate only needed components.
            if min(self.options.get_out_colorspace().num_components() - 1, pos) == pos
            {
                let mut len = comp.width_stride * DCT_BLOCK / 8;

                len *= comp.vertical_sample * comp.horizontal_sample;

                if self.sub_sample_ratio == SubSampRatios::H && comp.component_id != ComponentID::Y
                {
                    len *= 2;
                }

                comp.needed = true;
                temporary[pos] = vec![0; len];
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
            for _ in 0..v_counter
            {
                for j in 0..mcu_width
                {
                    // iterate over components
                    for pos in 0..self.input_colorspace.num_components()
                    {
                        let component = &mut self.components[pos];

                        if !component.needed
                        {
                            continue;
                        }

                        let qt_table = &component.quantization_table;
                        let data_block = &block[pos & 3];

                        let channel = &mut temporary[pos & 3];

                        for v_samp in 0..component.vertical_sample
                        {
                            for h_samp in 0..component.horizontal_sample
                            {
                                data_block[component.counter..component.counter + 64]
                                    .iter()
                                    .zip(tmp.iter_mut())
                                    .zip(qt_table.iter())
                                    .for_each(|((x, out), qt_val)| {
                                        *out = i32::from(*x) * qt_val;
                                    });

                                let idct_position = {
                                    if self.is_interleaved
                                    {
                                        if self.sub_sample_ratio == SubSampRatios::H
                                        {
                                            let c2 =
                                                ((bias * component.vertical_sample) + v_samp) * 8;
                                            let c3 =
                                                ((j * component.horizontal_sample) + h_samp) * 8;

                                            component.width_stride * c2 + c3
                                        }
                                        else
                                        {
                                            component.idct_pos
                                        }
                                    }
                                    else
                                    {
                                        j * 8
                                    }
                                };

                                let idct_pos = channel.get_mut(idct_position..).unwrap();
                                (self.idct_func)(&mut tmp, idct_pos, component.width_stride);

                                component.counter += 64;
                                component.idct_pos += 8;
                            }
                        }
                    }
                }

                // After iterating every halfway through a hv unsampled image.
                // Bump up idct pos to point 1 MCU row downward.
                // This ensures the other iteration starts one MCU down
                for component in &mut self.components
                {
                    if component.component_id == ComponentID::Y
                    {
                        component.idct_pos = 8 * component.width_stride;
                        break;
                    }
                }
            }
            bias += 1;

            if i == 0
            {
                // copy first row of idct to the upsampler
                // Needed for HV and V upsampling.
                self.components.iter_mut().for_each(|x| {
                    if x.needed && self.is_interleaved && x.component_id != ComponentID::Y
                    {
                        //copy
                        let length = x.upsample_scanline.len();

                        x.upsample_scanline.copy_from_slice(
                            &temporary[usize::from(x.id.saturating_sub(1))][0..length],
                        );
                    }
                });
            }
            if self.is_interleaved
            {
                if (self.sub_sample_ratio == SubSampRatios::H && i % 2 == 1)
                    || (self.sub_sample_ratio == SubSampRatios::V)
                    || (self.sub_sample_ratio == SubSampRatios::HV && i % 2 == 1)
                {
                    bias = 0;
                    self.components.iter_mut().for_each(|x| x.idct_pos = 0);
                    // We have done a complete mcu width, we can upsample.

                    upsample_and_color_convert(
                        &temporary,
                        &mut self.components,
                        self.sub_sample_ratio,
                        self.color_convert_16,
                        self.input_colorspace,
                        self.options.get_out_colorspace(),
                        chunks.next().unwrap(),
                        width,
                        &mut upsampler_scratch_space,
                    );
                }
            }
            else
            {
                let mut un_t: [&[i16]; 3] = [&[]; 3];

                self.components.iter_mut().for_each(|x| x.idct_pos = 0);

                temporary
                    .iter()
                    .enumerate()
                    .for_each(|(pos, x)| un_t[pos] = x);

                // Color convert.
                color_convert_no_sampling(
                    &un_t,
                    self.color_convert_16,
                    self.input_colorspace,
                    self.options.get_out_colorspace(),
                    chunks.next().unwrap(),
                    width,
                );
            }
        }

        debug!("Finished decoding image");

        let actual_dims = usize::from(self.width())
            * usize::from(self.height())
            * self.options.get_out_colorspace().num_components();

        pixels.truncate(actual_dims);

        return Ok(pixels);
    }


    #[rustfmt::skip]
    #[allow(clippy::too_many_lines,clippy::cast_sign_loss)]
    fn parse_entropy_coded_data(
        &mut self, stream: &mut BitStream, buffer: &mut [Vec<i16>; 3],
    ) -> Result<(), DecodeErrors>
    {
        self.check_component_dimensions()?;
        stream.reset();
        self.components.iter_mut().for_each(|x| x.dc_pred = 0);

        if usize::from(self.num_scans) > self.input_colorspace.num_components() {
            return Err(Format(format!("Number of scans {} cannot be greater than number of components, {}", self.num_scans, self.input_colorspace.num_components())));
        }

        if self.num_scans == 1
        {
            // Safety checks
            if self.spec_end != 0 && self.spec_start == 0
            {
                return Err(DecodeErrors::FormatStatic(
                    "Can't merge DC and AC corrupt jpeg"
                ));
            }
            // non interleaved data, process one block at a time in trivial scanline order

            let k = self.z_order[0];

            if k >= self.components.len() {
                return Err(DecodeErrors::Format(format!("Cannot find component {}, corrupt image", k)));
            }

            let (mcu_width, mcu_height);
            // For Y channel  or non interleaved scans , mcu's is the image dimensions divided
            // by 8
            if self.components[k].component_id == ComponentID::Y || !self.is_interleaved
            {
                mcu_width = ((self.info.width + 7) / 8) as usize;
                mcu_height = ((self.info.height + 7) / 8) as usize;
            } else {
                // For other channels, in an interleaved mcu, number of MCU's
                // are determined by some weird maths done in headers.rs->parse_sos()
                mcu_width = self.mcu_x;
                mcu_height = self.mcu_y;
            }
            let mut i = 0;
            let mut j = 0;

            while i < mcu_height
            {
                while j < mcu_width
                {
                    let start = 64 * (j + i * (self.components[k].width_stride / 8));

                    if i >= mcu_height
                    {
                        break;
                    }

                    let data: &mut [i16; 64] = buffer.get_mut(k)
                        .unwrap().get_mut(start..start + 64)
                        .unwrap().try_into().unwrap();

                    if self.spec_start == 0
                    {
                        let pos = self.components[k].dc_huff_table & (MAX_COMPONENTS - 1);
                        let dc_table = self.dc_huffman_tables.get(pos)
                            .ok_or(DecodeErrors::FormatStatic("No huffman table for DC component"))?
                            .as_ref()
                            .ok_or(DecodeErrors::FormatStatic("Huffman table at index  {} not initialized"))?;

                        let dc_pred = &mut self.components[k].dc_pred;

                        if self.succ_high == 0
                        {
                            // first scan for this mcu
                            stream.decode_prog_dc_first(&mut self.stream, dc_table, &mut data[0], dc_pred)?;
                        } else {
                            // refining scans for this MCU
                            stream.decode_prog_dc_refine(&mut self.stream, &mut data[0])?;
                        }

                    } else {
                        let pos = self.components[k].ac_huff_table;
                        let ac_table = self.ac_huffman_tables.get(pos)
                            .ok_or_else(|| DecodeErrors::Format(format!("No huffman table for component:{}", pos)))?
                            .as_ref()
                            .ok_or_else(|| DecodeErrors::Format(format!("Huffman table at index  {} not initialized", pos)))?;

                        if self.succ_high == 0
                        {
                            // first scan for this MCU
                            if stream.eob_run > 0
                            {
                                // EOB runs indicate the whole block is empty, but unlike for baseline
                                // EOB in progressive tell us the number of proceeding blocks currently zero.

                                // other decoders use a check in decode_mcu_first decrement and return if it's an
                                // eob run(since the array is expected to contain zeroes). but that's a function call overhead(if not inlined) and a branch check
                                // we do it a bit differently
                                // we can use divisors to determine how many MCU's to skip
                                // which is more faster than a decrement and return since EOB runs can be
                                // as big as 10,000

                                i += (j + stream.eob_run as usize - 1) / mcu_width;
                                j = (j + stream.eob_run as usize - 1) % mcu_width;
                                stream.eob_run = 0;
                            } else {
                                stream.decode_mcu_ac_first(&mut self.stream, ac_table, data)?;
                            }
                        } else {
                            // refinement scan
                            stream.decode_mcu_ac_refine(&mut self.stream, ac_table, data)?;
                        }
                    }
                    j += 1;
                    // TODO, look into RST streams 
                    // + EOB and investigate effect.
                    self.todo -= 1;

                    if self.todo == 0
                    {
                        self.handle_rst(stream)?;
                    }
                }
                j = 0;
                i += 1;
            }
        } else {
            if self.spec_end != 0
            {
                return Err(DecodeErrors::HuffmanDecode(
                    "Can't merge dc and AC corrupt jpeg".to_string(),
                ));
            }
            // process scan n elements in order

            // Do the error checking with allocs here.
            // Make the one in the inner loop free of allocations.
            for k in 0..self.num_scans
            {
                let n = self.z_order[k as usize];

                if n >= self.components.len() {
                    return Err(DecodeErrors::Format(format!("Cannot find component {}, corrupt image", n)));
                }

                let component = &mut self.components[n];
                let _ = self.dc_huffman_tables.get(component.dc_huff_table)
                    .ok_or_else(|| DecodeErrors::Format(format!("No huffman table for component:{}", component.dc_huff_table)))?
                    .as_ref()
                    .ok_or_else(|| DecodeErrors::Format(format!("Huffman table at index  {} not initialized", component.dc_huff_table)))?;
            }
                // Interleaved scan

            // Components shall not be interleaved in progressive mode, except for
            // the DC coefficients in the first scan for each component of a progressive frame.
            for i in 0..self.mcu_y
            {
                for j in 0..self.mcu_x
                {
                    // process scan n elements in order
                    for k in 0..self.num_scans
                    {
                        let n = self.z_order[k as usize];
                        let component = &mut self.components[n];
                        let huff_table = self.dc_huffman_tables.get(component.dc_huff_table)
                            .ok_or(DecodeErrors::FormatStatic("No huffman table for component"))?
                            .as_ref()
                            .ok_or(DecodeErrors::FormatStatic("Huffman table at index not initialized"))?;

                        for v_samp in 0..component.vertical_sample
                        {
                            for h_samp in 0..component.horizontal_sample
                            {
                                let x2 = j * component.horizontal_sample + h_samp;
                                let y2 = i * component.vertical_sample + v_samp;
                                let position = 64 * (x2 + y2 * component.width_stride / 8);

                                let data = &mut buffer[n as usize][position];

                                if self.succ_high == 0
                                {
                                    stream.decode_prog_dc_first(&mut self.stream, huff_table, data, &mut component.dc_pred)?;
                                } else {
                                    stream.decode_prog_dc_refine(&mut self.stream, data)?;
                                }
                            }
                        }
                    }
                    // We want wrapping subtraction here because it means
                    // we get a higher number in the case this underflows
                    self.todo = self.todo.wrapping_sub(1);
                    // after every scan that's a mcu, count down restart markers.
                    if self.todo == 0 {
                        self.handle_rst(stream)?;
                    }
                }
            }
        }
        return Ok(());
    }
}

///Get a marker from the bit-stream.
///
/// This reads until it gets a marker or end of file is encountered
fn get_marker(reader: &mut ZByteReader, stream: &mut BitStream) -> Result<Marker, DecodeErrors>
{
    if let Some(marker) = stream.marker
    {
        stream.marker = None;
        return Ok(marker);
    }

    // read until we get a marker

    while !reader.eof()
    {
        let marker = reader.get_u8_err()?;

        if marker == 255
        {
            let mut r = reader.get_u8_err()?;
            // 0xFF 0XFF(some images may be like that)
            while r == 0xFF
            {
                r = reader.get_u8_err()?;
            }

            if r != 0
            {
                return Marker::from_u8(r)
                    .ok_or_else(|| DecodeErrors::Format(format!("Unknown marker 0xFF{:X}", r)));
            }
        }
    }
    return Err(DecodeErrors::from("No more bytes"));
}
