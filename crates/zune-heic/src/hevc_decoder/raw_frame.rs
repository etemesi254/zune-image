use alloc::{string::ToString, vec::Vec};
use core::ops::Range;
use crate::utils::Lock;

use crate::hevc_decoder::nal_parser::NalError;
use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Sps};
use crate::colour::{ColourInfo, Conversion};

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

        // Still images are intra only: nothing reads outside the picture, so
        // the planes need no border (inter prediction would want ~80 px).
        let padding = 0;

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

/// A horizontal band of a finished plane, handed to one thread by
/// [`for_each_band`] (deblocking and SAO).
pub struct RowBand<'a> {
    pub pixels: &'a mut [u8],
    /// plane index of `pixels[0]`
    start:      usize,
    /// which band this is (position in the `cuts` list)
    pub index:  usize,
    /// logical rows owned by this band
    pub rows:   Range<usize>,
    pub stride: usize,
    padding:    usize
}

impl RowBand<'_> {
    /// Index into `pixels` of logical sample (x, y); `y` must be in `rows`.
    #[inline(always)]
    pub fn index(&self, x: usize, y: usize) -> usize {
        (y + self.padding) * self.stride + x + self.padding - self.start
    }
}

/// Row boundaries splitting `height` rows into about `n` bands:
/// `[0, c1, .., height]`, every inner cut `≡ offset (mod align)`.
pub fn band_cuts(height: usize, n: usize, align: usize, offset: usize) -> Vec<usize> {
    let mut cuts = vec![0];
    let per = height / n.max(1);
    for k in 1..n {
        let cut = (k * per) / align * align + offset;
        if cut > *cuts.last().unwrap() && cut < height {
            cuts.push(cut);
        }
    }
    cuts.push(height);
    cuts
}

/// Run `f` on the bands of `plane` given by `cuts` (from [`band_cuts`]),
/// in parallel when there is more than one band. Each band owns its rows
/// exclusively; the top/bottom padding rows go to the first/last band.
pub fn for_each_band<F>(plane: &mut SingleFrame, cuts: &[usize], f: F)
where
    F: Fn(&mut RowBand) + Sync
{
    if plane.pixels.is_empty() {
        return;
    }
    let geom = PlaneGeometry::of(plane);
    let total = plane.pixels.len();
    let mut rest: &mut [u8] = &mut plane.pixels;
    let mut start = 0;
    let mut bands = Vec::with_capacity(cuts.len() - 1);
    for (index, rows) in cuts.windows(2).enumerate() {
        let end = if index + 2 == cuts.len() {
            total
        } else {
            geom.physical_row(rows[1]) * geom.stride
        };
        let (band, tail) = rest.split_at_mut(end - start);
        bands.push(RowBand {
            pixels: band,
            start,
            index,
            rows: rows[0]..rows[1],
            stride: geom.stride,
            padding: geom.padding
        });
        rest = tail;
        start = end;
    }

    #[cfg(feature = "std")]
    if bands.len() > 1 {
        let f = &f;
        std::thread::scope(|s| {
            let mut bands = bands.into_iter();
            let mut first = bands.next().unwrap();
            for mut band in bands {
                s.spawn(move || f(&mut band));
            }
            f(&mut first);
        });
        return;
    }
    for mut band in bands {
        f(&mut band);
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

    /// Copy `dst.len()` pixels starting at picture index `index` into `dst`.
    /// The run must lie entirely inside the band or entirely inside the row
    /// above it.
    #[inline(always)]
    pub fn copy_to(&self, index: usize, dst: &mut [u8]) {
        let src = if index >= self.start {
            &self.band[index - self.start..]
        } else {
            index
                .checked_sub(self.above_start)
                .and_then(|i| self.above.get(i..))
                .expect("pixel outside this CTU row and the row above it")
        };
        dst.copy_from_slice(&src[..dst.len()]);
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
    /// The decoded planes (Y, then Cb and Cr if present), each cropped to the
    /// conformance `window` (`(left, right, top, bottom)` in luma samples),
    /// concatenated: the layout reference decoders such as `dec265 -o` write.
    #[cfg(feature = "dump-tiles")]
    pub(crate) fn planar_yuv(&self, window: (usize, usize, usize, usize)) -> Vec<u8> {
        let (left, right, top, bottom) = window;
        let (sx, sy) = self.format.get_subsampling();
        let mut out = Vec::new();
        for (plane, (sub_x, sub_y)) in [(&self.luma, (1, 1)), (&self.cb, (sx, sy)), (&self.cr, (sx, sy))] {
            if plane.pixels.is_empty() {
                continue;
            }
            let (x0, x1) = (left / sub_x, plane.width.saturating_sub(right / sub_x));
            let (y0, y1) = (top / sub_y, plane.height.saturating_sub(bottom / sub_y));
            for y in y0..y1 {
                let row = offset_plane(plane, 0, y);
                out.extend_from_slice(&plane.pixels[row + x0..row + x1]);
            }
        }
        out
    }

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
        // full-range Rec. 601, for debugging output
        let conv = Conversion::new(ColourInfo {
            matrix_coefficients: 2,
            colour_primaries:    2,
            full_range:          true
        });
        self.write_into_canvas(&rows, 0, 0, width, height, 3, &conv)
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
    /// `channels` may be 1 (luma only), 3 (RGB) or 4 (RGB + opaque alpha);
    /// `conv` is the YCbCr → RGB conversion for this frame.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_into_canvas(
        &self, canvas_rows: &[Lock<&mut [u8]>], x_off: usize, y_off: usize, canvas_w: usize,
        canvas_h: usize, channels: usize, conv: &Conversion
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
                    convert_row(self.format, luma, cb, cr, row, vis_w, dst, channels, conv);
                    Ok::<(), NalError>(())
                })?;
        }
        Ok(())
    }
}

/// Convert the first `vis_w` pixels of row `row` into `dst`
/// (`vis_w * channels` bytes). Chroma is upsampled nearest-neighbour.
#[allow(clippy::too_many_arguments)]
fn convert_row(
    format: ChromaFormat, luma: &SingleFrame, cb: &SingleFrame, cr: &SingleFrame, row: usize,
    vis_w: usize, dst: &mut [u8], channels: usize, conv: &Conversion
) {
    let y_row_base = (row + luma.padding) * luma.stride + luma.padding;
    let y_row = &luma.pixels[y_row_base..y_row_base + vis_w];
    let dst = &mut dst[..vis_w * channels];

    if channels == 1 {
        dst.copy_from_slice(y_row);
        return;
    }

    if cb.pixels.is_empty() || cr.pixels.is_empty() {
        // monochrome: R = G = B = Y
        for (px, &y) in dst.chunks_exact_mut(channels).zip(y_row) {
            px[..3].fill(y);
            if channels == 4 {
                px[3] = 255;
            }
        }
        return;
    }

    let (sub_x, sub_y) = format.get_subsampling();
    let c_row_base = (row / sub_y + cb.padding) * cb.stride + cb.padding;
    let cb_row = &cb.pixels[c_row_base..c_row_base + cb.width];
    let cr_row = &cr.pixels[c_row_base..c_row_base + cr.width];
    // alpha is not decoded yet: RGBA output gets opaque pixels
    conv.convert_row(y_row, cb_row, cr_row, sub_x, dst, channels);
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
