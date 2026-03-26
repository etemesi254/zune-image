use std::cmp::min;

use zune_core::log::trace;

use crate::hvec_decoder::DEBUG_MORE;
use crate::hvec_decoder::cabac::CabacEngine;
use crate::hvec_decoder::cabac_tables::{CABAC_INIT_VALUES, CONTEXT_MODEL_SPLIT_CU_FLAG};
use crate::hvec_decoder::context_model::NeighborTracker;
use crate::hvec_decoder::nal_parser::{NalError, NalUnit};
use crate::hvec_decoder::nal_unit_headers::{ChromaFormat, Pps, SliceHeader, SliceType, Sps};
use crate::hvec_decoder::nal_unit_parsers::decode_slice_header;
use crate::hvec_decoder::quadtree::sao::read_sao;
use crate::hvec_decoder::utils::extract_rbsp;

mod sao;
struct DecodeSliceContext<'a> {
    sps:                  &'a Sps,
    pps:                  &'a Pps,
    // slice header
    shdr:                 &'a SliceHeader,
    cabac_engine:         CabacEngine<'a>,
    neighbor_tracker:     NeighborTracker,
    is_cu_qp_delta_coded: bool,
    cu_qp_delta:          i32,
    // quantization group
    current_qg_x:         usize,
    current_qg_y:         usize
}
impl<'a> DecodeSliceContext<'a> {
    fn new(
        sps: &'a Sps, pps: &'a Pps, shdr: &'a SliceHeader, cabac_engine: CabacEngine<'a>,
        neighbor_tracker: NeighborTracker
    ) -> Self {
        Self {
            sps,
            pps,
            shdr,
            cabac_engine,
            neighbor_tracker,
            is_cu_qp_delta_coded: false,
            cu_qp_delta: 0,
            current_qg_x: 0,
            current_qg_y: 0
        }
    }
}
pub fn decode_slice(
    nal: &NalUnit, pps_storage: &[Option<Pps>], sps_storage: &[Option<Sps>]
) -> Result<(), NalError> {
    // 1. Clean the entire NAL unit first! Skip the 2-byte NAL header.
    let clean_rbsp = extract_rbsp(&nal.payload[..]);

    // 2. Pass the clean bytes to the slice header parser
    let slice_header = decode_slice_header(&nal, &pps_storage, &sps_storage, &clean_rbsp)?;
    // 1. Resolve Active Parameter Sets
    let pps = pps_storage[slice_header.slice_pic_parameter_set_id as usize]
        .as_ref()
        .expect("Stream error: PPS missing!");
    let sps = sps_storage[pps.sps_id as usize]
        .as_ref()
        .expect("Stream error: SPS missing!");

    // 2. Extract the raw CABAC payload
    let payload_start = slice_header.cabac_start_position;

    let payload_start = &clean_rbsp[payload_start..];
    // 1. Get the slice type from header
    let init_type = match slice_header.slice_type {
        SliceType::I => 0,

        _ => todo!("Confirm CABAC init flag")
    };

    // 2. The slice_qp derived during header parsing
    let slice_qp = 26 + pps.init_qp_minus26 + slice_header.slice_qp_delta;

    // 4. Create the engine
    let cabac = CabacEngine::new(
        payload_start,
        slice_qp as i32,
        init_type,
        &CABAC_INIT_VALUES
    );

    // 5. The CTU Raster Scan Loop
    let ctu_size = sps.ctb_size_y as usize;
    let width_in_ctus = sps.pic_width_in_ctbs_y as usize;
    let height_in_ctus = sps.pic_height_in_ctbs_y as usize;
    let total_ctus = width_in_ctus * height_in_ctus;

    let neighbor_tracker = NeighborTracker::new(width_in_ctus);

    let mut decode_slice_context =
        DecodeSliceContext::new(&sps, &pps, &slice_header, cabac, neighbor_tracker);

    if DEBUG_MORE {
        println!("decode slice POC={}", slice_header.slice_pic_order_cnt_lsb);
    }
    //let output = vec![];

    for ctu_idx in 0..total_ctus {
        // 1. Calculate our grid coordinates using the ctu_idx
        let ctu_x = ctu_idx % width_in_ctus;
        let ctu_y = ctu_idx / width_in_ctus;

        if DEBUG_MORE {
            println!("ctbX -> {ctu_x}, ctbY -> {ctu_y}");
        }
        read_coding_tree_unit(&mut decode_slice_context, ctu_x, ctu_y)
    }
    Ok(())
}

fn read_coding_quadtree(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8, ct_depth: u8
) {
    if DEBUG_MORE {
        println!(
            "read_coding_quadtree (x0={},y0={},cb_size:{}, depth:{})",
            x0,
            y0,
            1_u64 << log_2_cb_size,
            ct_depth,
        );
    }
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

        read_coding_quadtree(ctx, x0, y0, log_2_cb_size - 1, ct_depth + 1);

        if x1 < sps.pic_width_in_luma_samples as usize {
            read_coding_quadtree(ctx, x1, y0, log_2_cb_size - 1, ct_depth + 1);
        }
        if y1 < sps.pic_height_in_luma_samples as usize {
            read_coding_quadtree(ctx, x0, y1, log_2_cb_size - 1, ct_depth + 1);
        }
        if x1 < sps.pic_width_in_luma_samples as usize
            && y1 < sps.pic_height_in_luma_samples as usize
        {
            read_coding_quadtree(ctx, x1, y1, log_2_cb_size - 1, ct_depth + 1);
        }
    } else {
        // --- THIS IS A LEAF CU ---
        //    1. Record the depth in the tracker for neighbors to use
        ctx.neighbor_tracker.set_split(x0, y0, cb_size, ct_depth);
        read_coding_unit(ctx, x0, y0, log_2_cb_size, ct_depth);
    }
}

fn decode_split_cu_flag(ctx: &mut DecodeSliceContext, x0: usize, y0: usize, depth: u8) -> bool {
    let ctx_v = ctx.neighbor_tracker.get_split_ctx(x0, y0, depth);
    let index = CONTEXT_MODEL_SPLIT_CU_FLAG + ctx_v;
    if DEBUG_MORE {
        println!(
            "decode_split_cu_flag -> ctx={} Range={} Value={}",
            ctx_v, ctx.cabac_engine.range, ctx.cabac_engine.value
        );
    }
    let bit = ctx.cabac_engine.decode_decision(index);
    if DEBUG_MORE {
        println!(
            "decode_split_cu_flag Range=>{}, ctx={} bit={}",
            ctx.cabac_engine.range, ctx_v, bit
        );
    }
    bit == 1
}

fn decode_quantization_parameters(
    ctx: &mut DecodeSliceContext, xc: usize, yc: usize, xc_base: usize, yc_base: usize
) {
    if DEBUG_MORE {
        println!(
            "-------------decode_quantization_parameters( xc={},yc={}) ---------------",
            xc, yc
        );
    }
    let pps = ctx.pps;
    // top left pixel position of current quantization group
    let x_qg = xc_base - (xc_base & ((1 << pps.log2_min_cu_qp_delta_size) - 1));
    let y_gq = yc_base - (yc_base & ((1 << pps.log2_min_cu_qp_delta_size) - 1));

    if DEBUG_MORE {
        println!("X_QG:{},Y_QG:{}", x_qg, y_gq);
    }
    if x_qg != ctx.current_qg_x || y_gq != ctx.current_qg_y {
        
    }
    panic!();
}
fn read_coding_unit(
    ctx: &mut DecodeSliceContext, x0: usize, y0: usize, log_2_cb_size: u8, ct_depth: u8
) {
    let cb_size = 1 << log_2_cb_size;
    if DEBUG_MORE {
        println!(
            "read_coding_unit x0={},y0={},log_2_cb_size:{}",
            x0, y0, cb_size
        );
    }
    decode_quantization_parameters(ctx, x0, y0, x0, y0);
}

fn read_coding_tree_unit(ctx: &mut DecodeSliceContext, ctu_x: usize, ctu_y: usize) {
    let sps = ctx.sps;
    let pps = ctx.pps;
    let shdr = &ctx.shdr;

    let log_2_ctb_size_y =
        sps.log2_min_luma_coding_block_size + sps.log2_diff_max_min_luma_coding_block_size;
    let x_ctb_pixels = ctu_x << log_2_ctb_size_y;
    let y_ctb_pixels = ctu_y << log_2_ctb_size_y;

    if DEBUG_MORE {
        println!(
            "DECODE CTB log_2_ctb_size_y: {log_2_ctb_size_y} x_ctb_pixels: {x_ctb_pixels} y_ctb_pixels: {y_ctb_pixels}"
        );
    }
    if shdr.slice_sao_luma_flag || shdr.slice_sao_chroma_flag {
        let sao_info = read_sao(ctx, x_ctb_pixels, y_ctb_pixels);
    }

    read_coding_quadtree(ctx, x_ctb_pixels, y_ctb_pixels, log_2_ctb_size_y, 0);

    let z = 0;
}
