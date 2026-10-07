#![allow(dead_code)]
use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SAO_MERGE_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::raw_frame::{RawFrame, SingleFrame, offset_plane};
use std::cmp::min;

#[derive(Default, Debug, Clone, Copy)]
pub struct SaoInfo {
    // todo combine sao_type_idx and sao_eo_class into one byte to save on space
    type_index: u8,
    sao_eo_class: u8,

    sao_band_position: [u8; 3],
    sao_offset_val: [[i8; 4]; 3],
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
        sao_info = *ctx.get_neighbor_sao(x_ctb - 1, y_ctb);
    } else if sao_merge_up_flag {
        // Copy from top CTB
        debug_more!("Merging SAO from UP");
        sao_info = *ctx.get_neighbor_sao(x_ctb, y_ctb - 1);
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
                            _ => ctx.sps.bit_depth_chroma,
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
                                log_offset_scale =
                                    i32::from(range_ext.log2_sao_offset_scale_chroma);
                            }
                        }
                        for j in 0..4 {
                            sao_info.sao_offset_val[i][j] *= (sign[j] << log_offset_scale) as i8;
                        }
                    }
                }
            }
        }
    }
    debug_more!(false=>"{:#?}", sao_info);
    ctx.set_sao_info(x_ctb, y_ctb, sao_info);
}

/// Apply SAO to the whole (deblocked) picture (spec 8.7.3).
///
/// SAO must classify every sample using the deblocked values *before* any SAO
/// is applied, including samples belonging to neighbouring CTBs. Instead of
/// cloning whole planes, each plane is processed row by row in raster order
/// and only two line buffers are kept:
///
/// * `cur`  – the pre-SAO copy of the row being modified (left/right neighbours
///   are read from here because pixels to the left are already modified),
/// * `prev` – the pre-SAO copy of the row above (already modified in place).
///
/// The row below has not been touched yet, so it is read straight from the
/// plane. CTB rows where no CTB uses SAO for a component are skipped entirely
/// (no copy at all), so pictures/components without SAO cost nothing.
pub fn apply_sao_frame(
    frame: &mut RawFrame,
    pic_width: usize,
    pic_height: usize,
    ctu_size: usize,
    sao_buffer: &[SaoInfo], // Pass ctx.ctb_sao_buffer here
) {
    let width_in_ctus = pic_width.div_ceil(ctu_size);
    let height_in_ctus = pic_height.div_ceil(ctu_size);
    let geom = SaoGeometry { width_in_ctus, height_in_ctus };

    // line buffers reused across all planes
    let mut prev = Vec::new();
    let mut cur = Vec::new();

    {
        sao_plane(
            &mut frame.luma, 0, ctu_size, ctu_size, pic_width, pic_height, &geom, sao_buffer, &mut prev,
            &mut cur,
        );
    }

    if frame.format == ChromaFormat::Monochrome {
        return;
    }
    let (sub_x, sub_y) = frame.format.get_subsampling();
    let (ctb_w, ctb_h) = (ctu_size / sub_x, ctu_size / sub_y);
    let (c_pic_w, c_pic_h) = (pic_width / sub_x, pic_height / sub_y);

    for (c_idx, plane) in [(1, &mut frame.cb), (2, &mut frame.cr)] {
        sao_plane(
            plane, c_idx, ctb_w, ctb_h, c_pic_w, c_pic_h, &geom, sao_buffer, &mut prev,
            &mut cur,
        );
    }
}

struct SaoGeometry {
    width_in_ctus:  usize,
    height_in_ctus: usize,
}

#[inline(always)]
fn sao_type_for(info: &SaoInfo, c_idx: usize) -> u8 {
    (info.type_index >> (2 * c_idx)) & 0x3
}

#[allow(clippy::too_many_arguments)]
fn sao_plane(
    plane: &mut SingleFrame, c_idx: usize, ctb_w: usize, ctb_h: usize, pw: usize, ph: usize,
    geom: &SaoGeometry, sao_buffer: &[SaoInfo], prev: &mut Vec<u8>, cur: &mut Vec<u8>,
) {
    if plane.pixels.is_empty() || pw == 0 || ph == 0 {
        return;
    }
    let wc = geom.width_in_ctus;
    let stride = plane.stride;

    prev.clear();
    prev.resize(pw, 0);
    cur.clear();
    cur.resize(pw, 0);

    // true when `prev` holds the pre-SAO copy of the row directly above
    let mut prev_valid = false;

    for ctb_y in 0..geom.height_in_ctus {
        let row_infos = &sao_buffer[ctb_y * wc..(ctb_y + 1) * wc];
        let y0 = ctb_y * ctb_h;
        let y1 = (y0 + ctb_h).min(ph);

        // Nothing to do for this component in this CTB row: rows stay
        // unmodified, so the plane itself remains the pre-SAO source.
        if row_infos.iter().all(|i| sao_type_for(i, c_idx) == 0) {
            prev_valid = false;
            continue;
        }
        // Row above was not modified, so it can be taken straight from the plane.
        if y0 > 0 && !prev_valid {
            let a = offset_plane(plane, 0, y0 - 1);
            prev.copy_from_slice(&plane.pixels[a..a + pw]);
        }

        for y in y0..y1 {
            let row = offset_plane(plane, 0, y);
            cur.copy_from_slice(&plane.pixels[row..row + pw]);

            for (ctb_x, info) in row_infos.iter().enumerate() {
                let t = sao_type_for(info, c_idx);
                if t == 0 {
                    continue;
                }
                let x0 = ctb_x * ctb_w;
                let x1 = (x0 + ctb_w).min(pw);
                let offsets = info.sao_offset_val[c_idx];
                let out = &mut plane.pixels;

                if t == 1 {
                    band_offset_row(
                        &mut out[row + x0..row + x1],
                        &cur[x0..x1],
                        info.sao_band_position[c_idx],
                        offsets,
                    );
                } else {
                    let eo_class = (info.sao_eo_class >> (c_idx * 2)) & 0x3;
                    edge_offset_row(
                        out, row, stride, cur, prev, x0, x1, y, pw, ph, eo_class, offsets,
                    );
                }
            }
            // the pre-SAO copy of this row becomes the "above" row for the next one
            std::mem::swap(prev, cur);
            prev_valid = true;
        }
    }
}

fn band_offset_row(out: &mut [u8], src: &[u8], band_pos: u8, offsets: [i8; 4]) {
    let mut offset_table = [0i16; 32];
    for (i, &o) in offsets.iter().enumerate() {
        offset_table[(band_pos as usize + i) & 31] = i16::from(o);
    }
    for (d, &px) in out.iter_mut().zip(src) {
        // >> 3 is bitDepth - 5 for 8-bit
        let offset = offset_table[(px >> 3) as usize];
        if offset != 0 {
            *d = (i16::from(px) + offset).clamp(0, 255) as u8;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn edge_offset_row(
    out: &mut [u8], row: usize, stride: usize, cur: &[u8], above: &[u8], x0: usize, x1: usize,
    y: usize, pw: usize, ph: usize, eo_class: u8, offsets: [i8; 4],
) {
    // (dx, dy) of the two neighbours, spec Table 8-13 (hPos / vPos)
    let ((dx1, dy1), (dx2, dy2)): ((isize, isize), (isize, isize)) = match eo_class {
        0 => ((-1, 0), (1, 0)),   // horizontal
        1 => ((0, -1), (0, 1)),   // vertical
        2 => ((-1, -1), (1, 1)),  // 135 degree
        3 => ((1, -1), (-1, 1)),  // 45 degree
        _ => unreachable!(),
    };
    // Category -> offset; index is 2 + sign(c - n1) + sign(c - n2)
    let eo_offsets: [i16; 5] = [
        i16::from(offsets[0]), // valley
        i16::from(offsets[1]), // concave corner
        0,                     // flat
        i16::from(offsets[2]), // convex corner
        i16::from(offsets[3]), // peak
    ];

    // Neighbours outside the picture disable SAO for that sample (spec 8.7.3.2):
    // shrink the x range and bail out for whole rows when needed.
    if (dy1 < 0 || dy2 < 0) && y == 0 || (dy1 > 0 || dy2 > 0) && y + 1 >= ph {
        return;
    }
    let x_start = if dx1 < 0 || dx2 < 0 { x0.max(1) } else { x0 };
    let x_end = if dx1 > 0 || dx2 > 0 { x1.min(pw - 1) } else { x1 };

    // Pre-SAO sample at (x + dx, y + dy). Rows above/current come from the
    // line buffers, the row below is still untouched in the plane.
    let fetch = |out: &[u8], x: usize, dx: isize, dy: isize| -> i32 {
        let nx = (x as isize + dx) as usize;
        i32::from(match dy {
            -1 => above[nx],
            0 => cur[nx],
            _ => out[row + stride + nx],
        })
    };

    for x in x_start..x_end {
        let c = i32::from(cur[x]);
        let n1 = fetch(out, x, dx1, dy1);
        let n2 = fetch(out, x, dx2, dy2);
        let edge_idx = (2 + (c - n1).signum() + (c - n2).signum()) as usize;
        let offset = eo_offsets[edge_idx];
        if offset != 0 {
            out[row + x] = (c as i16 + offset).clamp(0, 255) as u8;
        }
    }
}
