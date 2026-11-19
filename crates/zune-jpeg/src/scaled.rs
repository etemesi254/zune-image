/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Reduced-IDCT decoding with coded and decoded geometry kept separate.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use zune_core::bytestream::ZByteReaderTrait;

use crate::decoder::{IDCTPtr, McuDecodeOutput, MAX_COMPONENTS};
use crate::errors::DecodeErrors;
use crate::idct::scalar::{
    idct_scaled_10x10, idct_scaled_12x12, idct_scaled_14x14, idct_scaled_1x1, idct_scaled_2x2,
    idct_scaled_3x3, idct_scaled_4x4, idct_scaled_5x5, idct_scaled_6x6, idct_scaled_7x7
};
use crate::worker::color_convert;
use crate::JpegDecoder;

/// Native reduced-IDCT output ratios. Dimensions are rounded up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JpegScale {
    /// Decode at one half of the coded width and height.
    Half,
    /// Decode at one quarter of the coded width and height.
    Quarter,
    /// Decode at one eighth of the coded width and height.
    Eighth,
    /// Decode at three eighths of the coded width and height.
    ThreeEighths,
    /// Decode at five eighths of the coded width and height.
    FiveEighths,
    /// Decode at three quarters of the coded width and height.
    ThreeQuarters,
    /// Decode at seven eighths of the coded width and height.
    SevenEighths
}

impl JpegScale {
    /// The numerator of the reduced output ratio.
    #[must_use]
    pub const fn numerator(self) -> usize {
        match self {
            Self::Half | Self::Quarter | Self::Eighth => 1,
            Self::ThreeEighths | Self::ThreeQuarters => 3,
            Self::FiveEighths => 5,
            Self::SevenEighths => 7
        }
    }

    /// The denominator of the reduced output ratio (2, 4, or 8).
    #[must_use]
    pub const fn denominator(self) -> usize {
        match self {
            Self::Half => 2,
            Self::Quarter | Self::ThreeQuarters => 4,
            Self::Eighth | Self::ThreeEighths | Self::FiveEighths | Self::SevenEighths => 8
        }
    }
}

pub(crate) struct ScaledDecodeState {
    scale:    JpegScale,
    output:   ScaledOutput,
    complete: bool
}

impl<T: ZByteReaderTrait> JpegDecoder<T> {
    /// Reduced width and height, each rounded up from the coded dimensions.
    ///
    /// Returns `None` until headers and nonzero image dimensions are known.
    /// This does not select an output mode or change coded metadata.
    #[must_use]
    pub fn scaled_dimensions(&self, scale: JpegScale) -> Option<(usize, usize)> {
        if !self.headers_decoded || self.info.width == 0 || self.info.height == 0 {
            return None;
        }
        Some((
            (usize::from(self.info.width) * scale.numerator()).div_ceil(scale.denominator()),
            (usize::from(self.info.height) * scale.numerator()).div_ceil(scale.denominator())
        ))
    }

    /// Required packed reduced-output bytes for the current output colorspace.
    ///
    /// Returns `None` until dimensions are known, or if the size overflows.
    #[must_use]
    pub fn scaled_output_buffer_size(&self, scale: JpegScale) -> Option<usize> {
        let (width, height) = self.scaled_dimensions(scale)?;
        width
            .checked_mul(height)?
            .checked_mul(self.options.jpeg_get_out_colorspace().num_components())
    }

    /// Decode a complete reduced frame into packed pixels.
    ///
    /// A short buffer is rejected before decoding. Bytes beyond
    /// [`Self::scaled_output_buffer_size`] are untouched. Enable
    /// [`Self::set_incremental_mode`] for growing input and retry with the same
    /// scale: reduced planes and entropy checkpoints survive recoverable scan
    /// EOF, without publishing partial pixels. Successful calls with the same
    /// scale copy the cached frame without decoding the input again.
    ///
    /// Changing scale, changing options, or switching between scaled and
    /// full-size/raw/scanline output discards the previous output sequence and
    /// replays from scan start. Coded metadata and existing full-size sizing
    /// methods are unchanged. Full-size pixel/preview progress remains zero
    /// during scaled decoding; this API does not publish progressive previews.
    pub fn decode_scaled_into(
        &mut self, scale: JpegScale, output: &mut [u8]
    ) -> Result<(), DecodeErrors> {
        self.decode_headers()?;
        let length = self
            .scaled_output_buffer_size(scale)
            .ok_or(DecodeErrors::FormatStatic(
                "scaled output size overflow or unknown image dimensions"
            ))?;
        if output.len() < length {
            return Err(DecodeErrors::TooSmallOutput(length, output.len()));
        }
        self.abort_raw_pull_sequence();
        self.abort_scanline_sequence();
        let mut state = match self.scaled_output.take() {
            Some(state) if state.scale == scale => state,
            _ => {
                self.clear_scan_checkpoints();
                Box::new(ScaledDecodeState {
                    scale,
                    output: ScaledOutput::new(self, scale)?,
                    complete: false
                })
            }
        };
        let result = (|| {
            if !state.complete {
                self.prepare_for_scan_decode()?;
                self.decode_mcu_output_with_success_cleanup(&mut McuDecodeOutput::Scaled(
                    &mut state.output
                ))?;
                state.complete = true;
            }
            state.output.pack(self, &mut output[..length])
        })();
        self.pixels_decoded = 0;
        self.progressive_displayed_scans = 0;
        self.scaled_output = Some(state);
        result
    }

    pub(crate) fn abort_scaled_sequence(&mut self) {
        if self.scaled_output.take().is_some() {
            self.clear_scan_checkpoints();
        }
    }
}

pub(crate) struct ScaledOutput {
    width:  usize,
    planes: Vec<ScaledPlane>
}

struct ScaledPlane {
    width:             usize,
    height:            usize,
    stride:            usize,
    block_size:        usize,
    horizontal_expand: usize,
    vertical_expand:   usize,
    fancy:             bool,
    transform:         IDCTPtr,
    samples:           Vec<i16>
}

impl ScaledOutput {
    fn new<T: ZByteReaderTrait>(
        decoder: &JpegDecoder<T>, scale: JpegScale
    ) -> Result<Self, DecodeErrors> {
        let width = usize::from(decoder.info.width);
        let height = usize::from(decoder.info.height);
        if width == 0 || height == 0 {
            return Err(DecodeErrors::FormatStatic(
                "scaled decoding requires known image dimensions"
            ));
        }
        let minimum = 8 * scale.numerator() / scale.denominator();
        let h_max = decoder
            .components
            .iter()
            .map(|c| c.horizontal_sample)
            .max()
            .ok_or(DecodeErrors::FormatStatic("scaled image has no components"))?;
        let v_max = decoder
            .components
            .iter()
            .map(|c| c.vertical_sample)
            .max()
            .ok_or(DecodeErrors::FormatStatic("scaled image has no components"))?;
        let mut planes = Vec::with_capacity(decoder.components.len());
        for component in &decoder.components {
            let mut block_size = minimum;
            // Match native component-size selection: e.g. half-scale 4:2:0
            // reconstructs 4x4 luma and 8x8 chroma, requiring no upsampling.
            while block_size < 8
                && (h_max * minimum) % (component.horizontal_sample * block_size * 2) == 0
                && (v_max * minimum) % (component.vertical_sample * block_size * 2) == 0
            {
                block_size *= 2;
            }
            let transform = match block_size {
                1 => idct_scaled_1x1,
                2 => idct_scaled_2x2,
                3 => idct_scaled_3x3,
                4 => idct_scaled_4x4,
                5 => idct_scaled_5x5,
                6 => idct_scaled_6x6,
                7 => idct_scaled_7x7,
                8 => decoder.idct_func,
                10 => idct_scaled_10x10,
                12 => idct_scaled_12x12,
                14 => idct_scaled_14x14,
                _ => unreachable!()
            };
            let stride = width.div_ceil(h_max * 8) * component.horizontal_sample * block_size;
            let rows = height.div_ceil(v_max * 8) * component.vertical_sample * block_size;
            let length = stride
                .checked_mul(rows)
                .ok_or(DecodeErrors::FormatStatic("scaled component size overflow"))?;
            planes.push(ScaledPlane {
                width: (width * component.horizontal_sample * block_size).div_ceil(h_max * 8),
                height: (height * component.vertical_sample * block_size).div_ceil(v_max * 8),
                stride,
                block_size,
                horizontal_expand: h_max * minimum / (component.horizontal_sample * block_size),
                vertical_expand: v_max * minimum / (component.vertical_sample * block_size),
                fancy: minimum > 1,
                transform,
                samples: vec![128; length]
            });
        }
        Ok(Self {
            width: (width * scale.numerator()).div_ceil(scale.denominator()),
            planes
        })
    }

    pub(crate) fn write_block(
        &mut self, component: usize, x: usize, y: usize, coefficients: &[i32; 64]
    ) -> Result<(), DecodeErrors> {
        let plane = self
            .planes
            .get_mut(component)
            .ok_or(DecodeErrors::FormatStatic(
                "invalid scaled output component"
            ))?;
        let side = plane.block_size;
        if x >= plane.stride / side || y >= plane.samples.len() / plane.stride / side {
            return Err(DecodeErrors::FormatStatic("scaled block outside component"));
        }
        let offset = y * side * plane.stride + x * side;
        // Full-size chroma IDCTs may mutate their input. Keep the entropy
        // decoder's sparse scratch coefficients intact on every backend.
        let mut coefficients = *coefficients;
        (plane.transform)(
            &mut coefficients,
            &mut plane.samples[offset..],
            plane.stride
        );
        Ok(())
    }

    fn pack<T: ZByteReaderTrait>(
        &self, decoder: &JpegDecoder<T>, output: &mut [u8]
    ) -> Result<(), DecodeErrors> {
        let padded_width = self.width.div_ceil(16) * 16;
        let mut rows: [Vec<i16>; MAX_COMPONENTS] = core::array::from_fn(|_| vec![0; padded_width]);
        let output_colorspace = decoder.options.jpeg_get_out_colorspace();
        let row_bytes = self.width * output_colorspace.num_components();
        let mut cancel = decoder.cancel_debounced(self.width.div_ceil(8));
        for (y, row) in output.chunks_exact_mut(row_bytes).enumerate() {
            if cancel.is_cancelled() {
                return Err(DecodeErrors::Cancelled);
            }
            for (plane, samples) in self.planes.iter().zip(rows.iter_mut()) {
                if plane.horizontal_expand == 1 && plane.vertical_expand == 1 {
                    let start = y.min(plane.height - 1) * plane.stride;
                    let width = self.width.min(plane.width);
                    samples[..width].copy_from_slice(&plane.samples[start..start + width]);
                    let edge = samples[width - 1];
                    samples[width..self.width].fill(edge);
                } else {
                    for (x, sample) in samples[..self.width].iter_mut().enumerate() {
                        *sample = plane.sample(x, y);
                    }
                }
                let edge = samples[self.width - 1];
                samples[self.width..].fill(edge);
            }
            color_convert(
                &core::array::from_fn(|index| rows[index].as_slice()),
                decoder.color_convert_16,
                decoder.input_colorspace,
                output_colorspace,
                row,
                self.width,
                padded_width
            )?;
        }
        Ok(())
    }
}

impl ScaledPlane {
    fn at(&self, x: usize, y: usize) -> i16 {
        self.samples[y.min(self.height - 1) * self.stride + x.min(self.width - 1)]
    }

    fn sample(&self, x: usize, y: usize) -> i16 {
        let source_x = x / self.horizontal_expand;
        let source_y = y / self.vertical_expand;
        let current = self.at(source_x, source_y);
        let adjacent_x = if x % 2 == 0 { source_x.saturating_sub(1) } else { source_x + 1 };
        let adjacent_y = if y % 2 == 0 { source_y.saturating_sub(1) } else { source_y + 1 };
        match (self.horizontal_expand, self.vertical_expand) {
            (2, 1) if self.fancy && self.width > 2 => {
                (3 * current + self.at(adjacent_x, source_y) + if x % 2 == 0 { 1 } else { 2 }) >> 2
            }
            (1, 2) if self.fancy => {
                (3 * current + self.at(source_x, adjacent_y) + if y % 2 == 0 { 1 } else { 2 }) >> 2
            }
            (2, 2) if self.fancy && self.width > 2 => {
                let near = 3 * current + self.at(source_x, adjacent_y);
                let far = 3 * self.at(adjacent_x, source_y) + self.at(adjacent_x, adjacent_y);
                (3 * near + far + if x % 2 == 0 { 8 } else { 7 }) >> 4
            }
            _ => current
        }
    }
}
