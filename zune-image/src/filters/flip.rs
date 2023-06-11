/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bit_depth::BitType;
use zune_imageprocs::flip::{flip, horizontal_flip};

use crate::errors::{ImageErrors, ImageOperationsErrors};
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct Flip;

impl Flip
{
    pub fn new() -> Flip
    {
        Self::default()
    }
}
impl OperationsTrait for Flip
{
    fn get_name(&self) -> &'static str
    {
        "Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let depth = image.get_depth();

        for inp in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    flip(inp.reinterpret_as_mut::<u8>()?);
                }
                BitType::U16 =>
                {
                    flip(inp.reinterpret_as_mut::<u16>()?);
                }
                BitType::F32 =>
                {
                    flip(inp.reinterpret_as_mut::<f32>()?);
                }
                _ => todo!()
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

/// Rearrange the pixels up side down
#[derive(Default)]
pub struct HorizontalFlip;

impl HorizontalFlip
{
    pub fn new() -> HorizontalFlip
    {
        Self::default()
    }
}

impl OperationsTrait for HorizontalFlip
{
    fn get_name(&self) -> &'static str
    {
        "Horizontal Flip"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        let depth = image.get_depth();
        let width = image.get_dimensions().0;

        for inp in image.get_channels_mut(false)
        {
            match depth.bit_type()
            {
                BitType::U8 =>
                {
                    horizontal_flip(inp.reinterpret_as_mut::<u8>()?, width);
                }
                BitType::U16 =>
                {
                    horizontal_flip(inp.reinterpret_as_mut::<u16>()?, width);
                }
                BitType::F32 =>
                {
                    horizontal_flip(inp.reinterpret_as_mut::<f32>()?, width);
                }
                _ =>
                {
                    return Err(ImageOperationsErrors::UnsupportedType(
                        self.get_name(),
                        depth.bit_type()
                    )
                    .into())
                }
            }
        }

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}
