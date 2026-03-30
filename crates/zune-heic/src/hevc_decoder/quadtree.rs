use crate::debug_more;
use crate::hevc_decoder::cabac::CabacDecoder;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SPLIT_CU_FLAG;
use crate::hevc_decoder::constants::PartMode;
use crate::hevc_decoder::nal_parser::{NalError, NalUnit};
use crate::hevc_decoder::nal_unit_headers::{Pps, SliceHeader, SliceType, Sps};
use crate::hevc_decoder::nal_unit_parsers::decode_slice_header;
use crate::hevc_decoder::neighbor_tracker::{BlockState, NeighborTracker, PredMode};
use crate::hevc_decoder::quadtree::coding_unit::{read_coding_tree_unit, read_coding_unit};
use crate::hevc_decoder::quadtree::sig_ctx_generator::generate_all_sig_ctx_maps;
use crate::hevc_decoder::utils::extract_rbsp;
use crate::hevc_decoder::{DEBUG_MORE, HevcDecoder};

mod transform_unit;

mod coding_unit;
mod intra;
mod part_mode;
mod quant;
mod residual_block;
mod sao;
mod sig_ctx_generator;

struct DecodeSliceContext<'a> {
    sps:                  &'a Sps,
    pps:                  &'a Pps,
    slice_header:         &'a SliceHeader,
    cabac:                CabacDecoder<'a>,
    neighbor_tracker:     NeighborTracker,
    is_cu_qp_delta_coded: bool,
    cu_qp_delta:          i32,
    // quantization group
    current_qg_x:         usize,
    current_qg_y:         usize,
    last_qp_in_slice:     i8, // This tracks the "previous" QP for the next CU

    // - CU state to be captured for the tracker
    pub is_skip:           bool,
    pub is_intra:          bool,
    pub intra_mode_luma:   u8,
    pub intra_mode_chroma: u8,

    qp_y_prime:  i32,
    qp_cb_prime: i32,
    qp_cr_prime: i32,

    // residual data
    cu_transquant_bypass_flag: bool,
    explicit_rdpcm_flag:       bool,
    explicit_rdpcm_dir:        u8,
    transform_skip_flag:       [u8; 3],
    // context significant maps
    pub sig_ctx_maps:          Vec<Vec<Vec<Vec<Vec<u8>>>>>,
    pub stat_coeff:            [u8; 4],
    pub coeff_list:            [[i16; 32 * 32]; 3],
    pub coeff_pos:             [[i16; 32 * 32]; 3],
    pub n_coeff:               [i16; 3]
}
impl<'a> DecodeSliceContext<'a> {
    fn new(
        sps: &'a Sps, pps: &'a Pps, slice_header: &'a SliceHeader, cabac_engine: CabacDecoder<'a>,
        neighbor_tracker: NeighborTracker, last_qp_in_slice: i8
    ) -> Self {
        Self {
            sps,
            pps,
            slice_header,
            cabac: cabac_engine,
            neighbor_tracker,
            last_qp_in_slice,
            is_cu_qp_delta_coded: false,
            cu_qp_delta: 0,
            current_qg_x: 0,
            current_qg_y: 0,
            is_skip: false,
            is_intra: false,
            intra_mode_luma: 0,
            intra_mode_chroma: 0,
            cu_transquant_bypass_flag: false,
            qp_y_prime: 0,
            qp_cb_prime: 0,
            qp_cr_prime: 0,
            transform_skip_flag: [0; 3],
            explicit_rdpcm_flag: false,
            explicit_rdpcm_dir: 0,
            sig_ctx_maps: generate_all_sig_ctx_maps(),
            stat_coeff: [0; 4],
            coeff_list: [[0; 32 * 32]; 3],
            coeff_pos: [[0; 32 * 32]; 3],
            n_coeff: [0; 3]
        }
    }
}
pub fn decode_slice(nal: &NalUnit, hevc_decoder: &mut HevcDecoder) -> Result<(), NalError> {
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

    let neighbor_tracker = NeighborTracker::new(width_in_8x8, height_in_8x8);

    debug_more!(
        "CTU Size: {}, Width in CTUs: {}, Width in 8x8 units: {}",
        ctu_size,
        width_in_ctus,
        width_in_8x8
    );

    if slice_header.dependent_slice_segment_flag {
        slice_header.slice_addr_rs = hevc_decoder.last_size_header.clone();
    }

    let mut decode_slice_context =
        DecodeSliceContext::new(&sps, &pps, &slice_header, cabac, neighbor_tracker, slice_qp);

    // 5. The CTU Loop
    let total_ctus = width_in_ctus * height_in_ctus;
    for ctu_idx in 0..total_ctus {
        let ctu_x = (ctu_idx % width_in_ctus) * ctu_size; // Convert to pixel coordinates
        let ctu_y = (ctu_idx / width_in_ctus) * ctu_size;

        debug_more!("Decoding CTU at pixel [{}, {}]", ctu_x, ctu_y);
        read_coding_tree_unit(&mut decode_slice_context, ctu_x, ctu_y)?;
    }

    Ok(())
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
    let ctx_v = ctx.neighbor_tracker.get_split_ctx(x0, y0, depth);
    let index = CONTEXT_MODEL_SPLIT_CU_FLAG + ctx_v;
    debug_more!(
        "decode_split_cu_flag -> ctx={} Range={} Value={}",
        ctx_v,
        ctx.cabac.range,
        ctx.cabac.value
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
