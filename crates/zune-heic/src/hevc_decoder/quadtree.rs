use std::sync::Arc;

use crate::debug_more;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SPLIT_CU_FLAG;
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{Pps, SliceHeader, SliceType, Sps};
use crate::hevc_decoder::nal_unit_parsers::decode_slice_header;
use crate::hevc_decoder::neighbor_tracker::{BlockState, NeighborTracker, PredMode};
use crate::hevc_decoder::quadtree::coding_unit::{read_coding_tree_unit, read_coding_unit};
use crate::hevc_decoder::quadtree::sig_ctx_generator::generate_all_sig_ctx_maps;
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

pub fn decode_slice_(
    nal: &NalUnit, hevc_decoder: &mut HevcDecoder, raw_frame: Arc<RawFrame>
) -> Result<(), NalError> {
    debug_more!("---------------decode-slice-------------");
    let clean_rbsp = extract_rbsp(&nal.payload[..]);
    let sps_storage = &hevc_decoder.sps_storage;
    let pps_storage = &hevc_decoder.pps_storage;

    let mut slice_header = decode_slice_header(&nal, &pps_storage, &sps_storage, &clean_rbsp)?;

    let pps = pps_storage[slice_header.slice_pic_parameter_set_id as usize]
        .as_ref()
        .expect("Stream error: PPS missing!");
    let sps = sps_storage[pps.sps_id as usize]
        .as_ref()
        .expect("Stream error: SPS missing!");

    // 1. Get the slice type from header
    let init_type = match slice_header.slice_type {
        SliceType::I => 0,

        _ => todo!("Confirm CABAC init flag")
    };

    // 2. Derive Slice QP
    let slice_qp = (26 + pps.init_qp_minus26 + slice_header.slice_qp_delta) as i8;

    // 3. Initialize CABAC Engine with the corrected init_type
    let payload_start = &clean_rbsp[slice_header.cabac_start_position..];
    let cabac = CabacDecoder::new(payload_start, slice_qp as i32, init_type);

    // 4. Fix the Neighbor Tracker size
    let ctu_size = sps.ctb_size_y as usize;
    let width_in_ctus = sps.pic_width_in_ctbs_y as usize;
    let height_in_ctus = sps.pic_height_in_ctbs_y as usize;

    // The tracker needs to know the width in 8x8 units (Minimum Coding Block size)
    let width_in_8x8 = (width_in_ctus * ctu_size) / 8;
    let height_in_8x8 = (height_in_ctus * ctu_size) / 8;
    let log2_unit_size = sps.log2_min_luma_coding_block_size - 1;

    debug_more!(
        "CTU Size: {}, Width in CTUs: {}, Width in 8x8 units: {}",
        ctu_size,
        width_in_ctus,
        width_in_8x8
    );

    if slice_header.dependent_slice_segment_flag {
        slice_header.slice_addr_rs = hevc_decoder.last_size_header.clone();
    }

    let neighbor_tracker = hevc_decoder.neighbor_tracker.as_mut().unwrap();

    let mut ctx = DecodeSliceContext::new(
        &sps,
        &pps,
        &slice_header,
        cabac,
        neighbor_tracker,
        slice_qp,
        raw_frame
    );

    // 5. The CTU Loop
    let total_ctus = width_in_ctus * height_in_ctus;
    for ctu_idx in 0..total_ctus {
        let ctu_x = (ctu_idx % width_in_ctus); // Convert to pixel coordinates
        let ctu_y = (ctu_idx / width_in_ctus);

        debug_more!(
            "Decoding CTU at pixel [{}, {}]",
            ctu_x * ctu_size,
            ctu_y * ctu_size
        );
        read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;
        finish_ctu(&mut ctx, ctu_x, ctu_y)?;
    }

    Ok(())
}

pub fn decode_slice(
    nal: &NalUnit, hevc_decoder: &mut HevcDecoder, raw_frame: Arc<RawFrame>
) -> Result<(), NalError> {
    let clean_rbsp = extract_rbsp(&nal.payload[..]);
    let sps_storage = &hevc_decoder.sps_storage;
    let pps_storage = &hevc_decoder.pps_storage;

    let slice_header = decode_slice_header(&nal, &pps_storage, &sps_storage, &clean_rbsp)?;

    let pps = pps_storage[slice_header.slice_pic_parameter_set_id as usize]
        .as_ref()
        .unwrap();
    let sps = sps_storage[pps.sps_id as usize].as_ref().unwrap();

    // 1. Initial QP
    let slice_qp = (26 + pps.init_qp_minus26 + slice_header.slice_qp_delta) as i8;

    // 2. CABAC Initialization (Section 9.3.2.2)
    // Dependent slices inherit the previous CABAC state, independent ones reset.
    let payload_start = &clean_rbsp[slice_header.cabac_start_position..];
    let mut cabac = if slice_header.dependent_slice_segment_flag {
        todo!("Dependent slice segment flag")
        // let mut prev_cabac = hevc_decoder.last_cabac_state.take().expect("Dependent slice without parent!");
        // prev_cabac.update_data(payload_start);
        // prev_cabac
    } else {
        // Derive init_type (0=I, 1=P, 2=B)
        let init_type = match slice_header.slice_type {
            SliceType::I => 0,
            SliceType::P => 1,
            SliceType::B => 2
        };
        CabacDecoder::new(payload_start, slice_qp as i32, init_type)
    };

    // 3. CTU Range (Raster Scan Address)

    let width_in_ctus = sps.pic_width_in_ctbs_y as usize;

    let start_ctu_addr = slice_header.slice_segment_address as usize;
    let total_ctus = width_in_ctus * (sps.pic_height_in_ctbs_y as usize);

    // 4. Update the existing Neighbor Tracker
    let neighbor_tracker = hevc_decoder.neighbor_tracker.as_mut().unwrap();

    let mut ctx = DecodeSliceContext::new(
        &sps,
        &pps,
        &slice_header,
        cabac,
        neighbor_tracker,
        //&mut hevc_decoder.neighbor_tracker, // Borrow the persistent tracker
        slice_qp,
        raw_frame
    );

    // 5. The CTU Loop (starts at slice address)
    for ctu_addr in start_ctu_addr..total_ctus {
        let ctu_x = ctu_addr % width_in_ctus;
        let ctu_y = ctu_addr / width_in_ctus;

        debug_more!("--- Decoding CTU {} [{}, {}] ---", ctu_addr, ctu_x, ctu_y);

        read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;

        // finish_ctu handles the end_of_slice_segment_flag terminal bit
        match finish_ctu(&mut ctx, ctu_x, ctu_y)? {
            CtuStatus::EndOfSliceSegment => {
                debug_more!("Slice Segment Finished at CTU {}", ctu_addr);

                // If it's a dependent slice, save the state for the next one
                if pps.dependent_slice_segments_enabled_flag {
                    todo!("Dependent slice segment flag")
                    // hevc_decoder.last_cabac_state = Some(ctx.cabac.clone());
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
    if pps.entropy_coding_sync_enabled_flag && ctbx == 1 {
        if ctby + 1 < sps.pic_height_in_ctbs_y as usize {
            debug_more!("Saving WPP Context for row {}", ctby);
            // We clone the current context model state (the "decouple" in libde265)
            // ctx.wpp_context_models[ctby] = ctx.cabac.contexts.clone();
        }
    }

    // --- 2. Decode Terminal Bit (end_of_slice_segment_flag) ---
    // This bit is mandatory after every CTU (Section 7.3.8.1)
    let end_of_slice_segment_flag = ctx.cabac.decode_terminate();
    debug_more!("end_of_slice_segment_flag -> {}", end_of_slice_segment_flag);

    if end_of_slice_segment_flag != 0 {
        // If dependent slices are enabled, save context for the next slice header
        if pps.dependent_slice_segments_enabled_flag {
            debug_more!("Saving context for dependent slice");
            // ctx.shdr.ctx_model_storage = Some(ctx.cabac.ctx_model.clone());
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
        let curr_tile_id = pps.tile_id_rs[curr_addr_rs];
        let next_addr_rs = curr_addr_rs + 1; // Raster scan increment

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
            debug_more!("ERROR: end_of_sub_stream_one_bit was 0!");
            return Err(NalError::Generic(
                "end_of_sub_stream_one_bit was 0!".to_string()
            ));
        }

        // Align CABAC: This flushes the engine and skips to the next byte
        ctx.cabac.init_cabac();

        return Ok(CtuStatus::EndOfSubstream);
    }

    ctx.math_scratchpad.fill(0);
    ctx.n_coeff.fill(0);
    ctx.coeff_list.iter_mut().for_each(|c| c.fill(0));
    ctx.coeff_pos.iter_mut().for_each(|c| c.fill(0));
    ctx.ref_samples_p.fill(0);
    ctx.ref_main_buf.fill(0);
    ctx.ref_samples_available.fill(false);
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
        split_cu_flag = decode_split_cu_flag(ctx, x0, y0, ct_depth)
    } else {
        if log_2_cb_size > sps.log2_min_luma_coding_block_size {
            split_cu_flag = true;
        }
    }
    if pps.cu_qp_delta_enabled_flag && log_2_cb_size >= pps.log2_min_cu_qp_delta_size {
        ctx.is_cu_qp_delta_coded = false;
        ctx.cu_qp_delta = 0;
    }
    // TODO: libde265 has some flags we are not reading
    //      so investigate
    //    Code
    //
    //      if (tctx->shdr->cu_chroma_qp_offset_enabled_flag &&
    //         log2CbSize >= pps.Log2MinCuChromaQpOffsetSize) {
    //         tctx->IsCuChromaQpOffsetCoded = 0;
    //     }

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
        read_coding_unit(ctx, x0, y0, log_2_cb_size, ct_depth)?;

        // 2. NOW update the neighbor tracker with the finalized state
        // We use the last_qp_in_slice because that is the official QP for this area
        debug_more!(
            "Updating tracker for Leaf CU at [{},{}] with QP {}",
            x0,
            y0,
            ctx.last_qp_in_slice
        );

        // 2. Prepare the block state for neighbors
        let final_state = BlockState {
            skip_flag: ctx.is_skip,
            cqt_depth: ct_depth,
            is_intra: ctx.is_intra,
            intra_mode_luma: ctx.intra_mode_luma,
            qp: ctx.last_qp_in_slice,
            slice_id: ctx.slice_header.slice_segment_address as u16,
            ..BlockState::default()
        };

        debug_more!(
            "CU [{},{}] size {} finished. State: Intra={}, Skip={}, QP={}",
            x0,
            y0,
            cb_size,
            final_state.is_intra,
            final_state.skip_flag,
            final_state.qp
        );

        // 3. Commit to tracker (updates both line buffer and left column)
        ctx.neighbor_tracker
            .update_block(x0, y0, cb_size, final_state);

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
