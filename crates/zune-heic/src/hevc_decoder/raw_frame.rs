use alloc::{string::ToString, vec::Vec};
use crate::utils::Lock;

use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Sps};
use crate::hevc_decoder::utils::ycbcr_to_rgb_inner_16_scalar;

pub struct SingleFrame {
    pub pixels:  Vec<u8>,
    pub width:   usize,
    pub height:  usize,
    pub stride:  usize,
    pub padding: usize
}
/// Gets the absolute 1D index for a logical x,y coordinate
#[inline(always)]
pub fn offset_plane(frame: &SingleFrame, x: usize, y: usize) -> usize {
    let physical_y = y + frame.padding;
    let physical_x = x + frame.padding;
    physical_y * frame.stride + physical_x
}

pub struct RawFrame {
    pub format: ChromaFormat,
    pub luma:   SingleFrame,
    pub cb:     SingleFrame,
    pub cr:     SingleFrame
}

impl RawFrame {
    pub fn new(width: usize, height: usize, format: ChromaFormat) -> Self {
        let (sub_x, sub_y) = format.get_subsampling();

        let padding = 32; // Standard padding for motion compensation filters

        // Helper to build a plane
        let make_plane = |w: usize, h: usize, is_active: bool| {
            let stride = w + (padding * 2);
            let buf_size = stride * (h + (padding * 2));
            let pixels = if is_active { vec![0u8; buf_size] } else { Vec::new() };

            SingleFrame {
                pixels,
                width: w,
                height: h,
                stride,
                padding
            }
        };

        Self {
            format,
            luma: make_plane(width, height, true),
            cb: make_plane(
                width / sub_x,
                height / sub_y,
                format != ChromaFormat::Monochrome
            ),
            cr: make_plane(
                width / sub_x,
                height / sub_y,
                format != ChromaFormat::Monochrome
            )
        }
    }
    pub fn from_sps(sps: &Sps) -> Self {
        let width = sps.pic_width_in_luma_samples as usize;
        let height = sps.pic_height_in_luma_samples as usize;

        Self::new(width, height, sps.chroma_format)
    }

    /// Bands covering the whole luma, cb and cr planes (one band each).
    pub fn whole(&mut self) -> [PlaneBand<'_>; 3] {
        [
            PlaneBand::whole(&mut self.luma),
            PlaneBand::whole(&mut self.cb),
            PlaneBand::whole(&mut self.cr)
        ]
    }
}

/// Plane geometry shared by the bands of one plane.
#[derive(Clone, Copy)]
pub struct PlaneGeometry {
    pub width:   usize,
    pub height:  usize,
    pub stride:  usize,
    pub padding: usize
}

impl PlaneGeometry {
    pub fn of(plane: &SingleFrame) -> Self {
        Self {
            width:   plane.width,
            height:  plane.height,
            stride:  plane.stride,
            padding: plane.padding
        }
    }

    /// Physical (padded) buffer row where logical row `y` lives
    #[inline(always)]
    pub fn physical_row(&self, y: usize) -> usize {
        y + self.padding
    }
}

/// Split a plane's pixel buffer into one band per CTU row.
///
/// Band `k` holds the physical rows of logical rows
/// `k * band_height .. (k + 1) * band_height`; the top padding rows belong to
/// the first band and the bottom padding rows to the last one. Returns the
/// bands and the physical row each starts at. An empty (inactive) plane gives
/// empty bands.
#[cfg(feature = "std")]
pub fn split_plane_rows(
    plane: &mut SingleFrame, band_height: usize, n_bands: usize
) -> Vec<(&mut [u8], usize)> {
    let geom = PlaneGeometry::of(plane);
    let mut rest: &mut [u8] = &mut plane.pixels;
    let mut bands = Vec::with_capacity(n_bands);
    if rest.is_empty() {
        for _ in 0..n_bands {
            bands.push((&mut [][..], 0));
        }
        return bands;
    }
    let mut row = 0;
    for k in 0..n_bands {
        let end_row = if k + 1 == n_bands {
            rest.len() / geom.stride + row
        } else {
            geom.physical_row(((k + 1) * band_height).min(geom.height))
        };
        let (band, tail) = rest.split_at_mut((end_row - row) * geom.stride);
        bands.push((band, row));
        rest = tail;
        row = end_row;
    }
    bands
}

/// The pixels of one band of rows of a plane (a CTU row, or the whole
/// plane), used while slices are being decoded.
///
/// All indices are *picture* (padded buffer) indices, as computed by
/// `offset_plane`-style arithmetic. Writes must fall inside the band; reads
/// may also hit the physical row directly above the band, which is served
/// from `above`, a copy kept up to date by the WPP row hand-off.
pub struct PlaneBand<'a> {
    band:        &'a mut [u8],
    /// picture index of `band[0]`
    start:       usize,
    /// copy of the physical row directly above the band (empty for the top band)
    pub above:   Vec<u8>,
    above_start: usize,
    pub width:   usize,
    pub height:  usize,
    pub stride:  usize,
    pub padding: usize
}

impl<'a> PlaneBand<'a> {
    pub fn whole(plane: &'a mut SingleFrame) -> Self {
        let geom = PlaneGeometry::of(plane);
        Self::new(&mut plane.pixels, 0, geom)
    }

    /// Band starting at physical row `first_row`; the row above it is
    /// unknown until `above` is filled.
    pub fn new(band: &'a mut [u8], first_row: usize, geom: PlaneGeometry) -> Self {
        let start = first_row * geom.stride;
        let above_len = if first_row > 0 { geom.stride } else { 0 };
        Self {
            band,
            start,
            above: vec![0; above_len],
            above_start: start - above_len,
            width: geom.width,
            height: geom.height,
            stride: geom.stride,
            padding: geom.padding
        }
    }

    #[inline(always)]
    fn local(&self, index: usize) -> usize {
        index
            .checked_sub(self.start)
            .expect("pixel outside this CTU row")
    }

    #[inline(always)]
    pub fn get(&self, index: usize) -> u8 {
        if index >= self.start {
            self.band[index - self.start]
        } else {
            index
                .checked_sub(self.above_start)
                .and_then(|i| self.above.get(i))
                .copied()
                .expect("pixel outside this CTU row and the row above it")
        }
    }

    /// Pixels `index..index + len` of the band itself
    #[inline(always)]
    pub fn slice(&self, index: usize, len: usize) -> &[u8] {
        let i = self.local(index);
        &self.band[i..i + len]
    }

    #[inline(always)]
    pub fn slice_mut(&mut self, index: usize, len: usize) -> &mut [u8] {
        let i = self.local(index);
        &mut self.band[i..i + len]
    }

    /// Copy `len` pixels from `src` to `dst` (ranges may overlap)
    #[inline(always)]
    pub fn copy_within(&mut self, src: usize, dst: usize, len: usize) {
        let (s, d) = (self.local(src), self.local(dst));
        self.band.copy_within(s..s + len, d);
    }
}

impl RawFrame {
    /// Converts planar YUV 4:2:0 to RGB and writes it into the provided slice.
    /// Expects `out_rgb` to have a length of at least `width * height * 3`.
    pub fn write_rgb_420(&self, out_rgb: &mut [u8]) -> Result<(), NalError> {
        let (width, height) = (self.luma.width, self.luma.height);
        let expected_len = width * height * 3;

        if out_rgb.len() < expected_len {
            return Err(NalError::Generic(format!(
                "Output buffer too small. Expected {}, got {}",
                expected_len,
                out_rgb.len()
            )));
        }
        let rows: Vec<Lock<&mut [u8]>> =
            out_rgb[..expected_len].chunks_mut(width * 3).map(Lock::new).collect();
        self.write_into_canvas(&rows, 0, 0, width, height, 3)
    }

    /// Converts this frame to interleaved pixels and writes it straight into
    /// a larger canvas at `(x_off, y_off)`.
    ///
    /// `canvas_rows` holds one entry per canvas row (`canvas_w * channels`
    /// bytes each), each behind its own lock so several tiles can be
    /// converted into the same canvas concurrently. Only the part of the
    /// frame that is visible inside the `canvas_w x canvas_h` canvas is
    /// converted; anything overhanging the right/bottom edge is skipped.
    ///
    /// `channels` may be 1 (luma only), 3 (RGB) or 4 (RGB + opaque alpha).
    pub(crate) fn write_into_canvas(
        &self, canvas_rows: &[Lock<&mut [u8]>], x_off: usize, y_off: usize, canvas_w: usize,
        canvas_h: usize, channels: usize
    ) -> Result<(), NalError> {
        if !matches!(channels, 1 | 3 | 4) {
            return Err(NalError::Generic(format!(
                "Unsupported number of output channels: {channels}"
            )));
        }
        let (luma, cb, cr) = (&self.luma, &self.cb, &self.cr);

        if x_off >= canvas_w || y_off >= canvas_h {
            // tile lies completely outside the visible canvas
            return Ok(());
        }
        let vis_w = luma.width.min(canvas_w - x_off);
        let vis_h = luma.height.min(canvas_h - y_off);

        for row in 0..vis_h {
            canvas_rows
                .get(y_off + row)
                .ok_or_else(|| NalError::Generic("Canvas row out of range".to_string()))?
                .with(|dst_row| {
                    let dst = dst_row
                        .get_mut(x_off * channels..(x_off + vis_w) * channels)
                        .ok_or_else(|| NalError::Generic("Canvas row too short".to_string()))?;
                    convert_row(self.format, luma, cb, cr, row, vis_w, dst, channels);
                    Ok::<(), NalError>(())
                })?;
        }
        Ok(())
    }
}

/// Convert the first `vis_w` pixels of row `row` into `dst`
/// (`vis_w * channels` bytes).
#[allow(clippy::too_many_arguments)]
fn convert_row(
    format: ChromaFormat, luma: &SingleFrame, cb: &SingleFrame, cr: &SingleFrame, row: usize,
    vis_w: usize, dst: &mut [u8], channels: usize
) {
    let y_row_base = (row + luma.padding) * luma.stride + luma.padding;

    if channels == 1 {
        dst[..vis_w].copy_from_slice(&luma.pixels[y_row_base..y_row_base + vis_w]);
        return;
    }

    let width = luma.width;
    let (sub_x, sub_y) = format.get_subsampling();
    let is_monochrome = cb.pixels.is_empty() || cr.pixels.is_empty();
    let c_row_base =
        if is_monochrome { 0 } else { (row / sub_y + cb.padding) * cb.stride + cb.padding };

    let mut cb_chunk = [128i16; 16];
    let mut cr_chunk = [128i16; 16];
    let mut temp = [0u8; 48];

    let full_chunks = vis_w / 16;
    let mut out_pos = 0usize;

    // full 16-pixel chunks plus one (clamped) partial chunk for the remainder
    let total_chunks = vis_w.div_ceil(16);
    for chunk in 0..total_chunks {
        let x_base = chunk * 16;
        let is_full = chunk < full_chunks;

        let y_chunk: [i16; 16] = if is_full {
            let y_src = &luma.pixels[y_row_base + x_base..][..16];
            core::array::from_fn(|i| i16::from(y_src[i]))
        } else {
            core::array::from_fn(|i| {
                let clamped_x = (x_base + i).min(width - 1);
                i16::from(luma.pixels[y_row_base + clamped_x])
            })
        };

        if !is_monochrome && is_full && sub_x == 2 {
            // 4:2:0 / 4:2:2: 8 chroma samples, each duplicated horizontally
            let cx_base = c_row_base + x_base / 2;
            let cb_src = &cb.pixels[cx_base..][..8];
            let cr_src = &cr.pixels[cx_base..][..8];
            for i in 0..8 {
                cb_chunk[2 * i] = i16::from(cb_src[i]);
                cb_chunk[2 * i + 1] = i16::from(cb_src[i]);
                cr_chunk[2 * i] = i16::from(cr_src[i]);
                cr_chunk[2 * i + 1] = i16::from(cr_src[i]);
            }
        } else if !is_monochrome {
            for i in 0..16 {
                let x_c = ((x_base + i) / sub_x).min(cb.width - 1);
                cb_chunk[i] = i16::from(cb.pixels[c_row_base + x_c]);
                cr_chunk[i] = i16::from(cr.pixels[c_row_base + x_c]);
            }
        }

        let n_px = if is_full { 16 } else { vis_w - x_base };

        if channels == 3 && is_full {
            // write straight into the destination
            ycbcr_to_rgb_inner_16_scalar::<false>(&y_chunk, &cb_chunk, &cr_chunk, dst, &mut out_pos);
            continue;
        }

        let mut temp_pos = 0usize;
        ycbcr_to_rgb_inner_16_scalar::<false>(&y_chunk, &cb_chunk, &cr_chunk, &mut temp, &mut temp_pos);

        if channels == 3 {
            dst[out_pos..out_pos + n_px * 3].copy_from_slice(&temp[..n_px * 3]);
            out_pos += n_px * 3;
        } else {
            // RGBA: alpha is not decoded yet, emit opaque pixels
            for (px, rgb) in dst[out_pos..out_pos + n_px * 4]
                .chunks_exact_mut(4)
                .zip(temp.chunks_exact(3))
            {
                px[..3].copy_from_slice(rgb);
                px[3] = 255;
            }
            out_pos += n_px * 4;
        }
    }
}

#[cfg(feature = "std")]
impl RawFrame {
    /// Dumps the reconstructed frame to a P6 PPM file.
    /// This automatically strips HEVC padding and converts YCbCr to RGB.
    pub fn dump_ppm(&self, filename: &str) -> std::io::Result<()> {
        let width = self.luma.width;
        let height = self.luma.height;

        let mut rgb_buf = vec![0u8; width * height * 3];

        // Ignore the Error string for simplicity, or map it to io::Error
        self.write_rgb_420(&mut rgb_buf)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);
        use std::io::Write;

        writeln!(writer, "P6\n{width} {height}\n255")?;
        writer.write_all(&rgb_buf)?;
        writer.flush()?;

        Ok(())
    }
}
