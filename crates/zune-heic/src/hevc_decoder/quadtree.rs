use std::sync::Arc;

use crate::debug_more;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SPLIT_CU_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::deblocker::deblock_frame;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::SliceType;
use crate::hevc_decoder::nal_unit_parsers::decode_slice_header;
use crate::hevc_decoder::quadtree::coding_unit::{read_coding_tree_unit, read_coding_unit};
use crate::hevc_decoder::quadtree::sao::apply_sao_frame;
use crate::hevc_decoder::raw_frame::RawFrame;
use crate::hevc_decoder::utils::extract_rbsp;
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
    nal: &NalUnit, hevc_decoder: &mut HevcDecoder, raw_frame: Arc<RawFrame>
) -> Result<(), NalError> {
    let clean_rbsp = extract_rbsp(nal.payload);
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

    // 4. Update the existing Neighbor Tracker
    let neighbor_tracker = hevc_decoder.neighbor_tracker.as_mut().unwrap();

    let mut ctx = DecodeSliceContext::new(
        sps,
        pps,
        &slice_header,
        cabac,
        neighbor_tracker,
        //&mut hevc_decoder.neighbor_tracker, // Borrow the persistent tracker
        slice_qp,
        raw_frame.clone()
    );

    // 5. The CTU Loop (starts at slice address)
    for ctu_addr in start_ctu_addr..total_ctus {
        let ctu_x = ctu_addr % width_in_ctus;
        let ctu_y = ctu_addr / width_in_ctus;

        debug_more!("--- Decoding CTU {} [{}, {}] ---", ctu_addr, ctu_x, ctu_y);

        // --- WPP CABAC Context Inheritance (The "Load") ---
        // 1. Is WPP enabled?
        // 2. Are we at the start of a row? (x == 0)
        // 3. Are we NOT on the first row? (y > 0)
        // 4. Are we NOT at the very start of an independent slice? (ctu_addr > start_ctu_addr)
        if pps.entropy_coding_sync_enabled_flag
            && ctu_x == 0
            && ctu_y > 0
            && ctu_addr > start_ctu_addr
        {
            if width_in_ctus > 1 {
                // Normal WPP: Pull the context saved by the previous row at ctu_x == 1
                // .take() moves it out of the Option, leaving None behind (saves memory)
                if let Some(wpp_contexts) = ctx.ctb_context[ctu_y - 1].take() {
                    debug_more!("WPP: Inheriting CABAC contexts from row {}", ctu_y - 1);
                    ctx.cabac.contexts = wpp_contexts;
                } else {
                    return Err(NalError::Generic(format!(
                        "WPP Error: Expected saved CABAC context for row {}, but found None!",
                        ctu_y - 1
                    )));
                }
            } else {
                // Edge Case: Video is only 1 CTU wide. There was no ctu_x == 1 to save from.
                // Spec says we just re-initialize the probabilities from the Slice QP.
                ctx.cabac.init_contexts(i32::from(slice_qp), init_type);
            }
        }
        read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;

        match finish_ctu(&mut ctx, ctu_x, ctu_y)? {
            CtuStatus::EndOfSliceSegment => {
                debug_more!("Slice Segment Finished at CTU {}", ctu_addr);
                debug_more!("cursor:{} len:{}", ctx.cabac.cursor, ctx.cabac.data.len());

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

    // apply deblocking
    if false {
        let rf_clone = raw_frame.clone();
        deblock_frame(
            &rf_clone,
            hevc_decoder.width,
            hevc_decoder.height,
            ctx.neighbor_tracker,
            pps.cb_qp_offset as i8,
            pps.cr_qp_offset as i8
        );
        apply_sao_frame(
            &raw_frame,
            hevc_decoder.width,
            hevc_decoder.height,
            1 << sps.log2_ctb_size_y,
            &ctx.ctb_sao_buffer
        )
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
        ctx.ctb_context[ctby] = Some(ctx.cabac.contexts);
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
    let curr_addr_rs = ctby * (sps.pic_width_in_ctbs_y as usize) + ctbx;

    // Check for Tile Change (Section 7.3.8.1)
    if pps.tiles_enabled_flag {
        // HEVC decodes tiles in Tile Scan order, not Raster Scan.
        // Assuming your loop handles the TS -> RS mapping:
        let _curr_tile_id = pps.tile_id_rs[curr_addr_rs];
        let _next_addr_rs = curr_addr_rs + 1; // Raster scan increment

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
