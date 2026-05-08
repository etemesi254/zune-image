/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Scanline-by-scanline output API, modelled on libjpeg-turbo's
//! `jpeg_start_decompress` / `jpeg_read_scanlines` / `jpeg_finish_decompress`.

use alloc::vec::Vec;

/// Per-call state for the scanline output API.
///
/// Stored on `JpegDecoder` as `Option<ScanlineState>` and populated
/// by `start_decompress`. Cleared by `finish_decompress`.
pub(crate) struct ScanlineState {
    /// Index of the next scanline that will be returned to the caller
    /// (mirrors libjpeg's `cinfo.output_scanline`).
    pub(crate) next_scanline: usize,

    /// Total scanlines this image will produce.
    pub(crate) out_height: usize,

    /// Bytes per output scanline = `output_width * output_components`.
    pub(crate) row_stride: usize,

    /// The fully-decoded image. Scanlines are sliced out of this on each
    /// `read_scanlines` call.
    pub(crate) pre_decoded: Vec<u8>
}
