use std::io::Write;
use std::sync::{Arc, Mutex};

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
    // Each plane is protected by a Mutex for internal mutability across threads
    pub luma:   Mutex<SingleFrame>,
    pub cb:     Mutex<SingleFrame>,
    pub cr:     Mutex<SingleFrame>
}

impl RawFrame {
    pub fn new(width: usize, height: usize, format: ChromaFormat) -> Arc<Self> {
        let (sub_x, sub_y) = format.get_subsampling();

        let padding = 32; // Standard padding for motion compensation filters

        // Helper to build a plane
        let make_plane = |w: usize, h: usize, is_active: bool| {
            let stride = w + (padding * 2);
            let buf_size = stride * (h + (padding * 2));
            let pixels = if is_active { vec![0u8; buf_size] } else { Vec::new() };

            Mutex::new(SingleFrame {
                pixels,
                width: w,
                height: h,
                stride,
                padding
            })
        };

        Arc::new(Self {
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
        })
    }
    pub fn from_sps(sps: &Sps) -> Arc<Self> {
        let width = sps.pic_width_in_luma_samples as usize;
        let height = sps.pic_height_in_luma_samples as usize;

        Self::new(width, height, sps.chroma_format)
    }
}
impl RawFrame {
    /// Converts planar YUV 4:2:0 to RGB and writes it into the provided slice.
    /// Expects `out_rgb` to have a length of at least `width * height * 3`.
    pub fn write_rgb_420(&self, out_rgb: &mut [u8]) -> Result<(), NalError> {
        let (width, height) = {
            let luma = self.luma.lock().unwrap();
            (luma.width, luma.height)
        };
        let expected_len = width * height * 3;

        if out_rgb.len() < expected_len {
            return Err(NalError::Generic(format!(
                "Output buffer too small. Expected {}, got {}",
                expected_len,
                out_rgb.len()
            )));
        }
        let rows: Vec<Mutex<&mut [u8]>> =
            out_rgb[..expected_len].chunks_mut(width * 3).map(Mutex::new).collect();
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
    pub fn write_into_canvas(
        &self, canvas_rows: &[Mutex<&mut [u8]>], x_off: usize, y_off: usize, canvas_w: usize,
        canvas_h: usize, channels: usize
    ) -> Result<(), NalError> {
        if !matches!(channels, 1 | 3 | 4) {
            return Err(NalError::Generic(format!(
                "Unsupported number of output channels: {channels}"
            )));
        }
        let luma = self.luma.lock().unwrap();
        let cb = self.cb.lock().unwrap();
        let cr = self.cr.lock().unwrap();

        if x_off >= canvas_w || y_off >= canvas_h {
            // tile lies completely outside the visible canvas
            return Ok(());
        }
        let vis_w = luma.width.min(canvas_w - x_off);
        let vis_h = luma.height.min(canvas_h - y_off);

        for row in 0..vis_h {
            let mut dst_row = canvas_rows
                .get(y_off + row)
                .ok_or_else(|| NalError::Generic("Canvas row out of range".to_string()))?
                .lock()
                .unwrap();
            let dst = dst_row
                .get_mut(x_off * channels..(x_off + vis_w) * channels)
                .ok_or_else(|| NalError::Generic("Canvas row too short".to_string()))?;

            convert_row(self.format, &luma, &cb, &cr, row, vis_w, dst, channels);
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
            std::array::from_fn(|i| i16::from(y_src[i]))
        } else {
            std::array::from_fn(|i| {
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

impl RawFrame {
    /// Dumps the reconstructed frame to a P6 PPM file.
    /// This automatically strips HEVC padding and converts YCbCr to RGB.
    pub fn dump_ppm(&self, filename: &str) -> std::io::Result<()> {
        let width = self.luma.lock().unwrap().width;
        let height = self.luma.lock().unwrap().height;

        let mut rgb_buf = vec![0u8; width * height * 3];

        // Ignore the Error string for simplicity, or map it to io::Error
        self.write_rgb_420(&mut rgb_buf)
            .map_err(|e| std::io::Error::other(e.to_string()))?;

        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);

        writeln!(writer, "P6\n{width} {height}\n255")?;
        writer.write_all(&rgb_buf)?;
        writer.flush()?;

        Ok(())
    }
}
