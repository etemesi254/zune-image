use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::debug_more;
use crate::hevc_decoder::cabac::{CabacDecoder, NUM_CABAC_CONTEXTS};
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SPLIT_CU_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::deblocker::{DeblockParams, deblock_frame};
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{Pps, SliceHeader, SliceType, Sps};
use crate::hevc_decoder::neighbor_tracker::NeighborTracker;
use crate::hevc_decoder::quadtree::sao::SaoInfo;
use crate::hevc_decoder::shared::SharedBuf;
use crate::hevc_decoder::nal_unit_parsers::decode_slice_header;
use crate::hevc_decoder::quadtree::coding_unit::{read_coding_tree_unit, read_coding_unit};
use crate::hevc_decoder::raw_frame::RawFrame;
use crate::hevc_decoder::utils::extract_rbsp_with_epb;
use crate::hevc_decoder::{DEBUG_MORE, HevcDecoder};

mod transform_unit;

mod coding_unit;
mod intra;
mod intra_prediction;
mod part_mode;
mod quant;
mod residual_block;
pub(crate) mod sao;
pub(crate) mod sig_ctx_generator;

#[derive(PartialEq)]
pub enum CtuStatus {
    Continue,
    EndOfSliceSegment,
    EndOfSubstream
}

pub fn decode_slice(
    nal: &NalUnit, hevc_decoder: &mut HevcDecoder, raw_frame: &Arc<RawFrame>
) -> Result<(), NalError> {
    let mut epb_positions = Vec::new();
    let clean_rbsp = extract_rbsp_with_epb(nal.payload, Some(&mut epb_positions));
    let sps_storage = &hevc_decoder.sps_storage;
    let pps_storage = &hevc_decoder.pps_storage;

    let slice_header = decode_slice_header(nal, pps_storage, sps_storage, &clean_rbsp)?;

    let pps = pps_storage
        .get(slice_header.slice_pic_parameter_set_id as usize)
        .ok_or_else(|| {
            NalError::Generic(format!(
                "no pps found for slice header {}",
                slice_header.slice_pic_parameter_set_id
            ))
        })?
        .as_ref()
        .unwrap();
    let sps = sps_storage[pps.sps_id as usize].as_ref().unwrap();

    // 1. Initial QP
    let slice_qp = (26 + pps.init_qp_minus26 + slice_header.slice_qp_delta) as i8;

    // 2. CABAC Initialization (Section 9.3.2.2)
    let init_type = match slice_header.slice_type {
        SliceType::I => 0,
        SliceType::P => 1,
        SliceType::B => 2
    };
    // Dependent slices inherit the previous CABAC state, independent ones reset.
    let payload_start = &clean_rbsp[slice_header.cabac_start_position..];
    let mut cabac = CabacDecoder::new(payload_start, i32::from(slice_qp), init_type);

    if slice_header.dependent_slice_segment_flag {
        if let Some(saved_contexts) = &hevc_decoder.dependent_slice_contexts {
            debug_more!("Loading CABAC contexts from previous slice segment.");
            cabac.contexts.clone_from(saved_contexts);
        } else {
            return Err(NalError::GenericStr(
                "Stream Error: Dependent slice flag is set, but no previous contexts exist!"
            ));
        }
    } else {
        hevc_decoder.dependent_slice_contexts = None;
    }

    // 3. CTU Range (Raster Scan Address)

    let width_in_ctus = sps.pic_width_in_ctbs_y as usize;

    let start_ctu_addr = slice_header.slice_segment_address as usize;
    let total_ctus = width_in_ctus * (sps.pic_height_in_ctbs_y as usize);

    // 4. State shared by all CTU rows of this slice
    let height_in_ctus = sps.pic_height_in_ctbs_y as usize;
    let sao_buffer = SharedBuf::new(width_in_ctus * height_in_ctus, SaoInfo::default());
    let wpp_contexts: Vec<Mutex<Option<[u8; NUM_CABAC_CONTEXTS]>>> =
        (0..height_in_ctus).map(|_| Mutex::new(None)).collect();

    let neighbor_tracker = hevc_decoder.neighbor_tracker.as_mut().unwrap();

    // 5. Decode the CTUs: CTU rows in parallel (WPP) when possible, else in order.
    let substreams = if slice_header.dependent_slice_segment_flag
        || pps.dependent_slice_segments_enabled_flag
        || pps.tiles_enabled_flag
        || !pps.entropy_coding_sync_enabled_flag
        || !start_ctu_addr.is_multiple_of(width_in_ctus)
    {
        None
    } else {
        wpp_substream_starts(
            slice_header.cabac_start_position,
            &slice_header.entry_point_offsets,
            &epb_positions,
            clean_rbsp.len(),
        )
    };
    let threads = substreams
        .as_ref()
        .map_or(1, |s| hevc_decoder.max_threads.min(s.len()));

    if let (Some(starts), true) = (&substreams, threads > 1) {
        let rows = WppRows {
            sps,
            pps,
            slice_header: &slice_header,
            data: &clean_rbsp,
            starts,
            first_row: start_ctu_addr / width_in_ctus,
            slice_qp,
            init_type,
            raw_frame,
            sao_buffer: &sao_buffer,
            wpp_contexts: &wpp_contexts,
        };
        rows.decode(neighbor_tracker, threads)?;
    } else {
        let mut ctx = DecodeSliceContext::new(
            sps,
            pps,
            &slice_header,
            cabac,
            neighbor_tracker,
            slice_qp,
            &raw_frame.clone(),
            // SAFETY: this is the only handle in use while decoding.
            unsafe { sao_buffer.shared_view() },
            &wpp_contexts,
        );

        // The CTU Loop (starts at slice address)
        for ctu_addr in start_ctu_addr..total_ctus {
            let ctu_x = ctu_addr % width_in_ctus;
            let ctu_y = ctu_addr / width_in_ctus;

            debug_more!("--- Decoding CTU {} [{}, {}] ---", ctu_addr, ctu_x, ctu_y);

            // --- WPP CABAC Context Inheritance (The "Load") ---
            if pps.entropy_coding_sync_enabled_flag
                && ctu_x == 0
                && ctu_y > 0
                && ctu_addr > start_ctu_addr
            {
                load_wpp_contexts(&mut ctx, ctu_y, width_in_ctus, slice_qp, init_type)?;
            }
            read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;

            match finish_ctu(&mut ctx, ctu_x, ctu_y)? {
                CtuStatus::EndOfSliceSegment => {
                    debug_more!("Slice Segment Finished at CTU {}", ctu_addr);
                    // If it's a dependent slice, save the state for the next one
                    if pps.dependent_slice_segments_enabled_flag {
                        hevc_decoder.dependent_slice_contexts = Some(ctx.cabac.contexts);
                    }
                    break; // Exit loop, slice is done
                }
                CtuStatus::EndOfSubstream => {
                    // Handle Tiles / WPP alignment
                    ctx.cabac.init_cabac();
                }
                CtuStatus::Continue => {}
            }
        }
    }
    let neighbor_tracker = hevc_decoder.neighbor_tracker.as_ref().unwrap();

    // apply deblocking
    if !slice_header.slice_deblocking_filter_disabled_flag {
        // perf wise
        // with deblocking     139.33 ms
        // without deblocking  129.64 ms
        //
        // So deblocking does have a speed hit
        // but its negligible imo
        let rf_clone = raw_frame.clone();
        deblock_frame(
            &rf_clone,
            neighbor_tracker,
            DeblockParams {
                beta_offset_div2: slice_header.slice_beta_offset_div2,
                tc_offset_div2:   slice_header.slice_tc_offset_div2,
                cb_qp_offset:     pps.cb_qp_offset as i8,
                cr_qp_offset:     pps.cr_qp_offset as i8
            }
        );
    }
    if slice_header.slice_sao_luma_flag || slice_header.slice_sao_chroma_flag {
        sao::apply_sao_frame(
            raw_frame,
            hevc_decoder.width,
            hevc_decoder.height,
            1 << sps.log2_ctb_size_y,
            &sao_buffer.to_vec()
        );
    }
    Ok(())
}
/// Start a CTU row from the CABAC contexts saved after CTU 1 of the row above
/// (WPP, spec 9.3.1), or from the initial contexts for 1-CTU-wide pictures.
fn load_wpp_contexts(
    ctx: &mut DecodeSliceContext, ctu_y: usize, width_in_ctus: usize, slice_qp: i8,
    init_type: usize
) -> Result<(), NalError> {
    if width_in_ctus > 1 {
        let saved = ctx.ctb_context[ctu_y - 1].lock().unwrap().take();
        match saved {
            Some(contexts) => {
                debug_more!("WPP: Inheriting CABAC contexts from row {}", ctu_y - 1);
                ctx.cabac.contexts = contexts;
            }
            None => {
                return Err(NalError::Generic(format!(
                    "WPP Error: Expected saved CABAC context for row {}, but found None!",
                    ctu_y - 1
                )));
            }
        }
    } else {
        // Picture is only 1 CTU wide: re-initialise from the slice QP.
        ctx.cabac.init_contexts(i32::from(slice_qp), init_type);
    }
    Ok(())
}

/// Start offsets (into the emulation-prevention-free slice data) of every WPP
/// substream, or `None` if the entry points are missing or inconsistent.
///
/// `entry_point_offset`s count bytes of the NAL unit *including* emulation
/// prevention bytes (spec 7.4.7.1), while decoding works on the cleaned
/// buffer, so offsets are mapped through the removed byte positions.
fn wpp_substream_starts(
    cabac_start: usize, entry_offsets: &[u32], epb_positions: &[usize], clean_len: usize
) -> Option<Vec<usize>> {
    if entry_offsets.is_empty() {
        return None;
    }
    // clean index -> raw index: every EPB whose clean position is <= c lies before it
    let clean_to_raw = |c: usize| {
        c + epb_positions
            .iter()
            .enumerate()
            .take_while(|&(i, &r)| r - i <= c)
            .count()
    };
    // raw index -> clean index: drop EPBs that come before it
    let raw_to_clean = |r: usize| r - epb_positions.iter().take_while(|&&p| p < r).count();

    let mut starts = Vec::with_capacity(entry_offsets.len() + 1);
    starts.push(cabac_start);
    let mut raw = clean_to_raw(cabac_start);
    for &off in entry_offsets {
        raw = raw.checked_add(off as usize)?;
        let clean = raw_to_clean(raw);
        if clean <= *starts.last().unwrap() || clean >= clean_len {
            return None;
        }
        starts.push(clean);
    }
    Some(starts)
}

/// Everything needed to decode the CTU rows of one slice with WPP threads.
struct WppRows<'a> {
    sps:          &'a Sps,
    pps:          &'a Pps,
    slice_header: &'a SliceHeader,
    /// cleaned slice NAL payload
    data:         &'a [u8],
    /// start of each row's substream in `data`
    starts:       &'a [usize],
    /// picture CTU row of the first substream
    first_row:    usize,
    slice_qp:     i8,
    init_type:    usize,
    raw_frame:    &'a Arc<RawFrame>,
    sao_buffer:   &'a SharedBuf<SaoInfo>,
    wpp_contexts: &'a [Mutex<Option<[u8; NUM_CABAC_CONTEXTS]>>]
}

impl WppRows<'_> {
    /// Decode all rows on `threads` scoped threads.
    ///
    /// Rows are handed out in order, and row `r` decodes CTU `x` only after
    /// row `r - 1` has finished CTU `x + 1` (its above-right neighbour), which
    /// is also when the CABAC contexts it starts from are available.
    fn decode(&self, tracker: &mut NeighborTracker, threads: usize) -> Result<(), NalError> {
        let n_rows = self.starts.len();
        // number of CTUs finished per row (usize::MAX once the row is done)
        let progress: Vec<AtomicUsize> = (0..n_rows).map(|_| AtomicUsize::new(0)).collect();
        let next_row = AtomicUsize::new(0);
        let abort = AtomicBool::new(false);
        let first_error: Mutex<Option<NalError>> = Mutex::new(None);
        let tracker = &*tracker;

        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    // SAFETY: every row writes only its own CTU row of the
                    // tracker / SAO buffer, and reads other rows only after
                    // `progress` (Release/Acquire) says those CTUs are done.
                    let mut tracker_view = unsafe { tracker.shared_view() };
                    loop {
                        let k = next_row.fetch_add(1, Ordering::Relaxed);
                        if k >= n_rows {
                            break;
                        }
                        if !abort.load(Ordering::Relaxed)
                            && let Err(e) = self.decode_row(k, &mut tracker_view, &progress, &abort)
                        {
                            abort.store(true, Ordering::Relaxed);
                            first_error.lock().unwrap().get_or_insert(e);
                        }
                        // wake up the row below, whatever happened
                        progress[k].store(usize::MAX, Ordering::Release);
                    }
                });
            }
        });

        match first_error.into_inner().unwrap() {
            Some(e) => Err(e),
            None => Ok(())
        }
    }

    fn decode_row(
        &self, k: usize, tracker: &mut NeighborTracker, progress: &[AtomicUsize], abort: &AtomicBool
    ) -> Result<(), NalError> {
        let width = self.sps.pic_width_in_ctbs_y as usize;
        let ctu_y = self.first_row + k;

        // Wait until the row above has finished `needed` CTUs.
        let wait_above = |needed: usize| -> Result<(), NalError> {
            if k == 0 {
                return Ok(());
            }
            while progress[k - 1].load(Ordering::Acquire) < needed {
                if abort.load(Ordering::Relaxed) {
                    return Err(NalError::GenericStr("WPP: aborted"));
                }
                std::thread::yield_now();
            }
            Ok(())
        };

        let cabac = CabacDecoder::new(
            &self.data[self.starts[k]..],
            i32::from(self.slice_qp),
            self.init_type
        );
        let mut ctx = DecodeSliceContext::new(
            self.sps,
            self.pps,
            self.slice_header,
            cabac,
            tracker,
            self.slice_qp,
            self.raw_frame,
            // SAFETY: same row discipline as the tracker (see `decode`).
            unsafe { self.sao_buffer.shared_view() },
            self.wpp_contexts
        );

        if k > 0 {
            // contexts are saved after CTU 1 of the row above
            wait_above(2.min(width))?;
            load_wpp_contexts(&mut ctx, ctu_y, width, self.slice_qp, self.init_type)?;
        }

        for ctu_x in 0..width {
            wait_above((ctu_x + 2).min(width))?;

            read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;
            let status = finish_ctu(&mut ctx, ctu_x, ctu_y)?;

            progress[k].store(ctu_x + 1, Ordering::Release);

            match status {
                CtuStatus::Continue => {}
                CtuStatus::EndOfSubstream | CtuStatus::EndOfSliceSegment => return Ok(())
            }
        }
        Err(NalError::GenericStr("WPP: row ended without end_of_subset_one_bit"))
    }
}

pub fn finish_ctu(
    ctx: &mut DecodeSliceContext, ctbx: usize, ctby: usize
) -> Result<CtuStatus, NalError> {
    let pps = &ctx.pps;
    let sps = &ctx.sps;

    debug_more!("finish_ctu ({} {})", ctbx, ctby);

    // --- 1. WPP Context Storage (Section 6.3.3) ---
    // If Wavefront Parallel Processing is enabled, we save the context state
    // after the second CTU of a row to initialize the first CTU of the next row.
    if pps.entropy_coding_sync_enabled_flag
        && ctbx == 1
        && ctby + 1 < sps.pic_height_in_ctbs_y as usize
    {
        debug_more!("Saving WPP Context for row {}", ctby);
        // We clone the current context model state (the "decouple" in libde265)
        *ctx.ctb_context[ctby].lock().unwrap() = Some(ctx.cabac.contexts);
    }

    debug_more!(
        "Cabac EOC range:{} value:{}",
        ctx.cabac.range,
        ctx.cabac.value,
    );
    // --- 2. Decode Terminal Bit (end_of_slice_segment_flag) ---
    // This bit is mandatory after every CTU (Section 7.3.8.1)
    let end_of_slice_segment_flag = ctx.cabac.decode_terminate();
    debug_more!("end_of_slice_segment_flag -> {}", end_of_slice_segment_flag);

    if end_of_slice_segment_flag != 0 {
        // If dependent slices are enabled, save context for the next slice header
        if pps.dependent_slice_segments_enabled_flag {
            debug_more!("Saving context for dependent slice");
            //ctx.shdr.ctx_model_storage = Some(ctx.cabac.ctx_model.clone());
        }
        return Ok(CtuStatus::EndOfSliceSegment);
    }

    // --- 3. Sub-stream Handling (Tiles and WPP Row Ends) ---
    // We check if the NEXT CTU belongs to a different sub-stream
    let mut end_of_sub_stream = false;

    // We need the next CTU address to check for transitions
    // let curr_addr_rs = ctby * (sps.pic_width_in_ctbs_y as usize) + ctbx;

    // Check for Tile Change (Section 7.3.8.1)
    if pps.tiles_enabled_flag {
        // HEVC decodes tiles in Tile Scan order, not Raster Scan.
        // Assuming your loop handles the TS -> RS mapping:
        // let _curr_tile_id = pps.tile_id_rs[curr_addr_rs];
        // let _next_addr_rs = curr_addr_rs + 1; // Raster scan increment

        todo!()
        // // Peek at next CTB in Tile Scan order
        // if let Some(next_addr_ts) = ctx.get_next_ctb_addr_ts() {
        //     if pps.tile_id_ts[next_addr_ts] != pps.tile_id_ts[next_addr_ts - 1] {
        //         end_of_sub_stream = true;
        //     }
        // }
    }

    // Check for WPP Row Change
    if pps.entropy_coding_sync_enabled_flag && (ctbx == (sps.pic_width_in_ctbs_y as usize - 1)) {
        end_of_sub_stream = true;
    }

    if end_of_sub_stream {
        debug_more!("End of sub-stream detected. Decoding alignment bit.");

        // Section 7.3.8.1: end_of_sub_stream_one_bit
        let eoss_bit = ctx.cabac.decode_terminate();
        if eoss_bit == 0 {
            // debug assert for tracing makes it easier to step into function
            // the return looses the context
            debug_assert!(false, "ERROR: end_of_sub_stream_one_bit was 0!");
            return Err(NalError::Generic(
                "end_of_sub_stream_one_bit was 0!".to_string()
            ));
        }

        return Ok(CtuStatus::EndOfSubstream);
    }

    ctx.n_coeff.fill(0);
    Ok(CtuStatus::Continue)
}
fn read_coding_quadtree(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8, ct_depth: u8
) -> Result<(), NalError> {
    debug_more!(
        "read_coding_quadtree (x0={},y0={},cb_size:{}, depth:{})",
        x0,
        y0,
        1_u64 << log_2_cb_size,
        ct_depth,
    );
    let cb_size = 1 << log_2_cb_size;

    debug_more!(
        "before split range:{},value:{},pos:{}",
        ctx.cabac.range,
        ctx.cabac.value,
        ctx.cabac.cursor
    );

    // if x0 == 408 && y0 == 112 && (1_u64 << log_2_cb_size) == 8 && ct_depth == 2 {
    //     let c = 0;
    //     //DEBUG_MORE.store(true, std::sync::atomic::Ordering::Relaxed);
    //
    //     //let v = DEBUG_MORE.load(std::sync::atomic::Ordering::Relaxed);
    //     println!("read_coding_quadtree cb_size:{}", cb_size, );
    // }
    let sps = ctx.sps;
    let pps = ctx.pps;

    // We only send a split flag if CU is larger than minimum size and
    // completely contained within the image area.
    // If it is partly outside the image area and not at minimum size,
    // it is split. If already at minimum size, it is not split further.
    let mut split_cu_flag = false;
    //
    if x0 + cb_size <= sps.pic_width_in_luma_samples as usize
        && y0 + cb_size <= sps.pic_height_in_luma_samples as usize
        && log_2_cb_size > sps.log2_min_luma_coding_block_size
    {
        split_cu_flag = decode_split_cu_flag(ctx, x0, y0, ct_depth);
    } else if log_2_cb_size > sps.log2_min_luma_coding_block_size {
        split_cu_flag = true;
    }
    if pps.cu_qp_delta_enabled_flag && log_2_cb_size >= pps.log2_min_cu_qp_delta_size {
        ctx.is_cu_qp_delta_coded = false;
        ctx.cu_qp_delta = 0;
    }

    if split_cu_flag {
        let cb_size_min_1 = 1 << (log_2_cb_size - 1);
        let x1 = x0 + cb_size_min_1;
        let y1 = y0 + cb_size_min_1;

        read_coding_quadtree(ctx, x0, y0, log_2_cb_size - 1, ct_depth + 1)?;

        if x1 < sps.pic_width_in_luma_samples as usize {
            read_coding_quadtree(ctx, x1, y0, log_2_cb_size - 1, ct_depth + 1)?;
        }
        if y1 < sps.pic_height_in_luma_samples as usize {
            read_coding_quadtree(ctx, x0, y1, log_2_cb_size - 1, ct_depth + 1)?;
        }
        if x1 < sps.pic_width_in_luma_samples as usize
            && y1 < sps.pic_height_in_luma_samples as usize
        {
            read_coding_quadtree(ctx, x1, y1, log_2_cb_size - 1, ct_depth + 1)?;
        }
        Ok(())
    } else {
        // set depth
        if ct_depth > 0 {
            ctx.neighbor_tracker
                .update_block_depth(x0, y0, cb_size, ct_depth);
        }
        // --- THIS IS A LEAF CU ---
        //    1. Record the depth in the tracker for neighbors to use
        read_coding_unit(ctx, x0, y0, log_2_cb_size)?;

        // 2. NOW update the neighbor tracker with the finalized state
        // We use the last_qp_in_slice because that is the official QP for this area
        debug_more!(
            "Updating tracker for Leaf CU at [{},{}] with QP {}",
            x0,
            y0,
            ctx.last_qp_in_slice
        );

        // Coding block boundaries are always deblocking edges
        ctx.neighbor_tracker.mark_block_edges(x0, y0, cb_size);

        // 3. Commit ONLY the CU-level properties to the tracker!
        // The Prediction Unit modes were already set safely inside read_coding_unit.
        ctx.neighbor_tracker.update_cu_info(
            x0,
            y0,
            cb_size,
            ct_depth,
            ctx.last_qp_in_slice,
            ctx.is_skip,
            ctx.slice_header.slice_segment_address as u16
        );

        Ok(())
    }
}

fn decode_split_cu_flag(ctx: &mut DecodeSliceContext, x0: usize, y0: usize, depth: u8) -> bool {
    let current_slice_id = ctx.slice_header.slice_segment_address as u16;
    let ctx_v = ctx
        .neighbor_tracker
        .get_split_ctx(x0, y0, depth, current_slice_id);

    let index = CONTEXT_MODEL_SPLIT_CU_FLAG + ctx_v;
    debug_more!(
        "decode_split_cu_flag -> ctx={} Range={} Value={} state:{}",
        ctx_v,
        ctx.cabac.range,
        ctx.cabac.value,
        index
    );
    let bit = ctx.cabac.decode_decision(index);
    debug_more!(
        "decode_split_cu_flag Range=>{}, ctx={} bit={}",
        ctx.cabac.range,
        ctx_v,
        bit
    );
    bit == 1
}

#[cfg(test)]
mod wpp_tests {
    use super::wpp_substream_starts;
    use crate::hevc_decoder::utils::extract_rbsp_with_epb;

    /// Build a fake slice payload: `header` bytes, then substreams. Each
    /// substream's raw bytes (which may contain EPBs) are given; entry point
    /// offsets are their raw lengths. Every substream starts with a marker
    /// byte so we can check where the mapped clean offsets land.
    #[test]
    fn entry_points_map_through_emulation_prevention_bytes() {
        let header = [0x11u8, 0x00, 0x00, 0x03, 0x01, 0x22]; // EPB inside the header
        let subs: [&[u8]; 3] = [
            &[0xA1, 0x00, 0x00, 0x03, 0x00, 0x05],       // EPB in the middle
            &[0xA2, 0x07, 0x00, 0x00, 0x03],             // EPB as the last byte
            &[0xA3, 0x00, 0x00, 0x03, 0x02, 0x00, 0x00, 0x03, 0x03, 0x09]
        ];
        let mut raw = header.to_vec();
        for s in subs {
            raw.extend_from_slice(s);
        }
        let mut epb = Vec::new();
        let clean = extract_rbsp_with_epb(&raw, Some(&mut epb));
        assert_eq!(epb.len(), 5);

        // slice data starts after the (cleaned) header
        let cabac_start = header.len() - 1;
        let offsets: Vec<u32> = subs[..2].iter().map(|s| s.len() as u32).collect();
        let starts = wpp_substream_starts(cabac_start, &offsets, &epb, clean.len()).unwrap();

        assert_eq!(starts.len(), 3);
        for (i, &st) in starts.iter().enumerate() {
            assert_eq!(clean[st], 0xA1 + i as u8, "substream {i} starts at {st}");
        }
    }

    #[test]
    fn inconsistent_entry_points_fall_back() {
        assert!(wpp_substream_starts(4, &[], &[], 100).is_none());
        assert!(wpp_substream_starts(4, &[200], &[], 100).is_none());
        assert!(wpp_substream_starts(4, &[10, 0], &[], 100).is_none());
    }
}
