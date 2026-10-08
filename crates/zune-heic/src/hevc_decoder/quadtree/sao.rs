#![allow(dead_code)]
use alloc::vec::Vec;
use crate::debug_more;
use crate::hevc_decoder::DEBUG_MORE;
use crate::hevc_decoder::cabac_tables::CONTEXT_MODEL_SAO_MERGE_FLAG;
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_unit_headers::ChromaFormat;
use crate::hevc_decoder::raw_frame::{RawFrame, RowBand, SingleFrame, band_cuts, for_each_band, offset_plane};
use core::cmp::min;

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
/// is applied, including samples belonging to neighbouring CTBs. Each plane is
/// split into bands of whole CTB rows, processed in parallel (`threads`).
/// Before any band starts, the pre-SAO rows just above and just below every
/// band are copied out, since a neighbouring band may already have modified
/// them by the time they are read.
///
/// Inside a band, rows are processed top to bottom with two line buffers:
///
/// * `cur`  – the pre-SAO copy of the row being modified (left/right neighbours
///   are read from here because pixels to the left are already modified),
/// * `prev` – the pre-SAO copy of the row above (already modified in place).
///
/// The row below has not been touched yet, so it is read straight from the
/// band (or from the saved copy for the band's last row). CTB rows where no
/// CTB uses SAO for a component are skipped entirely.
pub fn apply_sao_frame(
    frame: &mut RawFrame,
    pic_width: usize,
    pic_height: usize,
    ctu_size: usize,
    sao_buffer: &[SaoInfo],
    threads: usize,
) {
    let width_in_ctus = pic_width.div_ceil(ctu_size);
    let height_in_ctus = pic_height.div_ceil(ctu_size);
    let geom = SaoGeometry { width_in_ctus, height_in_ctus, threads };

    sao_plane(&mut frame.luma, 0, ctu_size, ctu_size, pic_width, pic_height, &geom, sao_buffer);

    if frame.format == ChromaFormat::Monochrome {
        return;
    }
    let (sub_x, sub_y) = frame.format.get_subsampling();
    let (ctb_w, ctb_h) = (ctu_size / sub_x, ctu_size / sub_y);
    let (c_pic_w, c_pic_h) = (pic_width / sub_x, pic_height / sub_y);

    for (c_idx, plane) in [(1, &mut frame.cb), (2, &mut frame.cr)] {
        sao_plane(plane, c_idx, ctb_w, ctb_h, c_pic_w, c_pic_h, &geom, sao_buffer);
    }
}

struct SaoGeometry {
    width_in_ctus:  usize,
    height_in_ctus: usize,
    threads:        usize,
}

#[inline(always)]
fn sao_type_for(info: &SaoInfo, c_idx: usize) -> u8 {
    (info.type_index >> (2 * c_idx)) & 0x3
}

#[allow(clippy::too_many_arguments)]
fn sao_plane(
    plane: &mut SingleFrame, c_idx: usize, ctb_w: usize, ctb_h: usize, pw: usize, ph: usize,
    geom: &SaoGeometry, sao_buffer: &[SaoInfo],
) {
    if plane.pixels.is_empty() || pw == 0 || ph == 0 {
        return;
    }
    let wc = geom.width_in_ctus;
    let has_sao = |ctb_y: usize| {
        sao_buffer[ctb_y * wc..(ctb_y + 1) * wc]
            .iter()
            .any(|i| sao_type_for(i, c_idx) != 0)
    };
    if !(0..geom.height_in_ctus).any(has_sao) {
        return;
    }

    let cuts = band_cuts(ph, geom.threads, ctb_h, 0);
    // pre-SAO copies of the row above and the row below each band
    let row = |y: usize| {
        let at = offset_plane(plane, 0, y);
        plane.pixels[at..at + pw].to_vec()
    };
    let edges: Vec<(Vec<u8>, Vec<u8>)> = cuts
        .windows(2)
        .map(|r| {
            let above = if r[0] > 0 { row(r[0] - 1) } else { Vec::new() };
            let below = if r[1] < ph { row(r[1]) } else { Vec::new() };
            (above, below)
        })
        .collect();

    for_each_band(plane, &cuts, |band| {
        let (above, below) = &edges[band.index];
        sao_band(band, c_idx, ctb_w, ctb_h, pw, ph, wc, sao_buffer, above, below);
    });
}

#[allow(clippy::too_many_arguments)]
fn sao_band(
    band: &mut RowBand, c_idx: usize, ctb_w: usize, ctb_h: usize, pw: usize, ph: usize, wc: usize,
    sao_buffer: &[SaoInfo], above: &[u8], below: &[u8],
) {
    let mut prev = vec![0; pw];
    let mut cur = vec![0; pw];

    // true when `prev` holds the pre-SAO copy of the row directly above
    let mut prev_valid = !above.is_empty();
    if prev_valid {
        prev.copy_from_slice(above);
    }

    for ctb_y in band.rows.start / ctb_h..band.rows.end.div_ceil(ctb_h) {
        let row_infos = &sao_buffer[ctb_y * wc..(ctb_y + 1) * wc];
        let y0 = ctb_y * ctb_h;
        let y1 = (y0 + ctb_h).min(ph);

        // Nothing to do for this component in this CTB row: rows stay
        // unmodified, so the plane itself remains the pre-SAO source.
        if row_infos.iter().all(|i| sao_type_for(i, c_idx) == 0) {
            prev_valid = false;
            continue;
        }
        // Row above (inside this band) was not modified, so it can be taken
        // straight from the plane.
        if y0 > 0 && !prev_valid {
            let a = band.index(0, y0 - 1);
            prev.copy_from_slice(&band.pixels[a..a + pw]);
        }

        for y in y0..y1 {
            let row = band.index(0, y);
            cur.copy_from_slice(&band.pixels[row..row + pw]);

            // the row below: untouched in the band, or saved for the last row
            let split = (row + band.stride).min(band.pixels.len());
            let (head, tail) = band.pixels.split_at_mut(split);
            let below_row = if y + 1 < band.rows.end { &tail[..pw] } else { below };
            let out = &mut head[row..row + pw];

            for (ctb_x, info) in row_infos.iter().enumerate() {
                let t = sao_type_for(info, c_idx);
                if t == 0 {
                    continue;
                }
                let x0 = ctb_x * ctb_w;
                let x1 = (x0 + ctb_w).min(pw);
                let offsets = info.sao_offset_val[c_idx];

                if t == 1 {
                    band_offset_row(
                        &mut out[x0..x1],
                        &cur[x0..x1],
                        info.sao_band_position[c_idx],
                        offsets,
                    );
                } else {
                    let eo_class = (info.sao_eo_class >> (c_idx * 2)) & 0x3;
                    let lines = EoLines { above: &prev, cur: &cur, below: below_row };
                    edge_offset_row(out, &lines, x0, x1, y, pw, ph, eo_class, offsets);
                }
            }
            // the pre-SAO copy of this row becomes the "above" row for the next one
            core::mem::swap(&mut prev, &mut cur);
            prev_valid = true;
        }
    }
}

/// Pre-SAO samples of the rows around the one being filtered.
struct EoLines<'a> {
    above: &'a [u8],
    cur:   &'a [u8],
    below: &'a [u8],
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
    out: &mut [u8], lines: &EoLines, x0: usize, x1: usize, y: usize, pw: usize, ph: usize,
    eo_class: u8, offsets: [i8; 4],
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

    // Pre-SAO sample at (x + dx, y + dy)
    let line = |dy: isize| match dy {
        -1 => lines.above,
        0 => lines.cur,
        _ => lines.below,
    };
    let (line1, line2) = (line(dy1), line(dy2));
    let fetch = |line: &[u8], x: usize, dx: isize| i32::from(line[(x as isize + dx) as usize]);

    for x in x_start..x_end {
        let c = i32::from(lines.cur[x]);
        let n1 = fetch(line1, x, dx1);
        let n2 = fetch(line2, x, dx2);
        let edge_idx = (2 + (c - n1).signum() + (c - n2).signum()) as usize;
        let offset = eo_offsets[edge_idx];
        if offset != 0 {
            out[x] = (c as i16 + offset).clamp(0, 255) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_match_whole_picture() {
        let (w, h, ctb) = (136_usize, 200_usize, 16_usize);
        let (wc, hc) = (w.div_ceil(ctb), h.div_ceil(ctb));
        let mut state = 3u32;
        let mut rand = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state >> 16
        };
        let infos: Vec<SaoInfo> = (0..wc * hc)
            .map(|i| {
                // leave some CTB rows without SAO to hit the skip path
                let skip = (i / wc) % 4 == 3;
                let t = |r: u32| if skip { 0 } else { (r % 3) as u8 };
                SaoInfo {
                    type_index: t(rand()) | t(rand()) << 2 | t(rand()) << 4,
                    sao_eo_class: (rand() & 0x3f) as u8,
                    sao_band_position: [(rand() % 32) as u8, (rand() % 32) as u8, (rand() % 32) as u8],
                    sao_offset_val: [[3, 1, -1, -3], [-2, 2, 4, -4], [5, -5, 1, -1]],
                }
            })
            .collect();

        let frame = || crate::hevc_decoder::deblocker::tests::blocky_frame(w, h, 9);
        let mut whole = frame();
        let before = whole.luma.pixels.clone();
        apply_sao_frame(&mut whole, w, h, ctb, &infos, 1);
        assert_ne!(before, whole.luma.pixels, "test picture should be changed by SAO");

        for threads in 2..=9 {
            let mut banded = frame();
            apply_sao_frame(&mut banded, w, h, ctb, &infos, threads);
            assert!(whole.luma.pixels == banded.luma.pixels, "luma, {threads} threads");
            assert!(whole.cb.pixels == banded.cb.pixels, "cb, {threads} threads");
            assert!(whole.cr.pixels == banded.cr.pixels, "cr, {threads} threads");
        }
    }
}
