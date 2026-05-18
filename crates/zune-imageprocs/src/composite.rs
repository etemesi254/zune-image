use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

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

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
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
                "Composite requires at least two images",
            ));
        }

        let src_image = images.pop().unwrap();
        let dst_image = images.last_mut().unwrap();

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

        let dims = if let Some(gravity) = self.gravity {
            calculate_gravity(&src_image, dst_image, gravity)
        } else if let Some(geometry) = self.geometry {
            geometry
        } else {
            unreachable!()
        };
        let (start_x, start_y) = dims;
        let (src_width, src_height) = src_image.dimensions();
        let colorspace = dst_image.colorspace();
        let has_alpha = colorspace.has_alpha();

        match dst_image.depth().bit_type() {
            BitType::U8 => {
                // Safely extract all immutable source channels upfront
                let src_channels: Vec<&[u8]> = src_image.frames_ref()[0] // Assuming single frame for brevity
                    .channels_ref(colorspace, false)
                    .into_iter()
                    .map(|ch| ch.reinterpret_as().unwrap())
                    .collect();

                dst_image.par_process_regions::<u8, _>(false, |region| {
                    composite_region::<u8>(
                        region,
                        &src_channels,
                        src_width,
                        src_height,
                        start_x,
                        start_y,
                        self.composite_method,
                        has_alpha,
                    );
                })?;
            }
            BitType::U16 => {
                // Safely extract all immutable source channels upfront
                let src_channels: Vec<&[u16]> =
                    src_image.frames_ref()[0] // Assuming single frame for brevity
                        .channels_ref(colorspace, false)
                        .into_iter()
                        .map(|ch| ch.reinterpret_as().unwrap())
                        .collect();

                dst_image.par_process_regions::<_, _>(false, |region| {
                    composite_region(
                        region,
                        &src_channels,
                        src_width,
                        src_height,
                        start_x,
                        start_y,
                        self.composite_method,
                        has_alpha,
                    );
                })?;
            }
            BitType::F32 => {
                // Safely extract all immutable source channels upfront
                let src_channels: Vec<&[f32]> =
                    src_image.frames_ref()[0] // Assuming single frame for brevity
                        .channels_ref(colorspace, false)
                        .into_iter()
                        .map(|ch| ch.reinterpret_as().unwrap())
                        .collect();

                dst_image.par_process_regions::<_, _>(false, |region| {
                    composite_region(
                        region,
                        &src_channels,
                        src_width,
                        src_height,
                        start_x,
                        start_y,
                        self.composite_method,
                        has_alpha,
                    );
                })?;
            }
            d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
        }

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_region<T>(
    region: &mut PlanarRegionMut<'_, T>, src_channels: &[&[T]], src_width: usize,
    src_height: usize, start_x: usize, start_y: usize, method: CompositeMethod, has_alpha: bool,
) where
    T: Copy + NumOps<T>,
    f32: From<T>,
{
    // 1. Calculate Y-axis intersection
    let r_start_y = region.y_offset;
    let r_end_y = region.y_offset + region.height;

    let overlap_start_y = r_start_y.max(start_y);
    let overlap_end_y = r_end_y.min(start_y + src_height);

    if overlap_start_y >= overlap_end_y {
        return;
    }

    // 2. Calculate X-axis intersection
    let overlap_start_x = start_x.min(region.width);
    let overlap_end_x = (start_x + src_width).min(region.width);
    let overlap_width = overlap_end_x.saturating_sub(overlap_start_x);

    if overlap_width == 0 {
        return;
    }

    // Determine channel indexes
    let num_color_channels =
        if has_alpha { region.channels.len() - 1 } else { region.channels.len() };
    let alpha_idx = if has_alpha { Some(region.channels.len() - 1) } else { None };

    let inv_max = 1.0 / f32::from(T::MAX_VAL);
    let max_val = f32::from(T::MAX_VAL);

    // Local macro to cleanly inline the spatial bounding box iteration
    // without triggering borrow checker conflicts or runtime branching.
    macro_rules! process_overlap {
        (|$s_idx:ident, $d_idx:ident| $body:block) => {
            for y in overlap_start_y..overlap_end_y {
                let src_y = y - start_y;
                let dst_local_y = y - r_start_y;

                let src_row_start = src_y * src_width;
                let dst_row_start = dst_local_y * region.width + overlap_start_x;

                for x in 0..overlap_width {
                    let $s_idx = src_row_start + x;
                    let $d_idx = dst_row_start + x;
                    $body
                }
            }
        };
    }

    // 3. Process the overlapping box based on the specified method
    match method {
        CompositeMethod::Dst => {
            // No-op: Destination remains entirely unchanged
        }
        CompositeMethod::Src => {
            process_overlap!(|s_idx, d_idx| {
                for c in 0..region.channels.len() {
                    region.channels[c][d_idx] = src_channels[c][s_idx];
                }
            });
        }
        CompositeMethod::Over => {
            if let Some(a_idx) = alpha_idx {
                process_overlap!(|s_idx, d_idx| {
                    let a_src = (f32::from(src_channels[a_idx][s_idx]) * inv_max).clamp(0.0, 1.0);
                    let a_dst =
                        (f32::from(region.channels[a_idx][d_idx]) * inv_max).clamp(0.0, 1.0);

                    // Blend colors: C_out = a_src * C_src + (1 - a_src) * C_dst
                    for c in 0..num_color_channels {
                        let c_src = f32::from(src_channels[c][s_idx]);
                        let c_dst = f32::from(region.channels[c][d_idx]);
                        let out = a_src * c_src + (1.0 - a_src) * c_dst;
                        region.channels[c][d_idx] = T::from_f32(out.clamp(0.0, max_val).round());
                    }

                    // Blend alpha: A_out = A_src + A_dst * (1 - A_src)
                    let out_alpha = a_src + a_dst * (1.0 - a_src);
                    region.channels[a_idx][d_idx] =
                        T::from_f32((out_alpha.clamp(0.0, 1.0) * max_val).round());
                });
            } else {
                // If there is no alpha, `Over` behaves exactly like `Src`
                process_overlap!(|s_idx, d_idx| {
                    for c in 0..num_color_channels {
                        region.channels[c][d_idx] = src_channels[c][s_idx];
                    }
                });
            }
        }
        CompositeMethod::DstIn
        | CompositeMethod::DstOut
        | CompositeMethod::SrcIn
        | CompositeMethod::SrcOut
        | CompositeMethod::Xor => {
            if let Some(a_idx) = alpha_idx {
                process_overlap!(|s_idx, d_idx| {
                    let a_src = (f32::from(src_channels[a_idx][s_idx]) * inv_max).clamp(0.0, 1.0);
                    let a_dst =
                        (f32::from(region.channels[a_idx][d_idx]) * inv_max).clamp(0.0, 1.0);

                    // Consolidate the math factors for all Alpha-Masked operators
                    let (color_src_factor, color_dst_factor, out_alpha) = match method {
                        CompositeMethod::DstIn => (0.0, a_src, a_dst * a_src),
                        CompositeMethod::DstOut => (0.0, 1.0 - a_src, a_dst * (1.0 - a_src)),
                        CompositeMethod::SrcIn => (a_dst, 0.0, a_src * a_dst),
                        CompositeMethod::SrcOut => (1.0 - a_dst, 0.0, a_src * (1.0 - a_dst)),
                        CompositeMethod::Xor => (
                            1.0 - a_dst,
                            1.0 - a_src,
                            a_src + a_dst - 2.0 * a_src * a_dst,
                        ),
                        _ => unreachable!(),
                    };

                    for c in 0..num_color_channels {
                        let c_src = f32::from(src_channels[c][s_idx]) * inv_max;
                        let c_dst = f32::from(region.channels[c][d_idx]) * inv_max;
                        let out = c_src * color_src_factor + c_dst * color_dst_factor;
                        region.channels[c][d_idx] =
                            T::from_f32((out.clamp(0.0, 1.0) * max_val).round());
                    }

                    region.channels[a_idx][d_idx] =
                        T::from_f32((out_alpha.clamp(0.0, 1.0) * max_val).round());
                });
            }
        }
        CompositeMethod::Multiply => {
            process_overlap!(|s_idx, d_idx| {
                for c in 0..num_color_channels {
                    let s = f32::from(src_channels[c][s_idx]) * inv_max;
                    let d = f32::from(region.channels[c][d_idx]) * inv_max;
                    let out = s * d;
                    region.channels[c][d_idx] =
                        T::from_f32((out.clamp(0.0, 1.0) * max_val).round());
                }
            });
        }
        CompositeMethod::Screen => {
            process_overlap!(|s_idx, d_idx| {
                for c in 0..num_color_channels {
                    let s = f32::from(src_channels[c][s_idx]) * inv_max;
                    let d = f32::from(region.channels[c][d_idx]) * inv_max;
                    let out = 1.0 - (1.0 - s) * (1.0 - d);
                    region.channels[c][d_idx] =
                        T::from_f32((out.clamp(0.0, 1.0) * max_val).round());
                }
            });
        }
    }
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
        let expected_r =
            ((f32::from(src_r) / 255.0) * (f32::from(dst_r) / 255.0) * 255.0).round() as u8;

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
}
