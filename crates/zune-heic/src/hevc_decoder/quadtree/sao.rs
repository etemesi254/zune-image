use std::cmp::min;
use zune_core::log::info;
use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SAO_MERGE_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;

#[derive(Default, Debug, Clone)]
pub struct SaoInfo {
    // todo combine sao_type_idx and sao_eo_class into one byte to save on space
    type_index:   u8,
    sao_eo_class: u8,

    sao_band_position: [u8; 3],
    sao_offset_val:    [[i8; 3]; 4]
}
fn decode_sao_type_idx(ctx: &mut DecodeSliceContext) -> u8 {
    const OFF_SAO_TYPE: usize = 1;
    debug_more!("decode_sao_type_idx(luma/chroma)");

    let bit0 = ctx.cabac.decode_decision(OFF_SAO_TYPE);

    return if bit0 == 0 {
        debug_more!("decode_sao_type_idx(bit0) {}", bit0);
        0
    } else {
        let bit1 = ctx.cabac.decode_bypass();

        if bit1 == 0 {
            debug_more!("decode_sao_type_idx(bit1) {}", 1);
            1
        } else {
            debug_more!("decode_sao_type_idx(bit1) {}", 2);
            2
        }
    };
}
fn decode_sao_offset_abs(ctx: &mut DecodeSliceContext, bit_depth: u8) -> u8 {
    debug_more!("sao_offset_abs");
    let c_max = (1 << (min(bit_depth, 10) - 5)) - 1;
    debug_assert!(c_max >= 7 && c_max <= 31);
    let value = ctx.cabac.decode_tu_bypass(c_max);

    debug_more!("sao_offset_abs(value) {}", value);

    value
}
fn decode_sao_offset_sign(ctx: &mut DecodeSliceContext) -> u8 {
    debug_more!("sao_offset_sign");
    let value = ctx.cabac.decode_bypass();
    debug_more!("sao_offset_sign(value) {}", value);
    value
}
fn decode_sao_band_position(ctx: &mut DecodeSliceContext) -> u8 {
    debug_more!("sao_band_position");
    let value = ctx.cabac.decode_bypass();
    debug_more!("sao_band_position(value) {}", value);
    value
}
fn decode_sao_class(ctx: &mut DecodeSliceContext) -> u8 {
    debug_more!("sao_class");

    let value = ctx.cabac.decode_fl_bypass(2);

    debug_more!("sao_class(value) {}", value);
    value as u8
}
fn decode_sao_merge_flag(ctx: &mut DecodeSliceContext) -> u8 {
    debug_more!("sao_merge_up/left_flag");
    let bit = ctx.cabac.decode_decision(CONTEXT_MODEL_SAO_MERGE_FLAG);
    debug_more!("decode_sao_merge_up/left_flag(bit) {}", bit);
    bit
}

pub fn read_sao(ctx: &mut DecodeSliceContext, x_ctb: usize, y_ctb: usize) {
    let shdr = ctx.slice_header;
    let sps = ctx.sps;

    debug_more!("read_sao ({} {})", x_ctb, y_ctb);
    let mut sao_info;

    let mut sao_merge_left_flag = false;
    let mut sao_merge_up_flag = false;

    if x_ctb > 0 {
        // Merge left uses context 0
        sao_merge_left_flag = decode_sao_merge_flag(ctx) != 0;
    }

    if y_ctb > 0 && !sao_merge_left_flag {
        // Merge up uses context 0
        sao_merge_up_flag = decode_sao_merge_flag(ctx) != 0;
    }

    if sao_merge_left_flag {
        // Copy from left CTB
        debug_more!("Merging SAO from LEFT");
        sao_info = ctx.get_neighbor_sao(x_ctb - 1, y_ctb).clone();
    } else if sao_merge_up_flag {
        // Copy from top CTB
        debug_more!("Merging SAO from UP");
        sao_info = ctx.get_neighbor_sao(x_ctb, y_ctb - 1).clone();
    } else {
        sao_info = SaoInfo::default();
        let mut n_chroma = 3;
        if sps.chroma_format == ChromaFormat::Monochrome {
            n_chroma = 1;
        }

        for i in 0..n_chroma {
            if (shdr.slice_sao_luma_flag && i == 0) || (shdr.slice_sao_chroma_flag && i > 0) {
                let sao_type_idx;

                if i == 0 {
                    let sao_type_idx_luma = decode_sao_type_idx(ctx);

                    debug_more!("sao_type_idx_luma -> {sao_type_idx_luma}");
                    sao_type_idx = sao_type_idx_luma;
                    sao_info.type_index = sao_type_idx_luma;
                } else if i == 1 {
                    let sao_idx_chroma = decode_sao_type_idx(ctx);

                    debug_more!("sao_idx_chroma -> {sao_idx_chroma}");
                    sao_type_idx = sao_idx_chroma;

                    // set for both chroma components
                    sao_info.type_index |= sao_type_idx << 2;
                    sao_info.type_index |= sao_type_idx << 4;
                } else {
                    sao_type_idx = (sao_info.type_index >> (2 * i)) & 0x3;
                }

                if sao_type_idx != 0 {
                    for j in 0..4 {
                        let bit_depth = match i {
                            0 => ctx.sps.bit_depth_luma as u8,
                            _ => ctx.sps.bit_depth_chroma as u8
                        };
                        sao_info.sao_offset_val[i][j] = decode_sao_offset_abs(ctx, bit_depth) as i8;

                        debug_more!(
                            " sao_offset_val[{}][{}]= {}",
                            i,
                            j,
                            sao_info.sao_offset_val[i][j]
                        );
                    }
                    let mut sign: [i32; 4] = [0; 4];
                    if sao_type_idx == 1 {
                        for j in 0..4 {
                            if sao_info.sao_offset_val[i][j] != 0 {
                                sign[i] = if decode_sao_offset_sign(ctx) == 0 { -1 } else { 1 };
                            }
                        }

                        sao_info.sao_band_position[i] = decode_sao_band_position(ctx)
                    } else {
                        sign[0] = 1;
                        sign[1] = 1;

                        sign[2] = -1;
                        sign[3] = -1;

                        if i == 0 {
                            sao_info.sao_eo_class = decode_sao_class(ctx);
                        } else if i == 1 {
                            let sao_eo_class = decode_sao_class(ctx);

                            sao_info.sao_eo_class |= sao_eo_class << 2;
                            sao_info.sao_eo_class |= sao_eo_class << 4;
                        }

                        debug_more!("sao_eo_class[{}](value) {}", i, sao_info.sao_eo_class);

                        let mut log_offset_scale: i32 = 0;

                        if let Some(range_ext) = ctx.pps.range_extension.as_ref() {
                            if i == 0 {
                                log_offset_scale = range_ext.log2_sao_offset_scale_luma as i32;
                            } else {
                                log_offset_scale = range_ext.log2_sao_offset_scale_chroma as i32;
                            }
                        }
                        for j in 0..4 {
                            sao_info.sao_offset_val[i][j] = (sign[i]
                                * ((sao_info.sao_offset_val[i][j] as i32) << log_offset_scale))
                                as i8;
                        }
                    }
                }
            }
        }
    }
    debug_more!(false=>"{:#?}", sao_info);
    ctx.set_sao_info(x_ctb, y_ctb, sao_info);
}
