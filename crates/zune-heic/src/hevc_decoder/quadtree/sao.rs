#![allow(dead_code)]
use std::cmp::min;
use std::sync::Arc;
use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SAO_MERGE_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::raw_frame::{offset_plane, RawFrame, SingleFrame};

#[derive(Default, Debug, Clone)]
pub struct SaoInfo {
    // todo combine sao_type_idx and sao_eo_class into one byte to save on space
    type_index:   u8,
    sao_eo_class: u8,

    sao_band_position: [u8; 3],
    sao_offset_val:    [[i8; 4]; 3]
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
    debug_assert!((7..=31).contains(&c_max));
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
    let value = ctx.cabac.decode_fl_bypass(5) as u8;
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
                            0 => ctx.sps.bit_depth_luma,
                            _ => ctx.sps.bit_depth_chroma
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
                                let s = if decode_sao_offset_sign(ctx) == 0 { 1 } else { -1 }; // 0 is positive in HEVC
                                sao_info.sao_offset_val[i][j] *= s;
                            }
                        }
                        sao_info.sao_band_position[i] = decode_sao_band_position(ctx);
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
                                log_offset_scale = i32::from(range_ext.log2_sao_offset_scale_luma);
                            } else {
                                log_offset_scale = i32::from(range_ext.log2_sao_offset_scale_chroma);
                            }
                        }
                        for j in 0..4 {
                            sao_info.sao_offset_val[i][j] *= (sign[j]  << log_offset_scale) as i8;
                        }
                    }
                }
            }
        }
    }
    debug_more!(false=>"{:#?}", sao_info);
    ctx.set_sao_info(x_ctb, y_ctb, sao_info);
}

pub fn apply_sao_frame(
    frame: &Arc<RawFrame>,
    pic_width: usize,
    pic_height: usize,
    ctu_size: usize,
    sao_buffer: &[SaoInfo], // Pass ctx.ctb_sao_buffer here
) {
    let mut luma = frame.luma.lock().unwrap();
    let mut cb = frame.cb.lock().unwrap();
    let mut cr = frame.cr.lock().unwrap();

    let width_in_ctus = pic_width.div_ceil(ctu_size);
    let height_in_ctus = pic_height.div_ceil(ctu_size);

    for y_ctb in 0..height_in_ctus {
        for x_ctb in 0..width_in_ctus {
            let ctb_addr = y_ctb * width_in_ctus + x_ctb;
            let info = &sao_buffer[ctb_addr];

            // If type_index is 0, SAO is disabled for this entire CTU
            if info.type_index == 0 {
                continue;
            }

            // --- Luma Dimensions ---
            let l_x = x_ctb * ctu_size;
            let l_y = y_ctb * ctu_size;
            let l_w = ctu_size.min(pic_width - l_x);
            let l_h = ctu_size.min(pic_height - l_y);

            // --- Apply Luma (c_idx = 0) ---
            let type_luma = info.type_index & 0x3;
            if type_luma != 0 {
                apply_sao_component(
                    &mut luma, l_x, l_y, l_w, l_h, 0, info, type_luma, pic_width, pic_height
                );
            }

            let c_x = l_x / 2;
            let c_y = l_y / 2;
            let c_w = l_w / 2;
            let c_h = l_h / 2;
            let c_pic_w = pic_width / 2;
            let c_pic_h = pic_height / 2;

            // --- Apply Cb (c_idx = 1) ---
            let type_cb = (info.type_index >> 2) & 0x3;
            if type_cb != 0 {
                apply_sao_component(
                    &mut cb, c_x, c_y, c_w, c_h, 1, info, type_cb, c_pic_w, c_pic_h
                );
            }

            // --- Apply Cr (c_idx = 2) ---
            let type_cr = (info.type_index >> 4) & 0x3;
            if type_cr != 0 {
                apply_sao_component(
                    &mut cr, c_x, c_y, c_w, c_h, 2, info, type_cr, c_pic_w, c_pic_h
                );
            }
        }
    }
}
fn apply_sao_component(
    plane: &mut SingleFrame, x0: usize, y0: usize, w: usize, h: usize,
    c_idx: usize, info: &SaoInfo, type_idx: u8, pic_width: usize, pic_height: usize
) {
    let offsets = &info.sao_offset_val[c_idx];

    if type_idx == 1 {
        // Band Offset
        let band_pos = info.sao_band_position[c_idx];
        apply_band_offset(plane, x0, y0, w, h, band_pos, offsets);
    } else if type_idx == 2 {
        // Edge Offset
        let eo_class = (info.sao_eo_class >> (c_idx * 2)) & 0x3;
        apply_edge_offset(plane, x0, y0, w, h, eo_class, offsets, pic_width, pic_height);
    }
}
fn apply_band_offset(
    plane: &mut SingleFrame, x0: usize, y0: usize, w: usize, h: usize,
    band_pos: u8, offsets: &[i8; 4]
) {
    let mut offset_table = [0i16; 32];
    let start_band = band_pos as usize;

    for i in 0..4 {
        let band_idx = (start_band + i) & 31;
        offset_table[band_idx] = i16::from(offsets[i]);
    }


    for y in 0..h {
        let mut idx = offset_plane(plane,x0, y0 + y);
        for _ in 0..w {
            let px = plane.pixels[idx];
            let band_idx = (px >> 3) as usize; // >> 3 is for 8-bit. Use >> 5 for 10-bit.
            let offset = offset_table[band_idx];

            if offset != 0 {
                plane.pixels[idx] = (i16::from(px) + offset).clamp(0, 255) as u8;
            }
            idx += 1;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn apply_edge_offset(
    plane: &mut SingleFrame, x0: usize, y0: usize, w: usize, h: usize,
    eo_class: u8, offsets: &[i8; 4], pic_width: usize, pic_height: usize
) {
    let (dx1, dy1, dx2, dy2): (isize, isize, isize, isize) = match eo_class {
        0 => (-1, 0, 1, 0),   // Horizontal
        1 => (0, -1, 0, 1),   // Vertical
        2 => (-1, -1, 1, 1),  // 135 degree
        3 => (1, -1, -1, 1),  // 45 degree
        _ => unreachable!(),
    };

    let eo_offsets: [i16; 5] = [
        i16::from(offsets[0]), // Category 1 (Valley)
        i16::from(offsets[1]), // Category 2 (Half-Valley)
        0,                     // Category 0 (Plane)
        i16::from(offsets[2]), // Category 3 (Half-Peak)
        i16::from(offsets[3]), // Category 4 (Peak)
    ];

    let stride = plane.stride as isize;

    let sign = |val: i32| -> isize {
        if val > 0 { 1 } else if val < 0 { -1 } else { 0 }
    };

    for y in 0..h {
        let abs_y = y0 + y;
        for x in 0..w {
            let abs_x = x0 + x;

            // Spec 8.7.3.2.3: Do not apply if neighbors cross the picture boundaries.
            if (abs_x as isize + dx1 < 0) || (abs_x as isize + dx2 >= pic_width as isize) ||
                (abs_y as isize + dy1 < 0) || (abs_y as isize + dy2 >= pic_height as isize) {
                continue;
            }

            let c_idx = offset_plane(plane,abs_x, abs_y);
            let n1_idx = (c_idx as isize + dy1 * stride + dx1) as usize;
            let n2_idx = (c_idx as isize + dy2 * stride + dx2) as usize;

            let c_val = i32::from(plane.pixels[c_idx]);
            let n1_val = i32::from(plane.pixels[n1_idx]);
            let n2_val = i32::from(plane.pixels[n2_idx]);

            let edge_idx = (2 + sign(c_val - n1_val) + sign(c_val - n2_val)) as usize;
            let offset = eo_offsets[edge_idx];

            if offset != 0 {
                plane.pixels[c_idx] = (c_val as i16 + offset).clamp(0, 255) as u8;
            }
        }
    }
}