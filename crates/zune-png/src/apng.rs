/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
#![allow(dead_code, unused_imports)] // when building for no_std
use alloc::vec::Vec;

use zune_core::colorspace::ColorSpace;

use crate::error::PngDecodeErrors;
use crate::PngInfo;

/// `num_frames` indicates the total number of frames in the animation. This must equal the number of `fcTL` chunks. 0 is not a valid value.
/// 1 is a valid value for a single-frame APNG.
/// If this value does not equal the actual number of frames it should be treated as an error.
//
// `num_plays` indicates the number of times that this animation should play;
// if it is 0, the animation should play indefinitely.
// If nonzero, the animation should come to rest on the final frame at the end of the last play.
#[derive(Copy, Clone)]
pub struct ActlChunk {
    pub num_frames: u32,
    pub num_plays: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisposeOp {
    /// No disposal is done on this frame before rendering the next;
    None,
    /// The frame's region of the output buffer is to be
    /// cleared to fully transparent black before rendering the next frame.
    Background,
    /// The frame's region of the output buffer is
    /// to be reverted to the previous contents before rendering the next frame.
    Previous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendOp {
    /// all color components of the frame, including alpha,
    /// overwrite the current contents of the frame's output buffer region.
    Source,
    /// Frame should be composited onto the output buffer
    /// based on its alpha, using a simple OVER operation as described
    /// in the "Alpha Channel Processing" section of the PNG specification [PNG-1.2]
    Over,
}

impl BlendOp {
    pub fn from_int(int: u8) -> Result<BlendOp, PngDecodeErrors> {
        match int {
            0 => Ok(BlendOp::Source),
            1 => Ok(BlendOp::Over),
            _ => Err(PngDecodeErrors::GenericStatic("Unknown blend operation")),
        }
    }
}

impl DisposeOp {
    pub fn from_int(int: u8) -> Result<DisposeOp, PngDecodeErrors> {
        match int {
            0 => Ok(DisposeOp::None),
            1 => Ok(DisposeOp::Background),
            2 => Ok(DisposeOp::Previous),
            _ => Err(PngDecodeErrors::GenericStatic("Unknown blend operation")),
        }
    }
}

/// Describes a single frame
#[derive(Clone, Copy, Debug)]
pub struct FrameInfo {
    /// Sequence number of frame. If image isn't an APNG, it is usually
    /// set to zero
    pub seq_number: i32,
    /// Width of the frame, If image isn't an APNG, it matches the width of
    /// the image
    pub width: usize,
    /// Height of frame. If image isn't an APNG, it matches the height of the image
    pub height: usize,
    /// X position at which to render the following frame, if image isn't APNG, set to zero
    pub x_offset: usize,
    /// Y position at which to render the following frame, if image isn't APNG, set to zero
    pub y_offset: usize,
    /// Frame delay fraction numerator
    pub delay_num: u16,
    /// Frame delay fraction denominator
    pub delay_denom: u16,
    /// Type of frame area disposal to be done after rendering this frame
    pub dispose_op: DisposeOp,
    /// Type of frame area rendering for this frame
    pub blend_op: BlendOp,
    /// Whether the frame is supposed to be part of the
    /// animation sequence.
    ///
    /// If an image contains standard IDAT chunks, this is what is to be
    /// displayed in case the decoder doesn't support apng but here
    /// it is usually the first frame
    pub is_part_of_seq: bool,
}

/// Represents a single frame
pub struct SingleFrame {
    /// If none, indicates data is IDAT, hence
    /// should be decoded as such
    pub fctl_info: FrameInfo,
}

impl SingleFrame {
    /// Create a new frame
    pub fn new(fctl_info: FrameInfo) -> SingleFrame {
        SingleFrame { fctl_info }
    }
}

/// ERROR:
///
/// #use  [post_process_image_apng]
///
/// This function had the wrong signature for post processing an APNG image, but due
/// to backward compatibility could not be entirely removed
#[deprecated(
    note = "This function signature was wrong, but maintained for backwards compatibility, use post_process_image_apng instead"
)]
#[allow(unused_variables, unused_mut)]
pub fn post_process_image(
    info: &PngInfo, colorspace: ColorSpace, frame_info: &FrameInfo, current_frame: &[u8],
    prev_frame: Option<&[u8]>, output: &mut [u8], gamma: Option<f32>,
) -> Result<(), PngDecodeErrors> {
    Err(PngDecodeErrors::GenericStatic(
        "Wrong function signature, use post_process_image_apng",
    ))
}

pub trait PixelDepth: Copy + PartialEq {
    const MAX_FLOAT: f32;

    fn zero() -> Self;
    fn is_transparent(self) -> bool;
    fn is_opaque(self) -> bool;
    fn to_f32(self) -> f32;
    fn from_linear(val: f32) -> Self;
}
impl PixelDepth for u8 {
    const MAX_FLOAT: f32 = 255.0;

    fn zero() -> Self {
        0
    }
    fn is_transparent(self) -> bool {
        self == 0
    }
    fn is_opaque(self) -> bool {
        self == 255
    }
    fn to_f32(self) -> f32 {
        self as f32
    }

    fn from_linear(val: f32) -> Self {
        (val * Self::MAX_FLOAT + 0.5).clamp(0.0, Self::MAX_FLOAT) as u8
    }
}

impl PixelDepth for u16 {
    const MAX_FLOAT: f32 = 65535.0;

    fn zero() -> Self {
        0
    }
    fn is_transparent(self) -> bool {
        self == 0
    }
    fn is_opaque(self) -> bool {
        self == 65535
    }
    fn to_f32(self) -> f32 {
        self as f32
    }

    fn from_linear(val: f32) -> Self {
        (val * Self::MAX_FLOAT + 0.5).clamp(0.0, Self::MAX_FLOAT) as u16
    }
}
/// Convert a single APNG frame into a fully composited image on the canvas.
///
/// The APNG specification requires a strict lifecycle for post-processing individual frames:
/// 1. **Disposal**: Modifying the canvas based on the *previous* frame's disposal operation
///    (e.g., clearing it to background or reverting it to a previous state).
/// 2. **Blending**: Compositing the *current* frame's raw pixels onto the canvas using its blend operation.
///
/// This function performs both operations in the correct order. It also automatically saves a snapshot
/// of the canvas into the `canvas_backup` buffer if the current frame requests `DisposeOp::Previous`
/// for the next iteration.
///
/// In the case of alpha compositing (`BlendOp::Over`), blending is performed in linear space.
/// The code matches the specification at [Alpha Channel Processing](https://www.w3.org/TR/2003/REC-PNG-20031110/#13Alpha-channel-processing).
///
/// # Requires
/// Requires the `#[std]` feature as we need the `powf` function from the standard library.
///
/// # Arguments
///
/// * `info`: PNG information containing the width and height of the main canvas.
/// * `colorspace`: The image colorspace, obtained from the decoder via `get_colorspace()`.
/// * `frame_info`: The `FrameInfo` for the *current* frame being processed.
/// * `prev_frame_info`: The `FrameInfo` for the *previous* frame. This is required to determine the correct disposal area and method. Pass `None` for the very first frame.
/// * `current_frame`: The raw decoded pixels of the current frame.
/// * `canvas_backup`: A mutable reference to a backup buffer. This must be the same size as `output`. It is used to restore the canvas if the previous frame requested `DisposeOp::Previous`, and is automatically updated internally if the current frame requests it for the next round.
/// * `output`: The main output canvas buffer. The dimensions should be `(info.width * info.height * colorspace.num_components())`.
/// * `gamma`: An optional gamma value. Best retrieved from [the image's gamma](crate::PngInfo.gamma). If `None`, it defaults to 2.2.
///
/// # Returns
/// * `Ok(())` on success. The composited image is written directly to the `output` variable.
///
/// # Examples
///
/// ```no_run
/// use zune_core::bytestream::ZCursor;
/// use zune_core::options::EncoderOptions;
/// use zune_png::{PngDecoder, post_process_image_apng, FrameInfo};
///
/// // Set up the decoder
/// let mut decoder = PngDecoder::new(ZCursor::new(&[]));
/// decoder.decode_headers().unwrap();
///
/// // Get useful information about the image
/// let colorspace = decoder.colorspace().unwrap();
/// let depth = decoder.depth().unwrap();
/// let info = decoder.info().unwrap().clone();
///
/// // Allocate the main canvas and a backup canvas for DisposeOp::Previous
/// let buffer_size = info.width * info.height * colorspace.num_components();
/// let mut output = vec![0; buffer_size];
/// let mut canvas_backup = vec![0; buffer_size];
///
/// // Keep track of the previous frame's info for disposal
/// let mut prev_frame_info: Option<FrameInfo> = None;
/// let mut i = 0;
///
/// while decoder.more_frames() {
///     decoder.decode_headers().unwrap();
///
///     // Get the current frame info. We clone it so we can save it at the end of the loop.
///     let frame = decoder.frame_info().unwrap().clone();
///
///     // Decode the raw pixels for this specific frame bounding box
///     let pix = decoder.decode_raw().unwrap();
///
///     // Process the frame (handles disposal of the old frame, and blending of the new one)
///     post_process_image_apng(
///         &info,
///         colorspace,
///         &frame,
///         prev_frame_info.as_ref(), // Use previous frame's rules for disposal
///         &pix,
///         &mut canvas_backup,       // Pass mutable backup buffer
///         &mut output,              // The main canvas
///         None
///     ).unwrap();
///
///     // The `output` buffer now contains the fully composited frame.
///     // (Encoding step omitted for brevity)
///     // let encoder_opts = EncoderOptions::new(info.width, info.height, colorspace, depth);
///     // let bytes = zune_png::PngEncoder::new(&output, encoder_opts).encode(&mut vec![]);
///
///     // Store the current frame info so it becomes the previous frame on the next iteration
///     prev_frame_info = Some(frame);
///     i += 1;
/// }
/// ```
#[cfg(feature = "std")]
#[allow(clippy::too_many_arguments)]
pub fn post_process_image_apng<T: PixelDepth>(
    info: &PngInfo, colorspace: ColorSpace, frame_info: &FrameInfo,
    prev_frame_info: Option<&FrameInfo>, current_frame: &[T], canvas_backup: &mut [T],
    output: &mut [T], gamma: Option<f32>,
) -> Result<(), PngDecodeErrors> where usize:From<T> {
    let nc = colorspace.num_components();

    // DISPOSAL (Applies to the PREVIOUS frame's state)
    if let Some(prev_info) = prev_frame_info {
        match prev_info.dispose_op {
            DisposeOp::None => {} // Do nothing
            DisposeOp::Background => {
                // Clear ONLY the exact bounding box of the previous frame
                for line_stride in output
                    .chunks_exact_mut(info.width * nc)
                    .skip(prev_info.y_offset)
                    .take(prev_info.height)
                {
                    let start = prev_info.x_offset * nc;
                    let end = (prev_info.x_offset + prev_info.width) * nc;
                    line_stride[start..end].fill(T::zero());
                }
            }
            DisposeOp::Previous => {
                // Restore the canvas state from the backup buffer
                if output.len() != canvas_backup.len() {
                    return Err(PngDecodeErrors::GenericStatic(
                        "Backup canvas size does not match output length",
                    ));
                }

                for (out_line, backup_line) in output
                    .chunks_exact_mut(info.width * nc)
                    .skip(prev_info.y_offset)
                    .take(prev_info.height)
                    .zip(
                        canvas_backup
                            .chunks_exact(info.width * nc)
                            .skip(prev_info.y_offset)
                            .take(prev_info.height),
                    )
                {
                    let start = prev_info.x_offset * nc;
                    let end = (prev_info.x_offset + prev_info.width) * nc;
                    out_line[start..end].copy_from_slice(&backup_line[start..end]);
                }
            }
        }
    }
    // If the current frame requires DisposeOp::Previous on the next loop,
    // we must save the canvas state now, after previous disposal, but before blending!
    if frame_info.dispose_op == DisposeOp::Previous {
        canvas_backup.copy_from_slice(output);
    }

    if frame_info.x_offset + frame_info.width > info.width {
        return Err(PngDecodeErrors::GenericStatic(
            "Frame X offset + width larger than image width",
        ));
    }
    if frame_info.y_offset + frame_info.height > info.height {
        return Err(PngDecodeErrors::GenericStatic(
            "Frame y offset + height larger than image height",
        ));
    }

    let frame_dims = frame_info.height * frame_info.width * nc;
    if current_frame.len() < frame_dims {
        return Err(PngDecodeErrors::Generic(format!(
            "Current frame dimensions ({}) less than expected ({})",
            current_frame.len(),
            frame_dims
        )));
    }

    let gamma_value = gamma.unwrap_or(2.2);
    let gamma_inv = 1.0 / gamma_value;

    let mut gamma_values = vec![0.0; (T::MAX_FLOAT+1.0) as usize];
    let mut inv_gamma_values = vec![0.0; (T::MAX_FLOAT+1.0) as usize];
    let max_sample = T::MAX_FLOAT;

    for (i, (item, c)) in gamma_values
        .iter_mut()
        .zip(inv_gamma_values.iter_mut())
        .enumerate()
    {
        let gam = (i as f32) / max_sample;
        *item = f32::powf(gam, gamma_inv);
        *c = f32::powf(gam, gamma_value);
    }

    for (src_width, h) in current_frame.chunks_exact(frame_info.width * nc).zip(
        output
            .chunks_exact_mut(info.width * nc)
            .skip(frame_info.y_offset)
            .take(frame_info.height),
    ) {
        let h_x = &mut h[frame_info.x_offset * nc..(frame_info.x_offset + frame_info.width) * nc];

        match frame_info.blend_op {
            BlendOp::Source => {
                h_x.copy_from_slice(src_width);
            }
            BlendOp::Over => {
                if !colorspace.has_alpha() {
                    return Err(PngDecodeErrors::GenericStatic(
                        "Image needs alpha for BlendOp::Over",
                    ));
                }

                for (src_comp, dst_comp) in src_width.chunks_exact(nc).zip(h_x.chunks_exact_mut(nc))
                {
                    let src_a_u8 = src_comp[nc - 1];

                    if src_a_u8.is_transparent() {
                        continue;
                    }

                    if src_a_u8.is_opaque() {
                        dst_comp.copy_from_slice(src_comp);
                        continue;
                    }

                    let foreground_alpha = src_a_u8.to_f32() / T::MAX_FLOAT;
                    let dst_alpha_u8 = dst_comp[nc - 1];
                    let background_alpha = dst_alpha_u8.to_f32() / T::MAX_FLOAT;

                    let out_alpha = foreground_alpha + background_alpha * (1.0 - foreground_alpha);

                    if out_alpha > 0.0 {
                        for (a, b) in src_comp.iter().zip(dst_comp.iter_mut()).take(nc - 1) {
                            let linfg = gamma_values[usize::from(*a)];
                            let linbg = gamma_values[usize::from(*b)];

                            let commpix = (linfg * foreground_alpha
                                + linbg * background_alpha * (1.0 - foreground_alpha))
                                / out_alpha;

                            let gamout = f32::powf(commpix, gamma_value);

                            *b = T::from_linear(gamout)
                        }
                        // Write the final calculated alpha back to destination
                        dst_comp[nc - 1] = T::from_linear(out_alpha);
                    } else {
                        dst_comp.fill(T::zero());
                    }
                }
            }
        }
    }
    Ok(())
}
