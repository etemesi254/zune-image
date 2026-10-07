//! Decoding the CTU rows of a slice in parallel (wavefront parallel
//! processing, WPP). Only available with the `std` feature.

use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::hevc_decoder::band::Band;
use crate::hevc_decoder::cabac::{CabacDecoder, NUM_CABAC_CONTEXTS};
use crate::hevc_decoder::ctx::DecodeSliceContext;
use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Pps, SliceHeader, Sps};
use crate::hevc_decoder::neighbor_tracker::{BandBlocks, BlockMap, BlockState, NeighborTracker};
use crate::hevc_decoder::quadtree::coding_unit::read_coding_tree_unit;
use crate::hevc_decoder::quadtree::sao::SaoInfo;
use crate::hevc_decoder::quadtree::{CtuStatus, finish_ctu, load_wpp_contexts};
use crate::hevc_decoder::raw_frame::{PlaneBand, PlaneGeometry, RawFrame, split_plane_rows};

/// Start offsets (into the emulation-prevention-free slice data) of every WPP
/// substream, or `None` if the entry points are missing or inconsistent.
///
/// `entry_point_offset`s count bytes of the NAL unit *including* emulation
/// prevention bytes (spec 7.4.7.1), while decoding works on the cleaned
/// buffer, so offsets are mapped through the removed byte positions.
pub(super) fn wpp_substream_starts(
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

/// One CTU row's exclusive share of the picture state.
struct RowBands<'a> {
    planes: [PlaneBand<'a>; 3],
    blocks: BandBlocks<'a>,
    sao:    Band<'a, SaoInfo>
}

/// The bottom edge of a CTU row, published CTU by CTU for the row below:
/// the last pixel row of each plane, the last row of neighbour units and the
/// SAO parameters (for SAO merge-up).
struct Edge {
    pixels:   [Vec<u8>; 3],
    units:    Vec<BlockState>,
    sao:      Vec<SaoInfo>,
    /// CABAC contexts after CTU 1 of the row, to start the row below
    contexts: Option<[u8; NUM_CABAC_CONTEXTS]>
}

/// CTU size of each plane: `(width, height)` in samples
fn plane_ctb_sizes(sps: &Sps, format: ChromaFormat) -> [(usize, usize); 3] {
    let ctb = 1usize << sps.log2_ctb_size_y;
    let (sub_x, sub_y) = format.get_subsampling();
    [(ctb, ctb), (ctb / sub_x, ctb / sub_y), (ctb / sub_x, ctb / sub_y)]
}

/// Everything needed to decode the CTU rows of one slice with WPP threads.
pub(super) struct WppRows<'a> {
    pub(super) sps:          &'a Sps,
    pub(super) pps:          &'a Pps,
    pub(super) slice_header: &'a SliceHeader,
    /// cleaned slice NAL payload
    pub(super) data:         &'a [u8],
    /// start of each row's substream in `data`
    pub(super) starts:       &'a [usize],
    /// picture CTU row of the first substream
    pub(super) first_row:    usize,
    pub(super) slice_qp:     i8,
    pub(super) init_type:    usize
}

impl WppRows<'_> {
    /// Decode all rows on `threads` scoped threads.
    ///
    /// The frame, neighbour map and SAO buffer are split into one band per CTU
    /// row, so each row decoder has plain `&mut` access to its own rows. The
    /// only thing a row needs from the row above is its bottom edge, which is
    /// handed over through a `Mutex<Edge>` as each CTU of the row above is
    /// finished. Rows are handed out in order, and row `r` decodes CTU `x`
    /// only after row `r - 1` has finished CTU `x + 1` (its above-right
    /// neighbour).
    pub(super) fn decode(
        &self, frame: &mut RawFrame, block_map: &mut BlockMap, sao_buffer: &mut [SaoInfo],
        threads: usize
    ) -> Result<(), NalError> {
        let width_ctus = self.sps.pic_width_in_ctbs_y as usize;
        let height_ctus = self.sps.pic_height_in_ctbs_y as usize;
        let ctb_sizes = plane_ctb_sizes(self.sps, frame.format);

        // --- split everything into per-row bands ---
        let geoms = [&frame.luma, &frame.cb, &frame.cr].map(PlaneGeometry::of);
        let strides = [&frame.luma, &frame.cb, &frame.cr]
            .map(|p| if p.pixels.is_empty() { 0 } else { p.stride });
        let mut plane_bands = [
            split_plane_rows(&mut frame.luma, ctb_sizes[0].1, height_ctus),
            split_plane_rows(&mut frame.cb, ctb_sizes[1].1, height_ctus),
            split_plane_rows(&mut frame.cr, ctb_sizes[2].1, height_ctus)
        ]
        .map(IntoIterator::into_iter);

        let units_w = block_map.width_in_units;
        let units_per_band = (ctb_sizes[0].1 >> block_map.log2_unit_size) * units_w;
        let mut unit_bands = block_map.blocks.chunks_mut(units_per_band);
        let mut sao_bands = sao_buffer.chunks_mut(width_ctus);

        let mut bands = Vec::with_capacity(height_ctus);
        for y in 0..height_ctus {
            let planes = [0, 1, 2].map(|c| {
                let (band, first_row) = plane_bands[c].next().unwrap();
                PlaneBand::new(band, first_row, geoms[c])
            });
            let above_units = if y > 0 { units_w } else { 0 };
            let above_sao = if y > 0 { width_ctus } else { 0 };
            bands.push(Mutex::new(Some(RowBands {
                planes,
                blocks: Band::new(unit_bands.next().unwrap(), y * units_per_band, above_units),
                sao: Band::new(sao_bands.next().unwrap(), y * width_ctus, above_sao)
            })));
        }

        let edges: Vec<Mutex<Edge>> = (0..height_ctus)
            .map(|_| {
                Mutex::new(Edge {
                    pixels: strides.map(|s| vec![0u8; s]),
                    units:  vec![BlockState::default(); units_w],
                    sao:    vec![SaoInfo::default(); width_ctus],
                    contexts: None
                })
            })
            .collect();

        // CTUs finished per picture row (usize::MAX once the row is done).
        // Rows above this slice are already decoded: publish their bottom edge.
        let progress: Vec<AtomicUsize> = (0..height_ctus).map(|_| AtomicUsize::new(0)).collect();
        if self.first_row > 0 {
            let y = self.first_row - 1;
            let mut guard = bands[y].lock().unwrap();
            let above = guard.as_mut().unwrap();
            let mut edge = edges[y].lock().unwrap();
            for x in 0..width_ctus {
                self.publish_edge(&mut edge, &above.planes, &above.blocks, &above.sao, y, x);
            }
            progress[y].store(usize::MAX, Ordering::Release);
        }

        let n_rows = self.starts.len();
        let next_row = AtomicUsize::new(0);
        let abort = AtomicBool::new(false);
        let first_error: Mutex<Option<NalError>> = Mutex::new(None);
        let tracker_dims = (block_map.width_in_units, block_map.height_in_units, block_map.log2_unit_size);

        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        let k = next_row.fetch_add(1, Ordering::Relaxed);
                        if k >= n_rows {
                            break;
                        }
                        let y = self.first_row + k;
                        let row = bands[y].lock().unwrap().take();
                        if !abort.load(Ordering::Relaxed)
                            && let Some(row) = row
                            && let Err(e) =
                                self.decode_row(k, row, tracker_dims, &edges, &progress, &abort)
                        {
                            abort.store(true, Ordering::Relaxed);
                            first_error.lock().unwrap().get_or_insert(e);
                        }
                        // wake up the row below, whatever happened
                        progress[y].store(usize::MAX, Ordering::Release);
                    }
                });
            }
        });

        match first_error.into_inner().unwrap() {
            Some(e) => Err(e),
            None => Ok(())
        }
    }

    /// Copy the bottom edge of CTU `x` of picture row `y` into `edge`.
    fn publish_edge(
        &self, edge: &mut Edge, planes: &[PlaneBand; 3], blocks: &BandBlocks, sao: &Band<SaoInfo>,
        y: usize, x: usize
    ) {
        let width_ctus = self.sps.pic_width_in_ctbs_y as usize;
        let ctb_sizes = plane_ctb_sizes(self.sps, self.sps.chroma_format);

        for (c, plane) in planes.iter().enumerate() {
            if edge.pixels[c].is_empty() {
                continue;
            }
            let (cw, ch) = ctb_sizes[c];
            let last_row = ((y + 1) * ch).min(plane.height) - 1;
            let x0 = x * cw;
            let x1 = (x0 + cw).min(plane.width);
            let row_start = (last_row + plane.padding) * plane.stride + plane.padding;
            edge.pixels[c][plane.padding + x0..plane.padding + x1]
                .copy_from_slice(plane.slice(row_start + x0, x1 - x0));
        }

        let units_w = edge.units.len();
        let n_unit_rows = blocks.own().len() / units_w;
        let last = &blocks.own()[(n_unit_rows - 1) * units_w..];
        let cu = ctb_sizes[0].0 >> 2;
        let (u0, u1) = (x * cu, ((x + 1) * cu).min(units_w));
        edge.units[u0..u1].copy_from_slice(&last[u0..u1]);

        edge.sao[x] = sao[y * width_ctus + x];
    }

    /// Copy the bottom edge of CTU `x` of the row above (from `edge`) into
    /// this row's `above` buffers.
    fn take_edge(&self, edge: &Edge, ctx: &mut DecodeSliceContext, x: usize) {
        let ctb_sizes = plane_ctb_sizes(self.sps, self.sps.chroma_format);
        for (c, plane) in ctx.planes.iter_mut().enumerate() {
            if edge.pixels[c].is_empty() {
                continue;
            }
            let cw = ctb_sizes[c].0;
            let (x0, x1) = (x * cw, ((x + 1) * cw).min(plane.width));
            let p = plane.padding;
            plane.above[p + x0..p + x1].copy_from_slice(&edge.pixels[c][p + x0..p + x1]);
        }
        let units_w = edge.units.len();
        let cu = ctb_sizes[0].0 >> 2;
        let (u0, u1) = (x * cu, ((x + 1) * cu).min(units_w));
        ctx.neighbor_tracker.blocks.above[u0..u1].copy_from_slice(&edge.units[u0..u1]);
        ctx.ctb_sao_buffer.above[x] = edge.sao[x];
    }

    fn decode_row(
        &self, k: usize, row: RowBands, tracker_dims: (usize, usize, u8), edges: &[Mutex<Edge>],
        progress: &[AtomicUsize], abort: &AtomicBool
    ) -> Result<(), NalError> {
        let width = self.sps.pic_width_in_ctbs_y as usize;
        let ctu_y = self.first_row + k;

        // Wait until the row above has finished `needed` CTUs.
        let wait_above = |needed: usize| -> Result<(), NalError> {
            if ctu_y == 0 {
                return Ok(());
            }
            while progress[ctu_y - 1].load(Ordering::Acquire) < needed {
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
        let (width_in_units, height_in_units, log2_unit_size) = tracker_dims;
        let mut ctx = DecodeSliceContext::new(
            self.sps,
            self.pps,
            self.slice_header,
            cabac,
            NeighborTracker {
                blocks: row.blocks,
                width_in_units,
                height_in_units,
                log2_unit_size
            },
            self.slice_qp,
            row.planes,
            row.sao
        );

        if k > 0 {
            // contexts are saved after CTU 1 of the row above
            wait_above(2.min(width))?;
            ctx.wpp_saved_contexts = edges[ctu_y - 1].lock().unwrap().contexts.take();
            load_wpp_contexts(&mut ctx, ctu_y, width, self.slice_qp, self.init_type)?;
        }

        // CTUs of the row above whose bottom edge has been copied
        let mut copied = 0;
        for ctu_x in 0..width {
            let needed = (ctu_x + 2).min(width);
            if ctu_y > 0 && copied < needed {
                wait_above(needed)?;
                let edge = edges[ctu_y - 1].lock().unwrap();
                for x in copied..needed {
                    self.take_edge(&edge, &mut ctx, x);
                }
                copied = needed;
            }

            read_coding_tree_unit(&mut ctx, ctu_x, ctu_y)?;
            let status = finish_ctu(&mut ctx, ctu_x, ctu_y)?;

            {
                let mut edge = edges[ctu_y].lock().unwrap();
                if let Some(contexts) = ctx.wpp_saved_contexts.take() {
                    edge.contexts = Some(contexts);
                }
                self.publish_edge(
                    &mut edge,
                    &ctx.planes,
                    &ctx.neighbor_tracker.blocks,
                    &ctx.ctb_sao_buffer,
                    ctu_y,
                    ctu_x
                );
            }
            progress[ctu_y].store(ctu_x + 1, Ordering::Release);

            match status {
                CtuStatus::Continue => {}
                CtuStatus::EndOfSubstream | CtuStatus::EndOfSliceSegment => return Ok(())
            }
        }
        Err(NalError::GenericStr("WPP: row ended without end_of_subset_one_bit"))
    }
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
