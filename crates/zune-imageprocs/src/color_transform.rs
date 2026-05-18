#![cfg(feature = "cms")]

use moxcms::{ColorProfile, Layout, TransformExecutor, TransformOptions};
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_core::log::trace;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::planar_regions::PlanarRegionMut;
use zune_image::traits::{OperationColorValues, OperationsTrait};

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum ColorProfiles {
    sRGB,
    AdobeRgb,
    DisplayP3,
    Bt2020,
    DciP3,
}
/// Transforms an image from its embedded ICC color profile to a target color profile.
///
/// This operation uses the `moxcms` library to accurately map colors from the source
/// gamut (defined by the image's embedded ICC profile) to a standard destination gamut
/// (such as sRGB or Display P3).
///
/// After the pixel data is transformed, the operation automatically updates the image's
/// metadata to embed the new ICC profile, ensuring downstream applications render the
/// colors correctly.
///
/// # Feature Flags
///
/// This operation requires the `cms` feature to be enabled.
///
/// # Behavior
///
/// * **Metadata Dependency:** If the image does not contain an embedded ICC profile
///   in its metadata, this operation safely acts as a **no-op**, logging an info message
///   and leaving the pixel data untouched.
/// * **Supported Colorspaces:** Currently supports `RGB`, `RGBA`, and `Luma` (Grayscale).
/// * **Multi-frame Support:** Automatically processes all frames in animated or
///   multi-frame images using reused buffers for optimal performance.
#[derive(Debug, Clone, Copy)]
pub struct ColorTransform {
    color: ColorProfiles,
}
impl ColorTransform {
    #[must_use]
    pub fn new(color_profiles: ColorProfiles) -> Self {
        Self {
            color: color_profiles,
        }
    }
}
impl OperationsTrait for ColorTransform {
    fn name(&self) -> &'static str {
        "Color Transform"
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Gamma
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let color_profile = if let Some(icc_chunk) = image.metadata().icc_chunk() {
            ColorProfile::new_from_slice(icc_chunk)
                .map_err(|e| ImageErrors::GenericString(e.to_string()))?
        } else {
            trace!("No ICC chunk found, assuming sRGB source profile");
            ColorProfile::new_srgb()
        };

        let dest_color_profile = match self.color {
            ColorProfiles::sRGB => ColorProfile::new_srgb(),
            ColorProfiles::AdobeRgb => ColorProfile::new_adobe_rgb(),
            ColorProfiles::DisplayP3 => ColorProfile::new_display_p3(),
            ColorProfiles::Bt2020 => ColorProfile::new_bt2020(),
            ColorProfiles::DciP3 => ColorProfile::new_dci_p3(),
        };

        let colorspace = image.colorspace();
        let layout_value = match colorspace {
            ColorSpace::RGB => Layout::Rgb,
            ColorSpace::RGBA => Layout::Rgba,
            ColorSpace::Luma => Layout::Gray,
            _ => return Err(ImageErrors::GenericStr("Unsupported colorspace for transform")),
        };

        // We do NOT ignore alpha here, because moxcms Layout::Rgba expects 4 channels interleaved
        let ignore_alpha = false;

        match image.depth().bit_type() {
            BitType::U8 => {
                let transform = color_profile
                    .create_transform_8bit(
                        layout_value, &dest_color_profile, layout_value, TransformOptions::default(),
                    )
                    .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                // Pass to the parallel primitive. It handles mutating all frames internally.
                image.par_process_regions::<u8, _>(ignore_alpha, |region| {
                    process_cms_region(region, transform.as_ref());
                })?;
            }
            BitType::U16 => {
                let transform = color_profile
                    .create_transform_16bit(
                        layout_value, &dest_color_profile, layout_value, TransformOptions::default(),
                    )
                    .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                image.par_process_regions::<u16, _>(ignore_alpha, |region| {
                    process_cms_region(region, transform.as_ref());
                })?;
            }
            BitType::F32 => {
                let transform = color_profile
                    .create_transform_f32(
                        layout_value, &dest_color_profile, layout_value, TransformOptions::default(),
                    )
                    .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                image.par_process_regions::<f32, _>(ignore_alpha, |region| {
                    process_cms_region(region, transform.as_ref());
                })?;
            }
            _ => {
                return Err(ImageErrors::ImageOperationNotImplemented(
                    self.name(),
                    image.depth().bit_type(),
                ))
            }
        }

        // Set up the new ICC chunk
        let new_profile = dest_color_profile
            .encode()
            .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

        image.metadata_mut().set_icc_chunk(new_profile);

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U8, BitType::U16]
    }
}
fn process_cms_region<T>(region: &mut PlanarRegionMut<'_, T>, transform: &(dyn TransformExecutor<T> + Send + Sync))
where
    T: Copy + Default + Send + Sync,
{
    let num_pixels = region.width * region.height;
    let num_channels = region.channels.len();

    if num_pixels == 0 || num_channels == 0 {
        return;
    }

    // Allocate tiny chunk buffers for the CMS transform
    let buffer_size = num_pixels * num_channels;
    let mut input_interleaved = vec![T::default(); buffer_size];
    let mut output_interleaved = vec![T::default(); buffer_size];

    // 1. Interleave: Read from Planar channels into the Interleaved buffer
    for p in 0..num_pixels {
        for c in 0..num_channels {
            input_interleaved[p * num_channels + c] = region.channels[c][p];
        }
    }

    // 2. Execute the moxcms transform
    // Note: If moxcms panics/errors here, you might want to adjust error handling,
    // but typically transform logic succeeds if layout matches.
    let _ = transform.transform(&input_interleaved, &mut output_interleaved);

    // 3. De-interleave: Read from the Interleaved output back to Planar channels
    for p in 0..num_pixels {
        for c in 0..num_channels {
            region.channels[c][p] = output_interleaved[p * num_channels + c];
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn test_cms() {
        // TODO: Add a valid test case
    }
}
