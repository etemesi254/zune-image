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
