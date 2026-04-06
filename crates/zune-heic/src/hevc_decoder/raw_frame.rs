use std::sync::{Arc, Mutex};

use crate::hevc_decoder::nal_unit_headers::{ChromaFormat, Sps};

pub struct SingleFrame {
    pub pixels:  Vec<u8>,
    pub width:   usize,
    pub height:  usize,
    pub stride:  usize,
    pub padding: usize
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

use std::fs::File;
use std::io::{BufWriter, Write};

impl RawFrame {
    /// Dumps the reconstructed frame to a P6 PPM file.
    /// This automatically strips HEVC padding and converts YCbCr to RGB.
    pub fn dump_ppm(&self, filename: &str) -> std::io::Result<()> {
        // Safely lock all three color planes
        let luma = self.luma.lock().unwrap();
        let cb = self.cb.lock().unwrap();
        let cr = self.cr.lock().unwrap();

        let width = luma.width;
        let height = luma.height;

        // Open the file with a BufWriter for maximum write speed
        let file = File::create(filename)?;
        let mut writer = BufWriter::new(file);

        // Write the PPM P6 Header
        // P6 = Binary RGB, followed by Width, Height, and Max Color Value (255)
        writeln!(writer, "P6\n{width} {height}\n255")?;

        let (sub_x, sub_y) = self.format.get_subsampling();
        let is_monochrome = cb.pixels.is_empty() || cr.pixels.is_empty();

        // Pre-allocate the RGB buffer
        let mut rgb_buf = vec![0u8; width * height * 3];
        let mut out_idx = 0;

        for y in 0..height {
            // Luma row offset (skipping top padding, moving to current row, skipping left padding)
            let y_row_offset = (y + luma.padding) * luma.stride + luma.padding;

            // Chroma row offset (scaled by subsampling)
            let c_row_offset = if is_monochrome {
                0
            } else {
                (y / sub_y + cb.padding) * cb.stride + cb.padding
            };

            for x in 0..width {
                // 1. Fetch Y
                let y_val = i32::from(luma.pixels[y_row_offset + x]);

                // 2. Fetch Cb and Cr (handling subsampling mapping)
                let (cb_val, cr_val) = if is_monochrome {
                    (128, 128) // Default chroma for monochrome
                } else {
                    let cx = x / sub_x;
                    // Because Cb and Cr were created identically, they share the same stride/padding
                    (
                        i32::from(cb.pixels[c_row_offset + cx]),
                        i32::from(cr.pixels[c_row_offset + cx]),
                    )
                };

                // 3. YCbCr to RGB Conversion (Fast Integer Approximation)
                // Center Chroma around 0
                let u = cb_val - 128;
                let v = cr_val - 128;

                // Full-Range BT.601 -> RGB
                let r = y_val + ((v * 359 + 128) >> 8);
                let g = y_val - ((u * 88 + v * 183 + 128) >> 8);
                let b = y_val + ((u * 454 + 128) >> 8);

                // 4. Clamp and Write to Buffer
                rgb_buf[out_idx]     = r.clamp(0, 255) as u8;
                rgb_buf[out_idx + 1] = g.clamp(0, 255) as u8;
                rgb_buf[out_idx + 2] = b.clamp(0, 255) as u8;

                out_idx += 3;
            }
        }

        // Blast the RGB buffer to the file
        writer.write_all(&rgb_buf)?;
        writer.flush()?;

        Ok(())
    }
}