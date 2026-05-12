use crate::decoder::{PLTEEntry, PngChunk};
use crate::enums::{FilterMethod, PngChunkType, PngColor};
use crate::error::PngDecodeErrors;
use crate::filters::de_filter::{
    handle_avg, handle_avg_first, handle_paeth, handle_paeth_first, handle_sub, handle_up,
};
use crate::utils::{add_alpha, expand_bits_to_byte, expand_palette, expand_trns};
use crate::{InterlaceMethod, PngDecoder};
use zune_core::bytestream::ZByteReaderTrait;
use zune_inflate::DecodeStatus;

const XORIG: [usize; 7] = [0, 4, 0, 2, 0, 1, 0];
const YORIG: [usize; 7] = [0, 0, 4, 0, 2, 0, 1];
const XSPC: [usize; 7] = [8, 8, 4, 4, 2, 2, 1];
const YSPC: [usize; 7] = [8, 8, 8, 4, 4, 2, 2];

// this is the single read size per decode of deflate
// This is to match the default PNG done by libpng, so means a general deflate
// will do 8 KB of decompressing a time
// NB: Other encoders like Photoshop do 32 KB, so if its beneficial we can switch to that
const BUF_READ: usize = 8192;
// Maximum deflate history that can be used by an image
const MAX_DEFLATE_HISTORY: usize = 32_768;

struct InterlaceState {
    current_pass: usize,
    pass_w: usize,
    pass_h: usize,
    current_row_idx: usize,
    row_size: usize,
    width_stride: usize,
    out_chunk_size: usize,
    prev_row_buf: Vec<u8>,
    curr_row_buf: Vec<u8>,
    filter_components: usize,
    is_complete: bool,
}

impl<T> PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    fn adam7_dimensions(&self, pass: usize) -> (usize, usize) {
        let w = self.png_info.width;
        let h = self.png_info.height;

        let pass_w = (w
            .saturating_sub(XORIG[pass])
            .saturating_add(XSPC[pass])
            .saturating_sub(1))
            / XSPC[pass];
        let pass_h = (h
            .saturating_sub(YORIG[pass])
            .saturating_add(YSPC[pass])
            .saturating_sub(1))
            / YSPC[pass];

        (pass_w, pass_h)
    }
    fn calculate_pass_row_size(&self, pass_w: usize) -> usize {
        let bpp =
            usize::from(self.png_info.color.num_components()) * usize::from(self.png_info.depth);
        let row_bits = pass_w * bpp;
        let row_bytes = row_bits.div_ceil(8);
        row_bytes + 1 // +1 for the filter byte
    }
    fn scatter_interlaced_row(
        &self,
        pass: usize,
        pass_y: usize, // current_row_idx in this pass
        pass_w: usize,
        post_processed_row: &[u8],
        final_image_out: &mut [u8],
    ) {
        let bytes_per_pixel = self.colorspace().unwrap().num_components()
            * if self.png_info.depth == 16 { 2 } else { 1 };

        let out_y = pass_y * YSPC[pass] + YORIG[pass];

        for i in 0..pass_w {
            let out_x = i * XSPC[pass] + XORIG[pass];

            let final_start = (out_y * self.png_info.width + out_x) * bytes_per_pixel;
            let src_start = i * bytes_per_pixel;

            if let Some(dest) = final_image_out.get_mut(final_start..final_start + bytes_per_pixel)
            {
                dest.copy_from_slice(&post_processed_row[src_start..src_start + bytes_per_pixel]);
            }
        }
    }

    fn extract_rows_interlaced(
        &mut self, deflate_buf: &[u8], processed_bytes: &mut usize, decode_dest: usize,
        final_out: &mut [u8], state: &mut InterlaceState,
    ) -> Result<(), PngDecodeErrors> {
        let num_components = self.colorspace().unwrap().num_components();
        while (decode_dest - *processed_bytes) >= state.row_size && !state.is_complete {
            let in_stride = &deflate_buf[*processed_bytes..*processed_bytes + state.row_size];
            let filter_byte = in_stride[0];
            let raw = &in_stride[1..];

            let will_post_process = self.will_post_process();
            // 1. Un-filter into `curr_row_buf`
            let is_first_row = state.current_row_idx == 0;
            self.apply_filter(
                filter_byte,
                &state.prev_row_buf[..state.width_stride],
                raw,
                &mut state.curr_row_buf[..state.width_stride],
                is_first_row,
                state.filter_components,
            )?;

            // 2. Post-Process and Scatter the PREVIOUS row (The 2-row lag)
            if state.current_row_idx > 0 {
                let prev_row = &mut state.prev_row_buf[..state.out_chunk_size];

                if will_post_process {
                    self.post_process_row(prev_row, state.width_stride, state.pass_w)?;
                }

                self.scatter_interlaced_row(
                    state.current_pass,
                    state.current_row_idx - 1,
                    state.pass_w,
                    prev_row,
                    final_out,
                );
            }

            // 3. Swap buffers for the next row
            std::mem::swap(&mut state.prev_row_buf, &mut state.curr_row_buf);

            *processed_bytes += state.row_size;
            state.current_row_idx += 1;

            // 4. Check for pass completion
            if state.current_row_idx == state.pass_h {
                // Post-process and scatter the trailing row of the pass
                let final_row = &mut state.prev_row_buf[..state.out_chunk_size];

                if will_post_process {
                    self.post_process_row(final_row, state.width_stride, state.pass_w)?;
                }

                self.scatter_interlaced_row(
                    state.current_pass,
                    state.current_row_idx - 1,
                    state.pass_w,
                    final_row,
                    final_out,
                );

                // Advance pass and skip empty passes
                state.current_pass += 1;
                loop {
                    if state.current_pass > 6 {
                        state.is_complete = true;
                        return Ok(());
                    }
                    let dims = self.adam7_dimensions(state.current_pass);
                    state.pass_w = dims.0;
                    state.pass_h = dims.1;
                    if state.pass_w > 0 && state.pass_h > 0 {
                        break; // Found a pass with actual pixels
                    }
                    state.current_pass += 1;
                }

                // Recalculate dimensions for the new pass
                state.row_size = self.calculate_pass_row_size(state.pass_w);
                state.width_stride = state.row_size - 1;

                let bytes_per_pixel = if self.png_info.depth == 16 { 2 } else { 1 };
                state.out_chunk_size = state.pass_w * num_components * bytes_per_pixel;
                state.current_row_idx = 0;
            }
        }
        Ok(())
    }

    pub fn decode_stream_interlaced(
        &mut self, final_out: &mut [u8],
    ) -> Result<(), PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner(false)?;
        }

        //  Find the first non-empty pass
        let mut start_pass = 0;
        let (mut initial_pass_w, mut initial_pass_h) = self.adam7_dimensions(start_pass);
        while initial_pass_w == 0 || initial_pass_h == 0 {
            start_pass += 1;
            if start_pass > 6 {
                return Ok(());
            }
            // Completely empty image
            let dims = self.adam7_dimensions(start_pass);
            initial_pass_w = dims.0;
            initial_pass_h = dims.1;
        }

        // --- 2. Initialize State ---
        let bytes_per_pixel = if self.png_info.depth == 16 { 2 } else { 1 };
        let mut filter_components =
            usize::from(self.png_info.color.num_components()) * bytes_per_pixel;
        if self.png_info.depth < 8 {
            filter_components = 1;
        }

        let max_row_size = self.calculate_pass_row_size(self.png_info.width);
        let max_out_chunk =
            self.png_info.width * self.colorspace().unwrap().num_components() * bytes_per_pixel;

        let initial_row_size = self.calculate_pass_row_size(initial_pass_w);

        let mut state = InterlaceState {
            current_pass: start_pass,
            pass_w: initial_pass_w,
            pass_h: initial_pass_h,
            current_row_idx: 0,
            row_size: initial_row_size,
            width_stride: initial_row_size - 1,
            out_chunk_size: initial_pass_w
                * self.colorspace().unwrap().num_components()
                * bytes_per_pixel,
            prev_row_buf: vec![0; std::cmp::max(max_row_size, max_out_chunk)],
            curr_row_buf: vec![0; std::cmp::max(max_row_size, max_out_chunk)],
            filter_components,
            is_complete: false,
        };

        if self.will_post_process() {
            self.previous_stride.resize(max_out_chunk, 0);
        }

        // --- 3. Setup Deflate ---
        let buf_size = std::cmp::max(65536, MAX_DEFLATE_HISTORY + max_row_size + 4096);
        // allocations we make, buf_size is generally enough to hold 2 rows, including
        //
        // Furthermore, we split it into 1 allocation of two buffers so that we reduce alloc pressure
        // the buffer is then split into 2 separate buffers and used for first reading the zlib
        // data into temporary buffer and then second for storing decoded bytes from the buffer
        let mut major_buf = vec![0u8; buf_size + BUF_READ];

        let (byte_buf, deflate_buf) = major_buf.split_at_mut(BUF_READ);

        let mut decoder = zune_inflate::StreamingDecoder::new();
        let mut processed_bytes = 0;

        let mut skipped_zlib_header = false;
        let mut is_final_chunk = false;

        // --- 4. The Streaming Loop ---
        loop {
            if self.current_idat_bytes_left == 0 && !is_final_chunk {
                // Skip the CRC of the previous IDAT chunk
                self.stream.skip(4)?;

                let header = self.read_chunk_header()?;

                if header.chunk_type != PngChunkType::IDAT {
                    is_final_chunk = true;
                    self.current_idat_bytes_left = 0;
                } else {
                    self.current_idat_bytes_left = header.length;
                }
            }

            let read_len = std::cmp::min(BUF_READ, self.current_idat_bytes_left);

            let chunk_size = if read_len > 0 {
                self.stream.read_bytes(&mut byte_buf[..read_len])?
            } else {
                0
            };

            self.current_idat_bytes_left -= chunk_size;
            // we updated the buffer,so reset position to
            // zero
            decoder.reset_position();

            let mut chunk_pos = 0;
            // skip the ZLIB chunk
            if !skipped_zlib_header && chunk_size >= 2 {
                chunk_pos += 2;
                skipped_zlib_header = true;
            }

            loop {
                let input_slice: &[u8] = if chunk_size == 0 && is_final_chunk {
                    &[]
                } else {
                    &byte_buf[chunk_pos..chunk_size]
                };

                let status = decoder.decode_chunk(input_slice, is_final_chunk, deflate_buf);

                match status {
                    DecodeStatus::NeedsMoreInput => break,

                    DecodeStatus::NeedsMoreOutput { .. } => {
                        self.extract_rows_interlaced(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            final_out,
                            &mut state,
                        )?;

                        if state.is_complete {
                            return Ok(());
                        }

                        let unread_bytes = decoder.current_dest_offset() - processed_bytes;
                        let keep_amount = std::cmp::max(32_768, unread_bytes);
                        let slide_amount =
                            decoder.current_dest_offset().saturating_sub(keep_amount);

                        if slide_amount > 0 {
                            deflate_buf.copy_within(slide_amount..decoder.current_dest_offset(), 0);
                            decoder.slide_window(slide_amount);
                            processed_bytes -= slide_amount;
                        }
                    }

                    DecodeStatus::Finished => {
                        self.extract_rows_interlaced(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            final_out,
                            &mut state,
                        )?;
                        return Ok(());
                    }

                    DecodeStatus::Error(e) => return Err(PngDecodeErrors::ZlibDecodeErrors(e)),
                    _ => unreachable!(),
                }
            }
        }
    }

    fn calculate_row_size(&self) -> usize {
        // 1. Get bits per pixel (components * bit depth)
        let bpp =
            usize::from(self.png_info.color.num_components()) * usize::from(self.png_info.depth);

        // 2. Total bits in a row
        let row_bits = self.png_info.width * bpp;

        // 3. Convert to bytes, rounding up (e.g. 5 bits -> 1 byte)
        let row_bytes = row_bits.div_ceil(8);

        // 4. Add the filter byte
        row_bytes + 1
    }
    pub fn decode_stream_raw(&mut self) -> Result<Vec<u8>, PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner(false)?;
        }
        let mut final_out = vec![0u8; self.output_buffer_size().unwrap()];
        if self.png_info.interlace_method == InterlaceMethod::Standard {
            self.decode_stream(&mut final_out)?;
        } else {
            self.decode_stream_interlaced(&mut final_out)?;
        }
        Ok(final_out)
    }
    pub(crate) fn decode_stream(&mut self, final_out: &mut [u8]) -> Result<(), PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner(false)?;
        }
        let row_size = self.calculate_row_size();

        let buf_size = std::cmp::max(65536, MAX_DEFLATE_HISTORY + row_size + 4096);
        // allocations we make, buf_size is generally enough to hold 2 rows, including
        //
        // Furthermore, we split it into 1 allocation of two buffers so that we reduce alloc pressure
        // the buffer is then split into 2 separate buffers and used for first reading the zlib
        // data into temporary buffer and then second for storing decoded bytes from the buffer
        let mut major_buf = vec![0u8; buf_size + BUF_READ];

        let (byte_buf, deflate_buf) = major_buf.split_at_mut(BUF_READ);

        let mut decoder = zune_inflate::StreamingDecoder::new();

        // Trackers for Deflate state
        let mut processed_bytes = 0;
        // Trackers for PNG row state
        let width_stride = row_size - 1;
        // 3. The final output size per row (post-processed)
        let bytes_per_channel = if self.png_info.depth == 16 { 2 } else { 1 };
        let out_chunk_size =
            self.png_info.width * self.colorspace().unwrap().num_components() * bytes_per_channel;
        let mut current_row_idx = 0;
        let mut final_out_pos = 0;

        // Zlib streams have a 2-byte header we must skip manually before raw Deflate
        let mut skipped_zlib_header = false;

        if self.will_post_process() {
            self.previous_stride.resize(out_chunk_size, 0);
        }
        let mut is_final_chunk = false;

        // 2. The Streaming Loop
        loop {
            // If we exhausted the current IDAT chunk, read the next one
            if self.current_idat_bytes_left == 0 {
                // Skip the CRC of the previous IDAT chunk
                self.stream.skip(4)?;

                let header = self.read_chunk_header()?;

                if header.chunk_type != PngChunkType::IDAT {
                    // We are out of IDAT chunks
                    is_final_chunk = true;
                    self.current_idat_bytes_left = 0;
                } else {
                    self.current_idat_bytes_left = header.length;
                }
            }

            // Read a chunk of compressed data from the stream
            // (Read up to 4KB at a time, or whatever is left in the IDAT)
            let read_len = std::cmp::min(byte_buf.len(), self.current_idat_bytes_left);

            let chunk_size = { self.stream.read_bytes(&mut byte_buf[..read_len])? };
            self.current_idat_bytes_left -= chunk_size;
            // We just overwrote the buffer with fresh data.
            decoder.reset_position();

            let mut chunk_pos = 0;

            if !skipped_zlib_header {
                // Skip 2 bytes of Zlib header
                chunk_pos += 2;
                skipped_zlib_header = true;
            }

            loop {
                let input_slice: &[u8] = if self.current_idat_bytes_left == 0 && is_final_chunk {
                    &[]
                } else {
                    &byte_buf[chunk_pos..chunk_size]
                };

                // on the very next iteration before it can be read again.
                match decoder.decode_chunk(input_slice, is_final_chunk, deflate_buf) {
                    DecodeStatus::NeedsMoreInput => {
                        // Current chunk fully consumed. Break to read more from stream.
                        break;
                    }

                    DecodeStatus::NeedsMoreOutput {
                        at_least: _at_least,
                    } => {
                        // Extract any fully decompressed rows before sliding
                        self.extract_rows(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            row_size,
                            &mut current_row_idx,
                            final_out,
                            &mut final_out_pos,
                            out_chunk_size,
                        )?;

                        // Now slide the window back to preserve the 32 KB history
                        let unread_bytes = decoder.current_dest_offset() - processed_bytes;
                        let keep_amount = std::cmp::max(MAX_DEFLATE_HISTORY, unread_bytes);
                        let slide_amount =
                            decoder.current_dest_offset().saturating_sub(keep_amount);

                        if slide_amount > 0 {
                            deflate_buf.copy_within(slide_amount..decoder.current_dest_offset(), 0);
                            decoder.slide_window(slide_amount);
                            processed_bytes -= slide_amount;
                        }

                        // Do not break; loop around to retry decode_chunk with the SAME input
                    }

                    DecodeStatus::Finished => {
                        // Extract the final remaining rows
                        self.extract_rows(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            row_size,
                            &mut current_row_idx,
                            final_out,
                            &mut final_out_pos,
                            out_chunk_size,
                        )?;

                        // Trigger the final post-process step for the very last row here
                        self.post_process_final_row(
                            final_out,
                            current_row_idx,
                            out_chunk_size,
                            width_stride,
                        )?;

                        return Ok(());
                    }

                    DecodeStatus::Error(e) => {
                        return Err(PngDecodeErrors::ZlibDecodeErrors(e));
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn extract_rows(
        &mut self, deflate_buf: &[u8], processed_bytes: &mut usize, decode_dest: usize,
        row_size: usize, current_row_idx: &mut usize, final_out: &mut [u8],
        final_out_pos: &mut usize, out_chunk_size: usize,
    ) -> Result<(), PngDecodeErrors> {
        let width_stride = row_size - 1;
        // As long as we have enough bytes for a full row
        while (decode_dest - *processed_bytes) >= row_size {
            let in_stride = &deflate_buf[*processed_bytes..*processed_bytes + row_size];
            let filter_byte = in_stride[0];
            let raw = &in_stride[1..];

            let bytes_per_channel = if self.png_info.depth == 16 { 2 } else { 1 };
            let filter_bpp = usize::from(self.png_info.color.num_components()) * bytes_per_channel;

            // 1. Grab the slice of the final_out buffer using the EXPANDED size
            let (prev_data, current_dest) = final_out.split_at_mut(*final_out_pos);
            let current = &mut current_dest[0..out_chunk_size];
            // 2. Identify the previous row for un-filtering (or dummy row if first)
            let prev_row = if *current_row_idx == 0 {
                &[0_u8]
            } else {
                &prev_data[*final_out_pos - out_chunk_size..*final_out_pos]
            };
            let is_first_row = *current_row_idx == 0;
            // 3. Un-filter the row
            self.apply_filter(
                filter_byte,
                prev_row,
                raw,
                &mut current[..width_stride],
                is_first_row,
                filter_bpp,
            )?;

            // 4. Post-Process the PREVIOUS row (The 2-row lag)
            if *current_row_idx > 0 && self.will_post_process() {
                let row_to_post_process =
                    &mut prev_data[*final_out_pos - out_chunk_size..*final_out_pos];

                // Process the row that is trailing one step behind
                self.post_process_row(row_to_post_process, row_size - 1, self.png_info.width)?;
            }

            // 5. Advance the trackers
            *processed_bytes += row_size;
            *final_out_pos += out_chunk_size;
            *current_row_idx += 1;
        }

        Ok(())
    }
}

impl<T> PngDecoder<T> {
    /// Apply PNG unfiltering
    ///
    /// # Arguments
    /// - `filter_byte`: The filter method, bound between 0 and 4
    /// - `prev_row`: The previous unfiltered row (n-1)
    /// - `raw`: The current row (n)
    /// - `current`: Where we will write our current output
    /// - `is_first_row`: Whether the filter is the first row so that we use filters that do not require
    ///   the previous row
    /// - `components`: Number of components in the image
    #[inline]
    fn apply_filter(
        &self, filter_byte: u8, prev_row: &[u8], raw: &[u8], current: &mut [u8],
        is_first_row: bool, components: usize,
    ) -> Result<(), PngDecodeErrors> {
        let use_sse4 = self.options.use_sse41();
        let use_sse2 = self.options.use_sse2();
        let width_stride = raw.len();

        let mut filter = FilterMethod::from_int(filter_byte)
            .ok_or_else(|| PngDecodeErrors::Generic(format!("Unknown filter {filter_byte}")))
            .unwrap();

        if is_first_row {
            // Match the filters to special filters for the first row.
            // These special filters do not need the previous scanline and treat it as zero.
            match filter {
                FilterMethod::Paeth => filter = FilterMethod::PaethFirst,
                FilterMethod::Up => filter = FilterMethod::None,
                FilterMethod::Average => filter = FilterMethod::AvgFirst,
                _ => {}
            }
        }

        match filter {
            FilterMethod::None => current[..width_stride].copy_from_slice(raw),
            FilterMethod::Average => handle_avg(prev_row, raw, current, components, use_sse4),
            FilterMethod::Sub => handle_sub(raw, current, components, use_sse2),
            FilterMethod::Up => handle_up(prev_row, raw, current),
            FilterMethod::Paeth => handle_paeth(prev_row, raw, current, components, use_sse4),
            FilterMethod::PaethFirst => handle_paeth_first(raw, current, components),
            FilterMethod::AvgFirst => handle_avg_first(raw, current, components),
            FilterMethod::Unknown => unreachable!(),
        }

        Ok(())
    }
}

impl<T> PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    /// Helper to check if we need to run any post-processing at all
    #[inline]
    fn will_post_process(&self) -> bool {
        let add_alpha =
            self.options.png_get_add_alpha_channel() && !self.png_info.color.has_alpha();
        self.seen_trns | self.seen_ptle | (self.png_info.depth < 8) | add_alpha
    }

    /// Post-processes a single row in-place (using `previous_stride` as a temporary buffer)
    #[inline]
    pub(crate) fn post_process_row(
        &mut self, to_filter_row: &mut [u8], width_stride: usize, row_width: usize,
    ) -> Result<(), PngDecodeErrors> {
        let info = &self.png_info;
        let width = info.width;
        let n_components = usize::from(info.color.num_components());
        let add_alpha_channel = self.options.png_get_add_alpha_channel() && !info.color.has_alpha();

        let extra_transform = self.seen_ptle | self.seen_trns | add_alpha_channel;

        if info.depth < 8 {
            if extra_transform {
                // Input data is in `to_filter_row`, we write output to `previous_stride`
                // since other parts will read from `previous_stride`.
                expand_bits_to_byte(
                    row_width,
                    usize::from(info.depth),
                    n_components,
                    self.seen_ptle,
                    to_filter_row,
                    &mut self.previous_stride,
                );
            } else {
                // No extra transform, just depth upscaling.
                // Copy the row to a temporary space
                self.previous_stride[..width_stride]
                    .copy_from_slice(&to_filter_row[..width_stride]);

                expand_bits_to_byte(
                    row_width,
                    usize::from(info.depth),
                    n_components,
                    self.seen_ptle,
                    &self.previous_stride,
                    to_filter_row,
                );
            }
        } else {
            // Copy the row to a temporary space for subsequent transforms
            self.previous_stride[..width_stride].copy_from_slice(&to_filter_row[..width_stride]);
        }

        if self.seen_trns && info.color != PngColor::Palette {
            if info.depth <= 8 {
                expand_trns::<false>(
                    &self.previous_stride,
                    to_filter_row,
                    info.color,
                    self.trns_bytes,
                    info.depth,
                );
            } else if info.depth == 16 {
                expand_trns::<true>(
                    &self.previous_stride,
                    to_filter_row,
                    info.color,
                    self.trns_bytes,
                    info.depth,
                );
            }
        }

        if self.seen_ptle && info.color == PngColor::Palette {
            if self.palette.is_empty() {
                return Err(PngDecodeErrors::EmptyPalette);
            }
            let plte_entry: &[PLTEEntry; 256] = self.palette[..256].try_into().unwrap();

            if self.seen_trns | add_alpha_channel {
                expand_palette(&self.previous_stride, to_filter_row, plte_entry, 4);
            } else {
                expand_palette(&self.previous_stride, to_filter_row, plte_entry, 3);
            }
        } else if add_alpha_channel {
            add_alpha(
                &self.previous_stride,
                to_filter_row,
                info.color,
                self.depth().unwrap(),
            );
        }

        Ok(())
    }

    /// Safely calculates the boundaries of the final row and processes it.
    pub(crate) fn post_process_final_row(
        &mut self, final_out: &mut [u8], current_row_idx: usize, out_chunk_size: usize,
        width_stride: usize,
    ) -> Result<(), PngDecodeErrors> {
        if !self.will_post_process() || current_row_idx == 0 {
            return Ok(());
        }

        // current_row_idx represents how many rows we have successfully *un-filtered*.
        // Therefore, the very last row in the buffer is at `current_row_idx - 1`.
        let final_row_idx = current_row_idx - 1;

        let start = final_row_idx * out_chunk_size;
        let end = start + out_chunk_size;

        let to_filter_row = &mut final_out[start..end];

        self.post_process_row(to_filter_row, width_stride, out_chunk_size)
    }
}

#[cfg(test)]
mod tests {
    use zune_core::bytestream::ZCursor;

    fn decode_zune_streaming(data: &[u8]) -> Vec<u8> {
        let mut decoder = crate::PngDecoder::new(ZCursor::new(data));

        decoder.decode_stream_raw().unwrap()
    }
    fn decode_zune_png(data: &[u8]) -> Vec<u8> {
        let mut decoder = crate::PngDecoder::new(ZCursor::new(data));
        decoder.decode_raw().unwrap()
    }
    // #[test]
    // fn test_simple_decode() {
    //     let path =
    //         "/Users/etemesi/rust/zune-image/crates/zune-png/tests/benchmarks/speed_bench_interlaced.png";
    //     let data = std::fs::read(path).unwrap();
    //     let last = decode_zune_png(&data[..]);
    //
    //     let first = decode_zune_streaming(&data[..]);
    //     assert_eq!(first, last);
    // }
}
