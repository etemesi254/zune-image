use crate::traits::NumOps;
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

pub struct HaldClut;

impl Default for HaldClut {
    fn default() -> Self {
        Self::new()
    }
}

impl HaldClut {
    #[must_use]
    pub fn new() -> Self {
        HaldClut
    }
}

impl OperationsTrait for HaldClut {
    fn name(&self) -> &'static str {
        "Hald-CLUT"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Hald-CLUT requires multiple images; call via execute_multiple",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Gamma
    }
    #[allow(clippy::too_many_lines)]
    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.len() < 2 {
            return Err(ImageErrors::GenericStr(
                "Hald-CLUT requires at least two images (CLUT + Target)",
            ));
        }

        let mut clut_img = images.pop().unwrap();
        let target_img = images.last_mut().unwrap();

        // Validate CLUT dimensions
        let (clut_w, clut_h) = clut_img.dimensions();
        if clut_w != clut_h {
            return Err(ImageErrors::GenericStr(
                "Hald-CLUT image must be a perfect square",
            ));
        }
        let level = (clut_w as f32).cbrt().round() as usize;
        if level * level * level != clut_w {
            return Err(ImageErrors::GenericStr(
                "Invalid Hald-CLUT dimensions (e.g., 512x512 for level 8)",
            ));
        }

        // Handle Colorspace Conversions
        let original_cs = target_img.colorspace();
        let is_rgb_variant = matches!(
            original_cs,
            ColorSpace::RGB
                | ColorSpace::RGBA
                | ColorSpace::ARGB
                | ColorSpace::BGR
                | ColorSpace::BGRA
        );

        if !is_rgb_variant {
            target_img.convert_color(ColorSpace::RGBA)?;
        }

        let clut_depth = clut_img.depth();
        let target_depth = target_img.depth();
        let new_depth = clut_depth.bit_type().max(target_depth.bit_type());

        let working_cs = target_img.colorspace();

        // Ensure CLUT has the exact same color layout so our zip loops match
        if clut_img.colorspace() != working_cs {
            clut_img.convert_color(working_cs)?;
        }
        if target_depth != clut_depth {
            // map to the next best thing
            trace!("Image depths differ converting the most appropriate depth of depth of {:?}",new_depth.to_depth());

            // convert both
            clut_img.convert_depth(new_depth.to_depth())?;
            target_img.convert_depth(new_depth.to_depth())?;
        }


        for (target_frame, clut_frame) in target_img
            .frames_mut()
            .iter_mut()
            .zip(clut_img.frames_ref())
        {
            // Safely split the channel array into multiple mutable references
            let t_channels = target_frame.channels_mut(working_cs, false);

            let (r_target, g_target, b_target) =
                if working_cs == ColorSpace::BGR || working_cs == ColorSpace::BGRA {
                    let (b_ch, rest) = t_channels.split_first_mut().unwrap();
                    let (g_ch, rest2) = rest.split_first_mut().unwrap();
                    let (r_ch, _) = rest2.split_first_mut().unwrap();
                    (r_ch, g_ch, b_ch)
                } else {
                    let (r_ch, rest) = t_channels.split_first_mut().unwrap();
                    let (g_ch, rest2) = rest.split_first_mut().unwrap();
                    let (b_ch, _) = rest2.split_first_mut().unwrap();
                    (r_ch, g_ch, b_ch)
                };

            let c_channels = clut_frame.channels_ref(working_cs, false);

            let (r_clut, g_clut, b_clut) =
                if working_cs == ColorSpace::BGR || working_cs == ColorSpace::BGRA {
                    (&c_channels[2], &c_channels[1], &c_channels[0])
                } else {
                    (&c_channels[0], &c_channels[1], &c_channels[2])
                };

            match new_depth {
                BitType::U8 => apply_hald_clut::<u8>(
                    r_target.reinterpret_as_mut()?,
                    g_target.reinterpret_as_mut()?,
                    b_target.reinterpret_as_mut()?,
                    r_clut.reinterpret_as()?,
                    g_clut.reinterpret_as()?,
                    b_clut.reinterpret_as()?,
                    level,
                    clut_w,
                ),
                BitType::U16 => apply_hald_clut::<u16>(
                    r_target.reinterpret_as_mut()?,
                    g_target.reinterpret_as_mut()?,
                    b_target.reinterpret_as_mut()?,
                    r_clut.reinterpret_as()?,
                    g_clut.reinterpret_as()?,
                    b_clut.reinterpret_as()?,
                    level,
                    clut_w,
                ),
                BitType::F32 => apply_hald_clut::<f32>(
                    r_target.reinterpret_as_mut()?,
                    g_target.reinterpret_as_mut()?,
                    b_target.reinterpret_as_mut()?,
                    r_clut.reinterpret_as()?,
                    g_clut.reinterpret_as()?,
                    b_clut.reinterpret_as()?,
                    level,
                    clut_w,
                ),
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d)),
            }
        }

        // convert back
        if target_img.depth() != target_depth {
            target_img.convert_depth(target_depth)?;
        }
        // Restore the original colorspace if we temporarily upgraded it
        if !is_rgb_variant {
            target_img.convert_color(original_cs)?;
        }

        Ok(())
    }
}

/// Applies a Hald-CLUT mapping to target RGB channels.
///
/// Uses Trilinear Interpolation for smooth, professional-grade color mapping.
/// Applies a Hald-CLUT mapping to target RGB channels.
///
/// Uses Trilinear Interpolation for smooth, professional-grade color mapping.
#[allow(clippy::too_many_arguments)]
pub fn apply_hald_clut<T>(
    r_target: &mut [T], g_target: &mut [T], b_target: &mut [T], r_clut: &[T], g_clut: &[T],
    b_clut: &[T], level: usize, clut_w: usize,
) where
    T: Copy + NumOps<T>,
{
    // dim is the number of nodes per axis. For 512px, dim = 64.
    let dim = level * level;
    let max_idx = (dim - 1) as f32;
    let max_val = T::max_val().to_f32();

    // Standard Hald-CLUT Mapping:
    // The image is a linear scan where Red changes fastest, then Green, then Blue.
    let get_clut_index = |ri: usize, gi: usize, bi: usize| -> usize {
        // 1D index in the cube
        let p = bi * (dim * dim) + gi * dim + ri;

        // Map 1D index to 2D image coordinates (x, y)
        let x = p % clut_w;
        let y = p / clut_w;

        y * clut_w + x
    };

    let lerp = |a: f32, b: f32, t: f32| -> f32 { a + (b - a) * t };

    for ((r_out, g_out), b_out) in r_target
        .iter_mut()
        .zip(g_target.iter_mut())
        .zip(b_target.iter_mut())
    {
        // 1. Normalize and scale to CLUT axis (0.0 .. 63.0)

        let rf = (r_out.to_f32() / max_val * max_idx).clamp(0.0, max_idx);
        let gf = (g_out.to_f32() / max_val * max_idx).clamp(0.0, max_idx);
        let bf = (b_out.to_f32() / max_val * max_idx).clamp(0.0, max_idx);

        let r0 = rf as usize;
        let g0 = gf as usize;
        let b0 = bf as usize;

        let r1 = (r0 + 1).min(dim - 1);
        let g1 = (g0 + 1).min(dim - 1);
        let b1 = (b0 + 1).min(dim - 1);

        let dr = rf - r0 as f32;
        let dg = gf - g0 as f32;
        let db = bf - b0 as f32;

        // 2. Sample 8 corners
        let i000 = get_clut_index(r0, g0, b0);
        let i100 = get_clut_index(r1, g0, b0);

        let i010 = get_clut_index(r0, g1, b0);
        let i110 = get_clut_index(r1, g1, b0);

        let i001 = get_clut_index(r0, g0, b1);
        let i101 = get_clut_index(r1, g0, b1);

        let i011 = get_clut_index(r0, g1, b1);
        let i111 = get_clut_index(r1, g1, b1);

        // 3. Trilinear Interpolation
        for (out_val, clut_chan) in [r_out, g_out, b_out]
            .iter_mut()
            .zip([r_clut, g_clut, b_clut])
        {
            let c00 = lerp(clut_chan[i000].to_f32(), clut_chan[i100].to_f32(), dr);
            let c10 = lerp(clut_chan[i010].to_f32(), clut_chan[i110].to_f32(), dr);
            let c01 = lerp(clut_chan[i001].to_f32(), clut_chan[i101].to_f32(), dr);
            let c11 = lerp(clut_chan[i011].to_f32(), clut_chan[i111].to_f32(), dr);

            let c0 = lerp(c00, c10, dg);
            let c1 = lerp(c01, c11, dg);

            let res = lerp(c0, c1, db);
            let value = res.clamp(0.0, max_val);
            **out_val = T::from_f32(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zune_core::colorspace::ColorSpace;
    use zune_image::image::Image;
    use zune_image::traits::OperationsTrait;

    /// Test that the operation rejects stacks with fewer than 2 images
    #[test]
    fn test_hald_clut_requires_two_images() {
        let target = Image::from_fn::<u8, _>(10, 10, ColorSpace::RGB, |_, _, px| {
            px[0] = 255;
            px[1] = 0;
            px[2] = 0;
        });

        let mut stack = vec![target];
        let op = HaldClut::new();

        let result = op.execute_multiple(&mut stack);
        assert!(
            result.is_err(),
            "Should error when only 1 image is in the stack"
        );
    }

    /// Test that the operation rejects invalid CLUT dimensions
    #[test]
    fn test_hald_clut_invalid_dimensions() {
        let target = Image::from_fn::<u8, _>(10, 10, ColorSpace::RGB, |_, _, px| {
            px.fill(255);
        });

        // 100x100 is not a valid Hald-CLUT cube root (Level 4 is 64x64, Level 5 is 125x125)
        let invalid_clut = Image::from_fn::<u8, _>(100, 100, ColorSpace::RGB, |_, _, px| {
            px.fill(128);
        });

        let mut stack = vec![target, invalid_clut];
        let op = HaldClut::new();

        let result = op.execute_multiple(&mut stack);
        assert!(result.is_err(), "Should error on invalid CLUT dimensions");
    }

    /// Test with a Level 1 CLUT (1x1 pixel).
    /// A 1x1 CLUT will force every single pixel in the target image to become that 1 color.
    #[test]
    fn test_hald_clut_solid_color() {
        let target = Image::from_fn::<u8, _>(10, 10, ColorSpace::RGB, |_, _, px| {
            px[0] = 255; // Solid Red
            px[1] = 0;
            px[2] = 0;
        });

        let clut = Image::from_fn::<u8, _>(1, 1, ColorSpace::RGB, |_, _, px| {
            px[0] = 0;
            px[1] = 0;
            px[2] = 255; // Solid Blue
        });

        let mut stack = vec![target, clut];
        HaldClut::new().execute_multiple(&mut stack).unwrap();

        // The stack should now contain 1 image (the target)
        assert_eq!(stack.len(), 1);

        // Every pixel should now be Blue
        let out_img = &stack[0];
        let r_chan = out_img.channels_ref(false)[0]
            .reinterpret_as::<u8>()
            .unwrap();
        let g_chan = out_img.channels_ref(false)[1]
            .reinterpret_as::<u8>()
            .unwrap();
        let b_chan = out_img.channels_ref(false)[2]
            .reinterpret_as::<u8>()
            .unwrap();

        assert_eq!(r_chan[0], 0);
        assert_eq!(g_chan[0], 0);
        assert_eq!(b_chan[0], 255);
    }
}
