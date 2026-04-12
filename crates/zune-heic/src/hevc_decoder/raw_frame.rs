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
        let luma = self.luma.lock().unwrap();
        let cb = self.cb.lock().unwrap();
        let cr = self.cr.lock().unwrap();

        let width = luma.width;
        let height = luma.height;
        let expected_len = width * height * 3;

        if out_rgb.len() < expected_len {
            return Err(NalError::Generic(format!(
                "Output buffer too small. Expected {}, got {}",
                expected_len,
                out_rgb.len()
            )));
        }

        let (sub_x, sub_y) = self.format.get_subsampling();
        let is_monochrome = cb.pixels.is_empty() || cr.pixels.is_empty();

        let chunks_of_16 = width / 16;
        let remainder = width % 16;

        let mut out_pos = 0usize;
        let mut cb_chunk = [0i16; 16];
        let mut cr_chunk = [0i16; 16];

        for row in 0..height {
            // Account for top and left padding in the stride
            let y_row_base = (row + luma.padding) * luma.stride + luma.padding;

            let c_row_base = if is_monochrome {
                0
            } else {
                (row / sub_y + cb.padding) * cb.stride + cb.padding
            };

            for chunk in 0..chunks_of_16 {
                let x_base = chunk * 16;

                // 1. Y: One contiguous slice read
                let y_src = &luma.pixels[y_row_base + x_base..][..16];
                let y_chunk: [i16; 16] = std::array::from_fn(|i| i16::from(y_src[i]));

                // 2. UV: Planar read, duplicate values for 4:2:0
                if is_monochrome {
                    cb_chunk.fill(128);
                    cr_chunk.fill(128);
                } else {
                    let cx_base = x_base / sub_x;
                    let cb_src = &cb.pixels[c_row_base + cx_base..][..8];
                    let cr_src = &cr.pixels[c_row_base + cx_base..][..8];

                    for i in 0..8 {
                        let cb_val = i16::from(cb_src[i]);
                        let cr_val = i16::from(cr_src[i]);
                        // Duplicate horizontally to match 16 Y pixels
                        cb_chunk[i * 2] = cb_val;
                        cb_chunk[i * 2 + 1] = cb_val;
                        cr_chunk[i * 2] = cr_val;
                        cr_chunk[i * 2 + 1] = cr_val;
                    }
                }

                // 3. Process chunk
                ycbcr_to_rgb_inner_16_scalar::<false>(
                    &y_chunk,
                    &cb_chunk,
                    &cr_chunk,
                    out_rgb,
                    &mut out_pos
                );
            }

            // Remainder: clamp to avoid reading padding bytes as valid pixel data
            if remainder > 0 {
                let x_base = chunks_of_16 * 16;

                let y_chunk: [i16; 16] = std::array::from_fn(|i| {
                    let clamped_x = (x_base + i).min(width - 1);
                    i16::from(luma.pixels[y_row_base + clamped_x])
                });

                if is_monochrome {
                    cb_chunk.fill(128);
                    cr_chunk.fill(128);
                } else {
                    for i in 0..8 {
                        // Carefully calculate and clamp the chroma index
                        let x_c = ((x_base + i * 2) / sub_x).min(cb.width - 1);
                        let cb_val = i16::from(cb.pixels[c_row_base + x_c]);
                        let cr_val = i16::from(cr.pixels[c_row_base + x_c]);

                        cb_chunk[i * 2] = cb_val;
                        cb_chunk[i * 2 + 1] = cb_val;
                        cr_chunk[i * 2] = cr_val;
                        cr_chunk[i * 2 + 1] = cr_val;
                    }
                }

                let mut temp = [0u8; 48]; // 16 pixels * 3 channels
                let mut temp_pos = 0usize;

                ycbcr_to_rgb_inner_16_scalar::<false>(
                    &y_chunk,
                    &cb_chunk,
                    &cr_chunk,
                    &mut temp,
                    &mut temp_pos
                );

                let valid_bytes = remainder * 3;
                out_rgb[out_pos..out_pos + valid_bytes].copy_from_slice(&temp[..valid_bytes]);
                out_pos += valid_bytes;
            }
        }

        Ok(())
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
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let file = std::fs::File::create(filename)?;
        let mut writer = std::io::BufWriter::new(file);

        writeln!(writer, "P6\n{width} {height}\n255")?;
        writer.write_all(&rgb_buf)?;
        writer.flush()?;

        Ok(())
    }
}
