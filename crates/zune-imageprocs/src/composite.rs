use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;
use crate::utils::{calculate_gravity, Gravity};

/// Composite method to use when composing two images together.
///
/// Each variant corresponds to a Porter-Duff compositing operator or a blend mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CompositeMethod {
    /// Porter-Duff: place source over destination, blending via source alpha.
    Over,
    /// Porter-Duff: replace destination entirely with source.
    ///
    /// The destination is first cleared to opaque white (or max value), then the
    /// source is copied on top.
    Src,
    /// Porter-Duff: keep destination unchanged. A no-op.
    Dst,
    /// Porter-Duff: keep destination only where source is present (non-zero alpha).
    ///
    /// Requires an alpha channel. Uses the source alpha as a mask applied to the
    /// destination channels.
    DstIn,
    /// Porter-Duff: keep destination only where source is absent (zero alpha).
    ///
    /// Requires an alpha channel. Inverts the source alpha mask before applying it
    /// to the destination channels.
    DstOut,
    /// Porter-Duff: keep source only where destination has content (non-zero alpha).
    ///
    /// Requires an alpha channel.
    SrcIn,
    /// Porter-Duff: keep source only where destination has no content (zero alpha).
    ///
    /// Requires an alpha channel.
    SrcOut,
    /// Porter-Duff: show source and destination where they do not overlap; discard
    /// pixels that appear in both.
    ///
    /// Requires an alpha channel.
    Xor,
    /// Blend mode: multiply source and destination channel values together.
    ///
    /// The result is always darker than or equal to either input. Works on both
    /// alpha and non-alpha images.
    Multiply,
    /// Blend mode: invert-multiply-invert (complement of Multiply).
    ///
    /// The result is always lighter than or equal to either input. Works on both
    /// alpha and non-alpha images.
    Screen,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CompositeMethodType {
    /// Method operates purely on channel values; no alpha compositing math required.
    ChannelBased,
    /// Method requires an alpha channel to compute a mask.
    AlphaChannel,
}

impl CompositeMethod {
    fn composite_type(self) -> CompositeMethodType {
        match self {
            CompositeMethod::Src
            | CompositeMethod::Dst
            | CompositeMethod::Over
            | CompositeMethod::Multiply
            | CompositeMethod::Screen => CompositeMethodType::ChannelBased,
            CompositeMethod::DstIn
            | CompositeMethod::DstOut
            | CompositeMethod::SrcIn
            | CompositeMethod::SrcOut
            | CompositeMethod::Xor => CompositeMethodType::AlphaChannel,
        }
    }
}
#[allow(clippy::struct_field_names)]
pub struct Composite {
    geometry: Option<(usize, usize)>,
    composite_method: CompositeMethod,
    gravity: Option<Gravity>,
}

impl Composite {
    /// Create a new filter that will copy an image to a specific location specified by `position`
    /// using the composite method specified.
    ///
    /// The source image will be pulled from the top of the image stack during execution.
    #[must_use]
    pub fn new(composite_method: CompositeMethod, position: (usize, usize)) -> Composite {
        Composite {
            geometry: Some(position),
            composite_method,
            gravity: None,
        }
    }

    /// Create a new filter that will composite placing it in the location
    /// specified by gravity using the composite method specified.
    ///
    /// The source image will be pulled from the top of the image stack during execution.
    #[must_use]
    pub fn new_gravity(composite_method: CompositeMethod, gravity: Gravity) -> Composite {
        Composite {
            geometry: None,
            gravity: Some(gravity),
            composite_method,
        }
    }
}

impl OperationsTrait for Composite {
    fn name(&self) -> &'static str {
        "Composite"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Composite requires multiple images; it must be called via execute_multiple",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    /// Executes the composite operation on the current image stack.
    ///
    /// It requires at least two images to be present in the pipeline's image queue.
    ///
    /// # Stack Manipulation
    /// **Note: This operation removes an item from the image stack.**
    ///
    /// The workflow is as follows:
    /// 1. **Pop:** The last (top) image is completely removed from the stack. This
    ///    becomes the `source` (overlay) image.
    /// 2. **Peek:** The next image in the stack (the new top) is accessed mutably.
    ///    This becomes the `destination` (background) image.
    /// 3. **Composite:** The `source` image is drawn onto the `destination` image
    ///    according to the configured geometry/gravity and composite method.
    ///
    /// As a result, the total number of images in the pipeline will decrease by one
    /// after this operation successfully executes.
    ///
    /// # Errors
    /// * Returns an error if the image stack contains fewer than 2 images.
    /// * Returns an error if the source and destination images have incompatible
    ///   bit depths or colorspaces.
    #[allow(clippy::too_many_lines)]
    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.len() < 2 {
            return Err(ImageErrors::GenericStr(
                "Composite requires at least two images in the pipeline",
            ));
        }

        // Pop the overlay image off the stack
        let src_image = images.pop().unwrap();

        // The background image is now the top of the stack
        let dst_image = images.last_mut().unwrap();

        let dims = if let Some(gravity) = self.gravity {
            calculate_gravity(&src_image, dst_image, gravity)
        } else if let Some(geometry) = self.geometry {
            geometry
        } else {
            unreachable!()
        };

        let (src_width, _) = src_image.dimensions();
        let (dst_width, _) = dst_image.dimensions();

        // Confirm compatibility
        if dst_image.depth() != src_image.depth() {
            return Err(ImageErrors::GenericStr(
                "Image depths do not match for composite",
            ));
        }

        if dst_image.colorspace() != src_image.colorspace() {
            return Err(ImageErrors::GenericString(format!(
                "Image colorspace does not match for composite src image = {:?}, dst_image = {:?}",
                src_image.colorspace(),
                dst_image.colorspace()
            )));
        }

        let b_type = dst_image.depth().bit_type();
        let colorspace = dst_image.colorspace();

        match self.composite_method.composite_type() {
            CompositeMethodType::ChannelBased => {
                if colorspace.has_alpha() {
                    for (src_frame, dst_frame) in
                        src_image.frames_ref().iter().zip(dst_image.frames_mut())
                    {
                        let (src_color_channels, src_alpha_channel) =
                            src_frame.separate_color_and_alpha_ref(colorspace).unwrap();

                        let (dst_color_channels, dst_alpha_channel) =
                            dst_frame.separate_color_and_alpha_mut(colorspace).unwrap();

                        for (src_chan, d_chan) in src_color_channels.iter().zip(dst_color_channels)
                        {
                            match b_type {
                                BitType::U8 => composite_alpha::<u8>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method,
                                ),
                                BitType::U16 => composite_alpha::<u16>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method,
                                ),
                                BitType::F32 => composite_alpha::<f32>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method,
                                ),
                                d => {
                                    return Err(ImageErrors::ImageOperationNotImplemented(
                                        self.name(),
                                        d,
                                    ));
                                }
                            }
                        }

                        // Composite the alpha channel
                        match b_type {
                            BitType::U8 => composite_alpha_channel::<u8>(
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::U16 => composite_alpha_channel::<u16>(
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::F32 => composite_alpha_channel::<f32>(
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d,
                                ));
                            }
                        }
                    }
                } else {
                    for (src_chan, d_chan) in src_image
                        .channels_ref(false)
                        .iter()
                        .zip(dst_image.channels_mut(false))
                    {
                        match b_type {
                            BitType::U8 => composite::<u8>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::U16 => composite::<u16>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::F32 => composite::<f32>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d,
                                ));
                            }
                        }
                    }
                }
            }
            CompositeMethodType::AlphaChannel => {
                if !colorspace.has_alpha() {
                    return Err(ImageErrors::GenericString(format!(
                        "Composite method {:?} requires an alpha channel, but colorspace {:?} has none",
                        self.composite_method, colorspace
                    )));
                }

                for (src_frame, dst_frame) in
                    src_image.frames_ref().iter().zip(dst_image.frames_mut())
                {
                    let (src_color_channels, src_alpha_channel) =
                        src_frame.separate_color_and_alpha_ref(colorspace).unwrap();

                    let (dst_color_channels, dst_alpha_channel) =
                        dst_frame.separate_color_and_alpha_mut(colorspace).unwrap();

                    for (src_chan, d_chan) in src_color_channels.iter().zip(dst_color_channels) {
                        match b_type {
                            BitType::U8 => composite_alpha_masked::<u8>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::U16 => composite_alpha_masked::<u16>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            BitType::F32 => composite_alpha_masked::<f32>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                src_alpha_channel.reinterpret_as()?,
                                dst_alpha_channel.reinterpret_as()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method,
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d,
                                ));
                            }
                        }
                    }

                    // Update the output alpha channel for alpha-based methods
                    match b_type {
                        BitType::U8 => composite_alpha_channel_masked::<u8>(
                            src_alpha_channel.reinterpret_as()?,
                            dst_alpha_channel.reinterpret_as_mut()?,
                            dims.0,
                            dims.1,
                            src_width,
                            dst_width,
                            self.composite_method,
                        ),
                        BitType::U16 => composite_alpha_channel_masked::<u16>(
                            src_alpha_channel.reinterpret_as()?,
                            dst_alpha_channel.reinterpret_as_mut()?,
                            dims.0,
                            dims.1,
                            src_width,
                            dst_width,
                            self.composite_method,
                        ),
                        BitType::F32 => composite_alpha_channel_masked::<f32>(
                            src_alpha_channel.reinterpret_as()?,
                            dst_alpha_channel.reinterpret_as_mut()?,
                            dims.0,
                            dims.1,
                            src_width,
                            dst_width,
                            self.composite_method,
                        ),
                        d => {
                            return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// ─── Channel-based dispatch ───────────────────────────────────────────────────

/// Dispatch channel-based compositing for images that have an alpha channel.
///
/// The `src_alpha` slice is used only for `Over` (which needs to blend by alpha);
/// all other channel-based methods ignore it and operate directly on pixel values.
#[allow(clippy::too_many_arguments)]
fn composite_alpha<T>(
    src: &[T], dest: &mut [T], src_alpha: &[T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize, method: CompositeMethod,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    match method {
        CompositeMethod::Over => composite_over_alpha(
            src, dest, src_alpha, start_x, start_y, width_src, width_dest,
        ),
        // For channel-based methods without alpha blending the alpha channel is
        // simply ignored and we fall through to the plain `composite` path.
        other => composite(src, dest, start_x, start_y, width_src, width_dest, other),
    }
}

/// Dispatch channel-based compositing for images **without** an alpha channel.
fn composite<T: Copy + NumOps<T>>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize,
    method: CompositeMethod,
) where
    f32: From<T>,
{
    match method {
        CompositeMethod::Over => composite_over(src, dest, start_x, start_y, width_src, width_dest),
        CompositeMethod::Src => composite_src(src, dest, start_x, start_y, width_src, width_dest),
        CompositeMethod::Dst => {} // no-op
        CompositeMethod::Multiply => composite_blend(
            src,
            dest,
            start_x,
            start_y,
            width_src,
            width_dest,
            blend_multiply,
        ),
        CompositeMethod::Screen => composite_blend(
            src,
            dest,
            start_x,
            start_y,
            width_src,
            width_dest,
            blend_screen,
        ),
        CompositeMethod::DstIn
        | CompositeMethod::DstOut
        | CompositeMethod::SrcIn
        | CompositeMethod::SrcOut
        | CompositeMethod::Xor => {
            unreachable!("Alpha-channel methods must not be routed through composite()")
        }
    }
}

/// Update the **alpha channel** for channel-based composite methods.
///
/// `Over` uses the Porter-Duff formula: `α_o = α_src + α_dst · (1 − α_src)`.
/// All other channel-based methods leave the destination alpha untouched.
fn composite_alpha_channel<T>(
    alpha_src: &[T], dst_alpha: &mut [T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize, method: CompositeMethod,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    if method != CompositeMethod::Over {
        return;
    }

    let inv_max = 1.0 / f32::from(T::MAX_VAL);

    for (dst_row, src_row) in dst_alpha
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(alpha_src.chunks_exact(width_src))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice.len().min(src_row.len());
            for (src_a, dst_a) in src_row[..min_width].iter().zip(dst_slice.iter_mut()) {
                // Porter-Duff Over: α_o = α_src + α_dst · (1 − α_src)
                let a_src = (f32::from(*src_a) * inv_max).clamp(0.0, 1.0);
                let a_dst = (f32::from(*dst_a) * inv_max).clamp(0.0, 1.0);
                *dst_a =
                    T::from_f32(((a_src + a_dst * (1.0 - a_src)) * f32::from(T::MAX_VAL)).round());
            }
        }
    }
}

/// Apply a Porter-Duff operator that requires both source **and** destination
/// alpha to compute the output colour channels.
#[allow(clippy::too_many_arguments)]
fn composite_alpha_masked<T>(
    src: &[T], dest: &mut [T], src_alpha: &[T], dst_alpha: &[T], start_x: usize, start_y: usize,
    width_src: usize, width_dest: usize, method: CompositeMethod,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    let inv_max = 1.0 / f32::from(T::MAX_VAL);
    let max_val = f32::from(T::MAX_VAL);

    for (((dst_row, src_row), src_alpha_row), dst_alpha_row) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
        .zip(src_alpha.chunks_exact(width_src))
        .zip(dst_alpha.chunks_exact(width_dest).skip(start_y))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice
                .len()
                .min(src_row.len())
                .min(dst_alpha_row.len().saturating_sub(start_x));

            let dst_alpha_slice = &dst_alpha_row[start_x..start_x + min_width];

            for (((src_p, src_a), dst_a), dst_p) in src_row[..min_width]
                .iter()
                .zip(src_alpha_row[..min_width].iter())
                .zip(dst_alpha_slice.iter())
                .zip(dst_slice[..min_width].iter_mut())
            {
                let a_src = (f32::from(*src_a) * inv_max).clamp(0.0, 1.0);
                let a_dst = (f32::from(*dst_a) * inv_max).clamp(0.0, 1.0);
                let c_src = f32::from(*src_p) * inv_max;
                let c_dst = f32::from(*dst_p) * inv_max;

                let out = match method {
                    // DstIn: dst · α_src  (mask dst with src shape)
                    CompositeMethod::DstIn => c_dst * a_src,
                    // DstOut: dst · (1 − α_src)
                    CompositeMethod::DstOut => c_dst * (1.0 - a_src),
                    // SrcIn: src · α_dst
                    CompositeMethod::SrcIn => c_src * a_dst,
                    // SrcOut: src · (1 − α_dst)
                    CompositeMethod::SrcOut => c_src * (1.0 - a_dst),
                    // Xor: src·(1−α_dst) + dst·(1−α_src)
                    CompositeMethod::Xor => c_src * (1.0 - a_dst) + c_dst * (1.0 - a_src),
                    _ => unreachable!("Non-alpha method routed to composite_alpha_masked"),
                };

                *dst_p = T::from_f32((out.clamp(0.0, 1.0) * max_val).round());
            }
        }
    }
}

/// Update the **alpha channel** for Porter-Duff operators that require both
/// source and destination alpha values.
#[allow(clippy::too_many_arguments)]
fn composite_alpha_channel_masked<T>(
    src_alpha: &[T], dst_alpha: &mut [T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize, method: CompositeMethod,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    let inv_max = 1.0 / f32::from(T::MAX_VAL);
    let max_val = f32::from(T::MAX_VAL);

    for (dst_row, src_row) in dst_alpha
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src_alpha.chunks_exact(width_src))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice.len().min(src_row.len());
            for (src_a, dst_a) in src_row[..min_width].iter().zip(dst_slice.iter_mut()) {
                let a_src = (f32::from(*src_a) * inv_max).clamp(0.0, 1.0);
                let a_dst = (f32::from(*dst_a) * inv_max).clamp(0.0, 1.0);

                let out_alpha = match method {
                    // DstIn:  α_dst · α_src
                    // SrcIn:  α_src · α_dst
                    CompositeMethod::DstIn | CompositeMethod::SrcIn => a_dst * a_src,
                    // DstOut: α_dst · (1 − α_src)
                    CompositeMethod::DstOut => a_dst * (1.0 - a_src),
                    // SrcOut: α_src · (1 − α_dst)
                    CompositeMethod::SrcOut => a_src * (1.0 - a_dst),
                    // Xor:    α_src + α_dst − 2·α_src·α_dst
                    CompositeMethod::Xor => a_src + a_dst - 2.0 * a_src * a_dst,
                    _ => unreachable!("Non-alpha method routed to composite_alpha_channel_masked"),
                };

                *dst_a = T::from_f32((out_alpha.clamp(0.0, 1.0) * max_val).round());
            }
        }
    }
}

// ─── Primitive compositing helpers ───────────────────────────────────────────

/// Clear the destination to opaque (max value), then copy the source on top.
fn composite_src<T: Copy + NumOps<T>>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize,
) {
    dest.fill(T::MAX_VAL);
    composite_over(src, dest, start_x, start_y, width_src, width_dest);
}

/// Copy source pixels on top of destination (no alpha blending).
fn composite_over<T: Copy>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize,
) {
    for (dst_row, src_row) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice.len().min(src_row.len());
            dst_slice[..min_width].copy_from_slice(&src_row[..min_width]);
        }
    }
}

/// Blend source over destination using source alpha:
/// `C_out = α_src · C_src + (1 − α_src) · C_dst`
fn composite_over_alpha<T>(
    src: &[T], dest: &mut [T], src_alpha: &[T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    let inv_max = 1.0 / f32::from(T::max_val());
    let max_val = f32::from(T::MAX_VAL);

    for ((dst_row, src_row), src_alpha_row) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
        .zip(src_alpha.chunks_exact(width_src))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice.len().min(src_row.len());
            for ((src_p, src_a), dst_p) in src_row[..min_width]
                .iter()
                .zip(src_alpha_row[..min_width].iter())
                .zip(dst_slice[..min_width].iter_mut())
            {
                let a = (f32::from(*src_a) * inv_max).clamp(0.0, 1.0);
                *dst_p = T::from_f32(
                    (a * f32::from(*src_p) + (1.0 - a) * f32::from(*dst_p))
                        .clamp(0.0, max_val)
                        .round(),
                );
            }
        }
    }
}

/// Apply a per-pixel blend function row-by-row, writing results into `dest`.
#[allow(clippy::too_many_arguments)]
fn composite_blend<T>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize,
    blend_fn: fn(f32, f32) -> f32,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    let inv_max = 1.0 / f32::from(T::MAX_VAL);
    let max_val = f32::from(T::MAX_VAL);

    for (dst_row, src_row) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
    {
        if let Some(dst_slice) = dst_row.get_mut(start_x..) {
            let min_width = dst_slice.len().min(src_row.len());
            for (src_p, dst_p) in src_row[..min_width]
                .iter()
                .zip(dst_slice[..min_width].iter_mut())
            {
                let s = f32::from(*src_p) * inv_max;
                let d = f32::from(*dst_p) * inv_max;
                *dst_p = T::from_f32((blend_fn(s, d).clamp(0.0, 1.0) * max_val).round());
            }
        }
    }
}

// ─── Blend functions (operate on normalised [0, 1] values) ───────────────────

#[inline(always)]
fn blend_multiply(src: f32, dst: f32) -> f32 {
    src * dst
}

#[inline(always)]
fn blend_screen(src: f32, dst: f32) -> f32 {
    1.0 - (1.0 - src) * (1.0 - dst)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    use crate::composite::{Composite, CompositeMethod};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn rgba_pixel(r: u8, g: u8, b: u8, a: u8) -> Image {
        Image::from_fn(1usize, 1usize, ColorSpace::RGBA, |_y, _x, pixels| {
            pixels[0] = r;
            pixels[1] = g;
            pixels[2] = b;
            pixels[3] = a;
        })
    }

    fn rgb_pixel(r: u8, g: u8, b: u8) -> Image {
        Image::from_fn(1usize, 1usize, ColorSpace::RGB, |_y, _x, pixels| {
            pixels[0] = r;
            pixels[1] = g;
            pixels[2] = b;
        })
    }

    /// Read a single channel value from the first pixel of an image.
    fn pixel_channel(images: &[Image], channel: usize) -> u8 {
        unsafe {
            *images[0].channels_ref(false)[channel]
                .alias()
                .first()
                .unwrap()
        }
    }

    // ── Over (alpha-aware) ────────────────────────────────────────────────────

    /// Fully opaque source completely replaces the destination.
    #[test]
    fn over_opaque_src_replaces_dst() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0xff);
        let src = rgba_pixel(0xff, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Over, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(
            pixel_channel(&images, 0),
            0xff,
            "red channel should be 0xff"
        );
    }

    /// Fully transparent source leaves the destination unchanged.
    #[test]
    fn over_transparent_src_leaves_dst() {
        let dst = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let src = rgba_pixel(0xff, 0x00, 0x00, 0x00); // transparent red
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Over, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        // Destination should be essentially unchanged
        assert_eq!(
            pixel_channel(&images, 0),
            0x80,
            "red channel should stay 0x80"
        );
        assert_eq!(
            pixel_channel(&images, 1),
            0x80,
            "green channel should stay 0x80"
        );
    }

    /// Alpha channel follows the Porter-Duff Over formula:
    /// α_o = α_src + α_dst · (1 − α_src).
    #[test]
    fn over_alpha_channel_formula() {
        // opaque src over transparent dst → fully opaque result
        let dst = rgba_pixel(0, 0, 0, 0x00);
        let src = rgba_pixel(0, 0, 0, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Over, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 3), 0xff);
    }

    // ── Src ───────────────────────────────────────────────────────────────────

    /// Src clears the destination and copies the source.
    #[test]
    fn src_overwrites_dst() {
        let dst = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let src = rgba_pixel(0x42, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Src, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x42);
    }

    // ── Dst ───────────────────────────────────────────────────────────────────

    /// Dst is a no-op; destination must be unchanged.
    #[test]
    fn dst_leaves_destination_unchanged() {
        let dst = rgba_pixel(0xAB, 0xCD, 0xEF, 0xff);
        let src = rgba_pixel(0x01, 0x02, 0x03, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Dst, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0xAB);
        assert_eq!(pixel_channel(&images, 1), 0xCD);
        assert_eq!(pixel_channel(&images, 2), 0xEF);
    }

    // ── DstIn ─────────────────────────────────────────────────────────────────

    /// DstIn with fully opaque source leaves destination channels unchanged.
    #[test]
    fn dst_in_opaque_src_preserves_dst() {
        let dst = rgba_pixel(0x80, 0x40, 0x20, 0xff);
        let src = rgba_pixel(0x00, 0x00, 0x00, 0xff); // fully opaque mask
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::DstIn, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x80);
    }

    /// DstIn with fully transparent source zeros the destination channels.
    #[test]
    fn dst_in_transparent_src_clears_dst() {
        let dst = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let src = rgba_pixel(0x00, 0x00, 0x00, 0x00); // fully transparent
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::DstIn, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x00);
        assert_eq!(
            pixel_channel(&images, 3),
            0x00,
            "alpha should also be zeroed"
        );
    }

    // ── DstOut ────────────────────────────────────────────────────────────────

    /// DstOut with fully transparent source leaves destination unchanged.
    #[test]
    fn dst_out_transparent_src_keeps_dst() {
        let dst = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let src = rgba_pixel(0x00, 0x00, 0x00, 0x00);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::DstOut, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x80);
    }

    /// DstOut with fully opaque source clears the destination.
    #[test]
    fn dst_out_opaque_src_clears_dst() {
        let dst = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let src = rgba_pixel(0x00, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::DstOut, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x00);
        assert_eq!(pixel_channel(&images, 3), 0x00);
    }

    // ── SrcIn ─────────────────────────────────────────────────────────────────

    /// SrcIn with fully opaque destination lets source through unchanged.
    #[test]
    fn src_in_opaque_dst_passes_src() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0xff);
        let src = rgba_pixel(0xCC, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::SrcIn, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0xCC);
    }

    /// SrcIn with fully transparent destination zeros the output.
    #[test]
    fn src_in_transparent_dst_clears_output() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0x00);
        let src = rgba_pixel(0xCC, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::SrcIn, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x00);
        assert_eq!(pixel_channel(&images, 3), 0x00);
    }

    // ── SrcOut ────────────────────────────────────────────────────────────────

    /// SrcOut with fully transparent destination passes source through.
    #[test]
    fn src_out_transparent_dst_passes_src() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0x00);
        let src = rgba_pixel(0xCC, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::SrcOut, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0xCC);
    }

    /// SrcOut with fully opaque destination clears the output.
    #[test]
    fn src_out_opaque_dst_clears_output() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0xff);
        let src = rgba_pixel(0xCC, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::SrcOut, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x00);
        assert_eq!(pixel_channel(&images, 3), 0x00);
    }

    // ── Xor ───────────────────────────────────────────────────────────────────

    /// Xor of identical images should produce a fully transparent result.
    #[test]
    fn xor_identical_images_cancels() {
        let img_a = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let img_b = rgba_pixel(0x80, 0x80, 0x80, 0xff);
        let mut images = vec![img_a, img_b];
        Composite::new(CompositeMethod::Xor, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(
            pixel_channel(&images, 3),
            0x00,
            "α should be 0 for Xor of identical opaque images"
        );
    }

    /// Xor of src-transparent dst preserves only the source shape.
    #[test]
    fn xor_transparent_dst_keeps_src() {
        let dst = rgba_pixel(0x00, 0x00, 0x00, 0x00);
        let src = rgba_pixel(0xCC, 0x00, 0x00, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Xor, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0xCC);
        assert_eq!(pixel_channel(&images, 3), 0xff);
    }

    // ── Multiply ─────────────────────────────────────────────────────────────

    /// Multiplying by full white (0xff) should leave the destination unchanged.
    #[test]
    fn multiply_by_white_is_identity() {
        let dst = rgb_pixel(0x80, 0x40, 0x20);
        let src = rgb_pixel(0xff, 0xff, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Multiply, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x80);
        assert_eq!(pixel_channel(&images, 1), 0x40);
        assert_eq!(pixel_channel(&images, 2), 0x20);
    }

    /// Multiplying by black (0x00) should produce black.
    #[test]
    fn multiply_by_black_gives_black() {
        let dst = rgb_pixel(0xff, 0xff, 0xff);
        let src = rgb_pixel(0x00, 0x00, 0x00);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Multiply, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x00);
        assert_eq!(pixel_channel(&images, 1), 0x00);
        assert_eq!(pixel_channel(&images, 2), 0x00);
    }

    /// Multiply result is always ≤ both inputs (darkening).
    #[test]
    fn multiply_is_darkening() {
        let dst = rgb_pixel(0xC0, 0x80, 0x40);
        let src = rgb_pixel(0x80, 0xC0, 0x80);
        let src_r = 0xC0_u8;
        let dst_r = 0x80_u8;
        let expected_r = ((f32::from(src_r) / 255.0) * (f32::from(dst_r) / 255.0) * 255.0).round() as u8;

        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Multiply, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        let result_r = pixel_channel(&images, 0);
        assert!(
            result_r <= src_r && result_r <= dst_r,
            "Multiply should be ≤ both inputs; got {result_r}"
        );
        assert!(
            (i16::from(result_r) - i16::from(expected_r)).abs() <= 1,
            "Expected ~{expected_r}, got {result_r}"
        );
    }

    // ── Screen ────────────────────────────────────────────────────────────────

    /// Screen with black source is identity.
    #[test]
    fn screen_by_black_is_identity() {
        let dst = rgb_pixel(0x80, 0x40, 0x20);
        let src = rgb_pixel(0x00, 0x00, 0x00);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Screen, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0x80);
        assert_eq!(pixel_channel(&images, 1), 0x40);
        assert_eq!(pixel_channel(&images, 2), 0x20);
    }

    /// Screen with white produces white.
    #[test]
    fn screen_by_white_gives_white() {
        let dst = rgb_pixel(0x40, 0x80, 0xC0);
        let src = rgb_pixel(0xff, 0xff, 0xff);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Screen, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        assert_eq!(pixel_channel(&images, 0), 0xff);
        assert_eq!(pixel_channel(&images, 1), 0xff);
        assert_eq!(pixel_channel(&images, 2), 0xff);
    }

    /// Screen result is always ≥ both inputs (brightening).
    #[test]
    fn screen_is_brightening() {
        let src_r = 0xC0_u8;
        let dst_r = 0x80_u8;
        let expected_r = (255.0
            - (1.0 - f32::from(src_r) / 255.0) * (1.0 - f32::from(dst_r) / 255.0) * 255.0)
            .round() as u8;

        let dst = rgb_pixel(dst_r, 0x80, 0x40);
        let src = rgb_pixel(src_r, 0xC0, 0x80);
        let mut images = vec![dst, src];
        Composite::new(CompositeMethod::Screen, (0, 0))
            .execute_multiple(&mut images)
            .unwrap();
        let result_r = pixel_channel(&images, 0);
        assert!(
            result_r >= src_r && result_r >= dst_r,
            "Screen should be ≥ both inputs; got {result_r}"
        );
        assert!(
            (i16::from(result_r) - i16::from(expected_r)).abs() <= 1,
            "Expected ~{expected_r}, got {result_r}"
        );
    }

    // ── Error paths ───────────────────────────────────────────────────────────

    /// Attempting to composite with fewer than two images is an error.
    #[test]
    fn error_on_single_image() {
        let img = rgba_pixel(0, 0, 0, 255);
        let mut images = vec![img];
        let result = Composite::new(CompositeMethod::Over, (0, 0)).execute_multiple(&mut images);
        assert!(result.is_err());
    }

    /// Mismatched bit depths must return an error.
    #[test]
    fn error_on_depth_mismatch() {
        use zune_image::image::Image;
        let dst = Image::from_fn(1usize, 1usize, ColorSpace::RGB, |_, _, p: &mut [u8; 4]| {
            p[0] = 0;
            p[1] = 0;
            p[2] = 0;
        });
        let src = Image::from_fn(1usize, 1usize, ColorSpace::RGB, |_, _, p: &mut [u16; 4]| {
            p[0] = 0;
            p[1] = 0;
            p[2] = 0;
        });
        let mut images = vec![dst, src];
        let result = Composite::new(CompositeMethod::Over, (0, 0)).execute_multiple(&mut images);
        assert!(result.is_err());
    }

    /// Alpha-channel methods on images without alpha should return an error.
    #[test]
    fn error_alpha_method_on_non_alpha_image() {
        let dst = rgb_pixel(0x80, 0x80, 0x80);
        let src = rgb_pixel(0x40, 0x40, 0x40);
        let mut images = vec![dst, src];
        let result = Composite::new(CompositeMethod::DstIn, (0, 0)).execute_multiple(&mut images);
        assert!(result.is_err(), "DstIn on RGB (no alpha) should error");
    }
}
