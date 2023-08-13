/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::rotate::rotate_180;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

pub struct Rotate {
    angle: f32
}

impl Rotate {
    pub fn new(angle: f32) -> Rotate {
        Rotate { angle }
    }
}

impl OperationsTrait for Rotate {
    fn get_name(&self) -> &'static str {
        "Rotate"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let im_type = image.metadata.depth.bit_type();

        let (width, _) = image.get_dimensions();

        if self.angle != 180.0 {
            panic!("Not implemented");
        }

        for channel in image.get_channels_mut(false) {
            match im_type {
                BitType::U8 => {
                    if self.angle == 180.0 {
                        rotate_180::<u8>(channel.reinterpret_as_mut().unwrap(), width);
                    }
                }
                BitType::U16 => {
                    if self.angle == 180.0 {
                        rotate_180::<u16>(channel.reinterpret_as_mut().unwrap(), width);
                    }
                }
                BitType::F32 => {
                    if self.angle == 180.0 {
                        rotate_180::<f32>(channel.reinterpret_as_mut().unwrap(), width);
                    }
                }
                _ => todo!("Implement")
            };
        }

        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
