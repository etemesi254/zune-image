use crate::decoder::PLTEEntry;
use crate::enums::{FilterMethod, PngChunkType, PngColor};
use crate::error::PngDecodeErrors;
use crate::filters::de_filter::{
    handle_avg, handle_avg_first, handle_paeth, handle_paeth_first, handle_sub, handle_up,
};
use crate::utils::{
    add_alpha, expand_bits_to_byte, expand_palette, expand_palette_sub_byte, expand_trns,
};
use crate::{InterlaceMethod, PngDecoder};
use zune_core::bytestream::ZByteReaderTrait;
use zune_inflate::DecodeStatus;

const XORIG: [usize; 7] = [0, 4, 0, 2, 0, 1, 0];
const YORIG: [usize; 7] = [0, 0, 4, 0, 2, 0, 1];
const XSPC: [usize; 7] = [8, 8, 4, 4, 2, 2, 1];
const YSPC: [usize; 7] = [8, 8, 8, 4, 4, 2, 2];

// this is the single read size per decode of deflate
// A bit larger than typical ZLIB idat chunks (which are 8KB)
// but this allows us to reduce the copy_within shifts in the decoder
const BUF_READ: usize = 32_768;
// Maximum deflate history that can be used by an image
const MAX_DEFLATE_HISTORY: usize = 32_768;

struct InterlaceState {
    // Current interlace pass, bounded from 0 to 7
    current_pass: usize,
    // Maximum pass width
    pass_w: usize,
    // Maximum pass height
    pass_h: usize,
    current_row_idx: usize,
    row_size: usize,
    width_stride: usize,
    out_chunk_size: usize,
    // The Ping-Pong buffer (size: width_stride * 2)
    raw_buffers: Vec<u8>,
    // Holds the fully processed row before scattering
    post_processed_row: Vec<u8>,
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
        &self, pass: usize, pass_y: usize, pass_w: usize, post_processed_row: &[u8],
        final_image_out: &mut [u8], num_components: usize,
    ) {
        let bpp = num_components * if self.png_info.depth == 16 { 2 } else { 1 };

        // We only need to find where the row starts once.
        let out_y = pass_y * YSPC[pass] + YORIG[pass];
        let row_start_idx = out_y * self.png_info.width * bpp;

        // Slicing here restricts the memory window, helping the compiler elide inner bounds checks.
        let out_row_slice = &mut final_image_out[row_start_idx..];
        let x_orig_bytes = XORIG[pass] * bpp;

        // Pass 7 (Index 6) has XSPC = 1. Pixels are strictly contiguous
        // We bypass the loop entirely.
        if pass == 6 {
            let total_bytes = pass_w * bpp;
            out_row_slice[x_orig_bytes..x_orig_bytes + total_bytes]
                .copy_from_slice(&post_processed_row[..total_bytes]);
            return;
        }

        let x_spc_bytes = XSPC[pass] * bpp;
        let src_pixels = &post_processed_row[..pass_w * bpp];

        let out_space = &mut out_row_slice[x_orig_bytes..];

        macro_rules! scatter {
            ($b_size:expr) => {
                if {$b_size} <= x_spc_bytes{
                    for (src_pixel, out_chunk) in src_pixels
                        .chunks_exact($b_size)
                        .zip(out_space.chunks_mut(x_spc_bytes))
                    {
                        out_chunk[..$b_size].copy_from_slice(src_pixel);
                    }
                }
            };
        }

        match bpp {
            1 => scatter!(1),
            2 => scatter!(2),
            3 => scatter!(3),
            4 => scatter!(4),
            6 => scatter!(6),
            8 => scatter!(8),
            _ => unreachable!(),
        }
    }

    fn extract_rows_interlaced(
        &mut self, deflate_buf: &[u8], processed_bytes: &mut usize, decode_dest: usize,
        final_out: &mut [u8], state: &mut InterlaceState,
    ) -> Result<(), PngDecodeErrors> {
        let num_components = self.colorspace().unwrap().num_components();
        let will_post_process = self.will_post_process();

        while (decode_dest - *processed_bytes) >= state.row_size && !state.is_complete {
            let in_stride = &deflate_buf[*processed_bytes..*processed_bytes + state.row_size];
            let filter_byte = in_stride[0];
            let raw = &in_stride[1..];

            // 1. Ping-pong buffer split (Required for interlaced so we have a contiguous prev_row)
            let midpoint = state.raw_buffers.len() / 2;
            let (half_a, half_b) = state.raw_buffers.split_at_mut(midpoint);

            let curr_idx = state.current_row_idx % 2;
            let (prev_row_full, curr_row_full) =
                if curr_idx == 0 { (&*half_b, half_a) } else { (&*half_a, half_b) };

            let prev_row = &prev_row_full[..state.width_stride];
            let curr_row = &mut curr_row_full[..state.width_stride];

            // 2. Un-filter
            let is_first_row = state.current_row_idx == 0;
            self.apply_filter(
                filter_byte,
                prev_row,
                raw,
                curr_row,
                is_first_row,
                state.filter_components,
            )?;

            // 3. Post-process & Scatter
            if will_post_process {
                // SLOW PATH: Needs color/depth conversion before scattering
                let dest_slice = &mut state.post_processed_row[..state.out_chunk_size];
                self.post_process_row_direct(curr_row, dest_slice, state.pass_w)?;

                self.scatter_interlaced_row(
                    state.current_pass,
                    state.current_row_idx,
                    state.pass_w,
                    dest_slice,
                    final_out,
                    num_components,
                );
            } else {
                // FAST PATH: No post-processing needed.
                // Skip `post_processed_row` entirely and scatter directly from `curr_row`.
                let valid_curr_row = &curr_row[..state.out_chunk_size.min(curr_row.len())];

                self.scatter_interlaced_row(
                    state.current_pass,
                    state.current_row_idx,
                    state.pass_w,
                    valid_curr_row,
                    final_out,
                    num_components,
                );
            }

            *processed_bytes += state.row_size;
            state.current_row_idx += 1;

            // 5. Check for pass completion
            if state.current_row_idx == state.pass_h {
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
                        break;
                    }
                    state.current_pass += 1;
                }

                state.row_size = self.calculate_pass_row_size(state.pass_w);
                state.width_stride = state.row_size - 1;

                let bytes_per_pixel = if self.png_info.depth == 16 { 2 } else { 1 };
                state.out_chunk_size = state.pass_w * num_components * bytes_per_pixel;
                state.current_row_idx = 0;
            }
        }
        Ok(())
    }

    fn decode_stream_interlaced(&mut self, final_out: &mut [u8]) -> Result<(), PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner()?;
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
        let initial_out_chunk =
            initial_pass_w * self.colorspace().unwrap().num_components() * bytes_per_pixel;

        let mut state = InterlaceState {
            current_pass: start_pass,
            pass_w: initial_pass_w,
            pass_h: initial_pass_h,
            current_row_idx: 0,
            row_size: initial_row_size,
            width_stride: initial_row_size - 1,
            out_chunk_size: initial_out_chunk,
            // Allocate the ping pong buffer AND the post-processed scatter buffer
            raw_buffers: vec![0; std::cmp::max(max_row_size, max_out_chunk) * 2],
            post_processed_row: vec![0; max_out_chunk],
            filter_components,
            is_complete: false,
        };

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
            // we updated the buffer,so reset position to zero
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
                        let keep_amount = std::cmp::max(MAX_DEFLATE_HISTORY, unread_bytes);

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
    /// Decode a single stream
    pub(crate) fn decode_stream_raw(&mut self) -> Result<Vec<u8>, PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner()?;
        }
        // make buffer
        let mut final_out = vec![0u8; self.output_buffer_size().unwrap()];
        // decode into buffer
        self.decode_stream_into(&mut final_out)?;
        Ok(final_out)
    }
    pub(crate) fn decode_stream_into(&mut self, out: &mut [u8]) -> Result<(), PngDecodeErrors> {
        if self.png_info.interlace_method == InterlaceMethod::Standard {
            self.decode_stream(out)?;
        } else {
            self.decode_stream_interlaced(out)?;
        }
        Ok(())
    }
}
impl<T> PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    fn decode_stream(&mut self, final_out: &mut [u8]) -> Result<(), PngDecodeErrors> {
        if !self.seen_headers {
            self.decode_headers_inner()?;
        }

        let row_size = self.calculate_row_size();
        let width_stride = row_size - 1;
        let bytes_per_channel = if self.png_info.depth == 16 { 2 } else { 1 };
        let out_chunk_size =
            self.png_info.width * self.colorspace().unwrap().num_components() * bytes_per_channel;

        let will_post_process = self.will_post_process();

        // 2. Setup Single-Allocation Ping-Pong Buffers
        // We allocate exactly enough space for TWO raw rows side-by-side.
        // This ensures the current and previous rows are right next to each other in cache.
        // But we only allocate it if we will need it, and it is only needed on images we will post process
        // e.g by palettes etc
        let mut raw_buffers = vec![0u8; width_stride * 2*usize::from(will_post_process)];

        // 3. Setup Deflate Buffers
        let buf_size = std::cmp::max(65536, MAX_DEFLATE_HISTORY + row_size + 4096);
        let mut major_buf = vec![0u8; buf_size + BUF_READ];
        let (byte_buf, deflate_buf) = major_buf.split_at_mut(BUF_READ);

        let mut decoder = zune_inflate::StreamingDecoder::new();
        let mut processed_bytes = 0;
        let mut current_row_idx = 0;
        let mut final_out_pos = 0;
        let mut skipped_zlib_header = false;
        let mut is_final_chunk = false;

        // 4. The Streaming Loop
        loop {
            if self.current_idat_bytes_left == 0 {
                self.stream.skip(4)?; // Skip CRC
                let header = self.read_chunk_header()?;

                if header.chunk_type != PngChunkType::IDAT {
                    is_final_chunk = true;
                    self.current_idat_bytes_left = 0;
                } else {
                    self.current_idat_bytes_left = header.length;
                }
            }

            // Read bytes from the IDAT stream
            let read_len = std::cmp::min(byte_buf.len(), self.current_idat_bytes_left);
            let chunk_size = self.stream.read_bytes(&mut byte_buf[..read_len])?;
            self.current_idat_bytes_left = self.current_idat_bytes_left.saturating_sub(chunk_size);
            decoder.reset_position();

            let mut chunk_pos = 0;
            // If first chunk, we skip the ZLIB bytes
            if !skipped_zlib_header {
                chunk_pos += 2;
                skipped_zlib_header = true;
            }

            loop {
                let input_slice: &[u8] = if self.current_idat_bytes_left == 0 && is_final_chunk {
                    &[]
                } else {
                    &byte_buf[chunk_pos..chunk_size]
                };

                match decoder.decode_chunk(input_slice, is_final_chunk, deflate_buf) {
                    DecodeStatus::NeedsMoreInput => break,

                    DecodeStatus::NeedsMoreOutput { .. } => {
                        self.extract_rows(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            row_size,
                            &mut current_row_idx,
                            final_out,
                            &mut final_out_pos,
                            out_chunk_size,
                            &mut raw_buffers,
                        )?;

                        let unread_bytes = decoder.current_dest_offset() - processed_bytes;
                        let keep_amount = std::cmp::max(MAX_DEFLATE_HISTORY, unread_bytes);
                        let slide_amount =
                            decoder.current_dest_offset().saturating_sub(keep_amount);

                        if slide_amount > 0 {
                            deflate_buf.copy_within(slide_amount..decoder.current_dest_offset(), 0);
                            decoder.slide_window(slide_amount);
                            processed_bytes -= slide_amount;
                        }
                    }

                    DecodeStatus::Finished => {
                        self.extract_rows(
                            deflate_buf,
                            &mut processed_bytes,
                            decoder.current_dest_offset(),
                            row_size,
                            &mut current_row_idx,
                            final_out,
                            &mut final_out_pos,
                            out_chunk_size,
                            &mut raw_buffers,
                        )?;
                        return Ok(());
                    }

                    DecodeStatus::Error(e) => return Err(PngDecodeErrors::ZlibDecodeErrors(e)),
                    _ => unreachable!(),
                }
            }
        }
    }
}
impl<T> PngDecoder<T>
where
    T: ZByteReaderTrait,
{
    #[allow(clippy::too_many_arguments)]
    fn extract_rows(
        &mut self,
        deflate_buf: &[u8],
        processed_bytes: &mut usize,
        decode_dest: usize,
        row_size: usize,
        current_row_idx: &mut usize,
        final_out: &mut [u8],
        final_out_pos: &mut usize,
        out_chunk_size: usize,
        raw_buffers: &mut [u8],
    ) -> Result<(), PngDecodeErrors> {
        let width_stride = row_size - 1;
        let filter_bpp = usize::from(self.png_info.color.num_components())
            * if self.png_info.depth == 16 { 2 } else { 1 };

        let will_post_process = self.will_post_process();

        while (decode_dest - *processed_bytes) >= row_size {
            // Grab the chunk of deflate data for this exact row
            let in_stride = &deflate_buf[*processed_bytes..*processed_bytes + row_size];
            let filter_byte = in_stride[0];
            let raw = &in_stride[1..];
            let is_first_row = *current_row_idx == 0;

            if will_post_process {
                // SLOW PATH: We must use temporary buffers because the previous row
                // in `final_out` has already been transformed, and the PNG filter
                // requires raw, unmodified previous row bytes.
                let (half_a, half_b) = raw_buffers.split_at_mut(width_stride);
                let curr_idx = *current_row_idx % 2;
                let (prev_row, curr_row) = if curr_idx == 0 {
                    (&*half_b, half_a)
                } else {
                    (&*half_a, half_b)
                };

                self.apply_filter(
                    filter_byte,
                    prev_row,
                    raw,
                    curr_row,
                    is_first_row,
                    filter_bpp,
                )?;

                let dest_slice = &mut final_out[*final_out_pos..*final_out_pos + out_chunk_size];
                self.post_process_row_direct(curr_row, dest_slice, self.png_info.width)?;

            } else {
                // FAST PATH (Zero-Copy): No post-processing needed.
                // We write directly into `final_out`.

                // split_at_mut safely gives us read access to what we've already written,
                // and write access to the remaining space.
                let (finished, remaining) = final_out.split_at_mut(*final_out_pos);
                let dest_slice = &mut remaining[..out_chunk_size];

                let prev_row = if is_first_row {
                    &[]
                } else {
                    // The previous row is simply the last `out_chunk_size` bytes we just wrote.
                    &finished[finished.len() - out_chunk_size..]
                };

                self.apply_filter(
                    filter_byte,
                    prev_row,
                    raw,
                    dest_slice,
                    is_first_row,
                    filter_bpp,
                )?;
            }

            // Advance all trackers
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
            .ok_or_else(|| PngDecodeErrors::Generic(format!("Unknown filter {filter_byte}")))?;

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
    T: zune_core::bytestream::ZByteReaderTrait,
{
    /// Directly transforms a raw unfiltered row into the final formatted output.
    /// Eliminates all intermediate vectors and 2-pass conversions.
    #[inline]
    pub(crate) fn post_process_row_direct(
        &self, raw_input: &[u8], final_output: &mut [u8], row_width: usize,
    ) -> Result<(), PngDecodeErrors> {
        let info = &self.png_info;
        let n_components = usize::from(info.color.num_components());
        let add_alpha_channel = self.options.png_get_add_alpha_channel() && !info.color.has_alpha();

        let has_trns = self.seen_trns && info.color != PngColor::Palette;
        let is_palette = self.seen_ptle && info.color == PngColor::Palette;

        // --- 1. PALETTE PATH (Fused 1-Pass) ---
        if is_palette {
            if self.palette.is_empty() {
                return Err(PngDecodeErrors::EmptyPalette);
            }
            let plte_entry: &[PLTEEntry; 256] = self.palette[..256].try_into().unwrap();

            // If there's a tRNS chunk for the palette, or we are forcing alpha, it becomes RGBA
            let components = if self.seen_trns || add_alpha_channel { 4 } else { 3 };

            if info.depth < 8 {
                expand_palette_sub_byte(
                    raw_input,
                    final_output,
                    plte_entry,
                    components,
                    info.depth,
                    row_width,
                );
            } else {
                expand_palette(raw_input, final_output, plte_entry, components);
            }
            return Ok(());
        }

        // --- 2. GRAYSCALE SUB-BYTE WITH tRNS OR ADDED ALPHA (Fused 1-Pass) ---
        // If an image is < 8-bit depth, not paletted, but requires an alpha channel,
        // we must expand the bits AND inject the alpha bytes simultaneously.
        if info.depth < 8 && (has_trns || add_alpha_channel) {
            // Pre-calculate the scaled tRNS match value
            let trns_val_scaled = if has_trns {
                let depth_mask = (1_u16 << info.depth) - 1;
                let scale = match info.depth {
                    1 => 0xFF,
                    2 => 0x55,
                    4 => 0x11,
                    _ => 0,
                };
                ((self.trns_bytes[0] & 0xFF & depth_mask) as u8) * scale
            } else {
                255 // Impossible value for u8, meaning it will never match
            };

            let scale = match info.depth {
                1 => 0xFF,
                2 => 0x55,
                4 => 0x11,
                _ => 0,
            };

            let mut out_idx = 0;
            let mut px_processed = 0;
            let pixels_per_byte = (8 / info.depth) as usize;

            for &in_byte in raw_input {
                if px_processed >= row_width {
                    break;
                }

                for i in 0..pixels_per_byte {
                    if px_processed >= row_width {
                        break;
                    }

                    let shift = 8 - info.depth - (i as u8 * info.depth);
                    let mask = (1 << info.depth) - 1;

                    let raw_val = (in_byte >> shift) & mask;
                    let expanded_luma = raw_val * scale;

                    final_output[out_idx] = expanded_luma;

                    if has_trns && expanded_luma == trns_val_scaled {
                        final_output[out_idx + 1] = 0; // Fully transparent
                    } else {
                        final_output[out_idx + 1] = 255; // Fully opaque
                    }

                    out_idx += 2;
                    px_processed += 1;
                }
            }
            return Ok(());
        }

        // --- 3. SUB-BYTE (NO ALPHA/tRNS TRANSFORMS) ---
        if info.depth < 8 {
            expand_bits_to_byte(
                row_width,
                usize::from(info.depth),
                n_components,
                self.seen_ptle,
                raw_input,
                final_output,
            );
            return Ok(());
        }

        // --- 4. 8-BIT / 16-BIT tRNS ---
        if has_trns {
            if info.depth <= 8 {
                expand_trns::<false>(
                    raw_input,
                    final_output,
                    info.color,
                    self.trns_bytes,
                    info.depth,
                );
            } else if info.depth == 16 {
                expand_trns::<true>(
                    raw_input,
                    final_output,
                    info.color,
                    self.trns_bytes,
                    info.depth,
                );
            }
            return Ok(());
        }

        // --- 5. 8-BIT / 16-BIT ADD ALPHA ---
        if add_alpha_channel {
            add_alpha(raw_input, final_output, info.color, self.depth().unwrap());
            return Ok(());
        }

        // --- 6. FAST PATH NO-OP (Direct Copy) ---
        // If no processing is required, safely copy over the exact length.
        let copy_len = final_output.len().min(raw_input.len());
        final_output[..copy_len].copy_from_slice(&raw_input[..copy_len]);

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
}

#[cfg(test)]
mod tests {
    use zune_core::bytestream::ZCursor;


    #[test]
    fn decode_normal() {
        let path =
            "/Users/etemesi/rust/zune-image/test-images/png/benchmarks/speed_bench.png";
        let data = std::fs::read(path).unwrap();
        let mut decoder = crate::PngDecoder::new(ZCursor::new(data));
        decoder.decode().unwrap();
    }
}
