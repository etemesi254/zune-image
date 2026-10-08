use alloc::{string::ToString, vec::Vec};

use crate::debug_more;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SPLIT_CU_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::deblocker::{DeblockParams, deblock_frame};
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::SliceType;
use crate::hevc_decoder::quadtree::sao::SaoInfo;
use crate::hevc_decoder::band::Band;
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
#[cfg(feature = "std")]
mod wpp;
pub(crate) mod sig_ctx_generator;

#[derive(PartialEq)]
pub enum CtuStatus {
    Continue,
    EndOfSliceSegment,
    EndOfSubstream
}

pub fn decode_slice(
    nal: &NalUnit, hevc_decoder: &mut HevcDecoder, raw_frame: &mut RawFrame
) -> Result<(), NalError> {
    // positions of removed emulation prevention bytes, needed to locate the
    // WPP substreams
    let mut epb_positions = Vec::new();
    let clean_rbsp = extract_rbsp_with_epb(
        nal.payload,
        if cfg!(feature = "std") { Some(&mut epb_positions) } else { None },
    );
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

    // 4. State of this slice's CTUs
    let height_in_ctus = sps.pic_height_in_ctbs_y as usize;
    let mut sao_buffer = vec![SaoInfo::default(); width_in_ctus * height_in_ctus];

    let block_map = hevc_decoder.neighbor_tracker.as_mut().unwrap();

    // 5. Decode the CTUs: one decoder per CTU row in parallel (WPP) when
    // possible, else all CTUs in order.
    #[cfg(feature = "std")]
    let decoded_in_parallel = {
        let substreams = if slice_header.dependent_slice_segment_flag
            || pps.dependent_slice_segments_enabled_flag
            || pps.tiles_enabled_flag
            || !pps.entropy_coding_sync_enabled_flag
            || !start_ctu_addr.is_multiple_of(width_in_ctus)
        {
            None
        } else {
            wpp::wpp_substream_starts(
                slice_header.cabac_start_position,
                &slice_header.entry_point_offsets,
                &epb_positions,
                clean_rbsp.len(),
            )
        };
        // With a single thread, decoding the rows in order is cheaper than the
        // per-row hand-off.
        let threads = substreams
            .as_ref()
            .map_or(1, |s| hevc_decoder.max_threads.min(s.len()));

        if let (Some(starts), true) = (&substreams, threads > 1) {
            let rows = wpp::WppRows {
                sps,
                pps,
                slice_header: &slice_header,
                data: &clean_rbsp,
                starts,
                first_row: start_ctu_addr / width_in_ctus,
                slice_qp,
                init_type,
            };
            rows.decode(raw_frame, block_map, &mut sao_buffer, threads)?;
            true
        } else {
            false
        }
    };
    #[cfg(not(feature = "std"))]
    let decoded_in_parallel = false;

    if !decoded_in_parallel {
        // One band covering the whole picture
        let mut ctx = DecodeSliceContext::new(
            sps,
            pps,
            &slice_header,
            cabac,
            block_map.whole(),
            slice_qp,
            raw_frame.whole(),
            Band::new(&mut sao_buffer, 0, 0),
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
        deblock_frame(
            raw_frame,
            neighbor_tracker,
            DeblockParams {
                beta_offset_div2: slice_header.slice_beta_offset_div2,
                tc_offset_div2:   slice_header.slice_tc_offset_div2,
                cb_qp_offset:     pps.cb_qp_offset as i8,
                cr_qp_offset:     pps.cr_qp_offset as i8
            },
            hevc_decoder.max_threads
        );
    }
    if slice_header.slice_sao_luma_flag || slice_header.slice_sao_chroma_flag {
        sao::apply_sao_frame(
            raw_frame,
            hevc_decoder.width,
            hevc_decoder.height,
            1 << sps.log2_ctb_size_y,
            &sao_buffer,
            hevc_decoder.max_threads
        );
    }
    Ok(())
}
/// Start a CTU row from the CABAC contexts saved after CTU 1 of the row above
/// (WPP, spec 9.3.1), or from the initial contexts for 1-CTU-wide pictures.
pub(super) fn load_wpp_contexts(
    ctx: &mut DecodeSliceContext, ctu_y: usize, width_in_ctus: usize, slice_qp: i8,
    init_type: usize
) -> Result<(), NalError> {
    if width_in_ctus > 1 {
        match ctx.wpp_saved_contexts.take() {
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
        ctx.wpp_saved_contexts = Some(ctx.cabac.contexts);
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
    //     //DEBUG_MORE.store(true, core::sync::atomic::Ordering::Relaxed);
    //
    //     //let v = DEBUG_MORE.load(core::sync::atomic::Ordering::Relaxed);
    //     crate::dbg_println!("read_coding_quadtree cb_size:{}", cb_size, );
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
