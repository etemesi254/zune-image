#![cfg(feature = "cms")]

use moxcms::{ColorProfile, Layout, TransformOptions};
use zune_core::bit_depth::BitType;
use zune_core::colorspace::ColorSpace;
use zune_core::log::{info, trace};
use zune_image::errors::ImageErrors;
use zune_image::frame::Frame;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
pub enum ColorProfiles {
    sRGB,
    AdobeRgb,
    DisplayP3,
    Bt2020
}

/// ColorTransform
///
/// Convert between one ICC color profile to another
///
/// The input color profile is specified by the image's ICC
/// profile
#[derive(Debug, Clone, Copy)]
pub struct ColorTransform {
    color: ColorProfiles
}

impl ColorTransform {
    pub fn new(color_profiles: ColorProfiles) -> Self {
        Self {
            color: color_profiles
        }
    }
}
impl OperationsTrait for ColorTransform {
    fn name(&self) -> &'static str {
        "hdr to sdr"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        if let Some(icc_chunk) = image.metadata().icc_chunk() {
            trace!("Image with ICC chunk");
            let color_profile = match ColorProfile::new_from_slice(icc_chunk) {
                Ok(profile) => profile,
                Err(e) => {
                    return Err(ImageErrors::GenericString(e.to_string()));
                }
            };

            let dest_color_profile = match self.color {
                ColorProfiles::sRGB => ColorProfile::new_srgb(),
                ColorProfiles::AdobeRgb => ColorProfile::new_adobe_rgb(),
                ColorProfiles::DisplayP3 => ColorProfile::new_display_p3(),
                ColorProfiles::Bt2020 => ColorProfile::new_bt2020()
            };
            let img_depth = image.depth();

            let (w, h) = image.dimensions();
            let colorspace = image.colorspace();
            let size = w * h * colorspace.num_components();

            // map colorspace to layout
            let layout_value = match colorspace {
                ColorSpace::RGB => Layout::Rgb,
                ColorSpace::RGBA => Layout::Rgba,
                ColorSpace::Luma => Layout::Gray,
                _ => {
                    return Err(ImageErrors::GenericStr(
                        "Unsupported colorspace for transform"
                    ))
                }
            };
            match img_depth.bit_type() {
                BitType::U8 => {
                    // make the transform once per image
                    let transform = color_profile
                        .create_transform_8bit(
                            layout_value,
                            &dest_color_profile,
                            layout_value,
                            TransformOptions::default()
                        )
                        .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                    // input output storage
                    let mut input_interleaved = vec![0_u8; size];
                    let mut output_interleaved = vec![0_u8; size];

                    // iterate over all image frames applying the transforms
                    for frame in image.frames_mut() {
                        // flatten the buffer
                        let bytes_written = frame.flatten_into(&mut input_interleaved)?;

                        transform
                            .transform(
                                &input_interleaved[..bytes_written],
                                &mut output_interleaved[..bytes_written]
                            )
                            .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                        // store our output now, de-interleaving
                        let new_frame = Frame::from_u8(
                            &output_interleaved[..bytes_written],
                            colorspace,
                            frame.numerator(),
                            frame.denominator()
                        );

                        *frame = new_frame;
                    }
                }
                BitType::U16 => {
                    let transform = color_profile
                        .create_transform_16bit(
                            layout_value,
                            &dest_color_profile,
                            layout_value,
                            TransformOptions::default()
                        )
                        .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                    let mut input_interleaved = vec![0_u16; size];
                    let mut output_interleaved = vec![0_u16; size];

                    // iterate over all image frames applying the transforms
                    for frame in image.frames_mut() {
                        // flatten the buffer
                        let bytes_written = frame.flatten_into(&mut input_interleaved)?;

                        transform
                            .transform(
                                &input_interleaved[..bytes_written],
                                &mut output_interleaved[..bytes_written]
                            )
                            .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                        // store our output now, de-interleaving
                        let new_frame = Frame::from_u16(
                            &output_interleaved[..bytes_written],
                            colorspace,
                            frame.numerator(),
                            frame.denominator()
                        );

                        *frame = new_frame;
                    }
                }
                BitType::F32 => {
                    let transform = color_profile
                        .create_transform_f32(
                            layout_value,
                            &dest_color_profile,
                            layout_value,
                            TransformOptions::default()
                        )
                        .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                    let mut input_interleaved = vec![0_f32; size];
                    let mut output_interleaved = vec![0_f32; size];

                    // iterate over all image frames applying the transforms
                    for frame in image.frames_mut() {
                        // flatten the buffer
                        let bytes_written = frame.flatten_into(&mut input_interleaved)?;

                        transform
                            .transform(
                                &input_interleaved[..bytes_written],
                                &mut output_interleaved[..bytes_written]
                            )
                            .map_err(|e| ImageErrors::GenericString(e.to_string()))?;

                        // store our output now, de-interleaving
                        let new_frame = Frame::from_f32(
                            &output_interleaved[..bytes_written],
                            colorspace,
                            frame.numerator(),
                            frame.denominator()
                        );

                        *frame = new_frame;
                    }
                }

                _ => {
                    return Err(ImageErrors::ImageOperationNotImplemented(
                        self.name(),
                        image.depth().bit_type()
                    ))
                }
            }
        } else {
            info!("No ICC chunk present, no transform will be done");
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U8, BitType::U16]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_cms() {
        // TODO: Add a valid test case
    }
}
