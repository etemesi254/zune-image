mod trc;

pub use trc::TransferFunction;
use zune_core::bit_depth::BitType;
use zune_core::log::warn;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::utils::execute_on;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ConversionType {
    GammaToLinear,
    LinearToGamma
}
pub struct ImageTransfer {
    transfer_function: TransferFunction,
    conversion_type:   ConversionType
}
impl ImageTransfer {
    pub fn new(transfer_function: TransferFunction, conversion_type: ConversionType) -> Self {
        ImageTransfer {
            transfer_function,
            conversion_type
        }
    }
}
impl OperationsTrait for ImageTransfer {
    fn name(&self) -> &'static str {
        "ImageTransfer"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        match self.conversion_type {
            ConversionType::GammaToLinear => {
                if image.metadata().is_linear() {
                    warn!("Image is already in linear colorspace, no operation will occur");
                    return Ok(());
                }
            }
            ConversionType::LinearToGamma => {
                if !image.metadata().is_linear() {
                    warn!("Image transfer characteristics are in gamma,no operation will occur");
                    return Ok(());
                }
            }
        }
        let depth = image.depth();
        let eight_bit_lut = if image.depth().bit_type() == BitType::U8 {
            match self.conversion_type {
                ConversionType::GammaToLinear => Some(build_8_bit_gamma_to_linear_lut_table(
                    self.transfer_function
                )),
                ConversionType::LinearToGamma => Some(build_8_bit_linear_to_gamma_lut_table(
                    self.transfer_function
                ))
            }
        } else {
            None
        };
        let sixteen_bit_lut = if image.depth().bit_type() == BitType::U16 {
            match self.conversion_type {
                ConversionType::GammaToLinear => Some(build_sixteen_bit_gamma_to_linear_lut_table(
                    self.transfer_function
                )),
                ConversionType::LinearToGamma => Some(build_sixteen_bit_linear_to_gamma_lut_table(
                    self.transfer_function
                ))
            }
        } else {
            None
        };
        let conversion_function = |input: &mut Channel| -> Result<(), ImageErrors> {
            match depth.bit_type() {
                BitType::U8 => {
                    if let Some(lut_table) = eight_bit_lut.as_ref() {
                        let channel = input.reinterpret_as_mut::<u8>()?;
                        channel.iter_mut().for_each(|x| *x = lut_table[*x as usize]);
                    } else {
                        panic!("LUT table was not made");
                    }
                }
                BitType::U16 => {
                    if let Some(lut_table) = sixteen_bit_lut.as_ref() {
                        let channel = input.reinterpret_as_mut::<u16>()?;
                        channel.iter_mut().for_each(|x| *x = lut_table[*x as usize]);
                    } else {
                        panic!("SIXTEEN_BIT table was not made");
                    }
                }
                BitType::F32 => {
                    // F32 is just all of them slowly by slowly
                    let channel = input.reinterpret_as_mut::<f32>()?;
                    match self.conversion_type {
                        ConversionType::GammaToLinear => {
                            channel
                                .iter_mut()
                                .for_each(|x| *x = self.transfer_function.linearize(*x));
                        }
                        ConversionType::LinearToGamma => {
                            channel
                                .iter_mut()
                                .for_each(|x| *x = self.transfer_function.gamma(*x));
                        }
                    }
                }
                d => return Err(ImageErrors::ImageOperationNotImplemented(self.name(), d))
            }
            Ok(())
        };

        execute_on(conversion_function, image, true)?;

        match self.conversion_type {
            ConversionType::GammaToLinear => image.metadata_mut().set_linear(true),
            ConversionType::LinearToGamma => image.metadata_mut().set_linear(false)
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8]
    }
}
pub fn build_8_bit_gamma_to_linear_lut_table(transfer_function: TransferFunction) -> [u8; 256] {
    let mut lut_table = [0u8; 256];
    for (i, item) in lut_table.iter_mut().enumerate() {
        *item = (transfer_function.linearize(i as f32 * (1. / 255.0)) * 255.).min(255.) as u8;
    }
    lut_table
}

pub fn build_8_bit_linear_to_gamma_lut_table(transfer_function: TransferFunction) -> [u8; 256] {
    let mut lut_table = [0u8; 256];
    for (i, item) in lut_table.iter_mut().enumerate() {
        *item = (transfer_function.gamma(i as f32 * (1. / 255.0)) * 255.).min(255.) as u8;
    }
    lut_table
}

fn build_sixteen_bit_gamma_to_linear_lut_table(transfer_function: TransferFunction) -> Vec<u16> {
    let max_colors = (1 << 16) - 1;
    let mut lut_table = vec![0u16; max_colors + 1];
    for (i, item) in lut_table.iter_mut().enumerate() {
        *item = (transfer_function.linearize(i as f32 * (1. / max_colors as f32))
            * max_colors as f32)
            .min(max_colors as f32) as u16;
    }
    lut_table
}
fn build_sixteen_bit_linear_to_gamma_lut_table(transfer_function: TransferFunction) -> Vec<u16> {
    let max_colors = (1 << 16) - 1;
    let mut lut_table = vec![0u16; max_colors + 1];
    for (i, item) in lut_table.iter_mut().enumerate() {
        *item = (transfer_function.gamma(i as f32 * (1. / max_colors as f32)) * max_colors as f32)
            .min(max_colors as f32) as u16;
    }
    lut_table
}
