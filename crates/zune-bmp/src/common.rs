/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::colorspace::ColorSpace;

#[derive(Debug, Eq, PartialEq)]
pub enum BmpCompression
{
    RGB,
    RLE8,
    RLE4,
    BITFIELDS,
    Unknown
}

impl BmpCompression
{
    pub fn from_u32(num: u32) -> Option<BmpCompression>
    {
        match num
        {
            0 => Some(BmpCompression::RGB),
            1 => Some(BmpCompression::RLE8),
            2 => Some(BmpCompression::RLE4),
            3 => Some(BmpCompression::BITFIELDS),
            _ => None
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum BmpPixelFormat
{
    None,
    // unknown
    ABGR,
    OBGR,
    BGRA,
    BGRO,
    ARGB,
    ORGB,
    RGBA,
    RGB0,
    RGB555,
    RGB565,
    RGB444,
    PAL8,
    GRAY8,
    RGB
}

impl BmpPixelFormat
{
    pub fn num_components(&self, is_alpha: bool) -> usize
    {
        match self
        {
            BmpPixelFormat::None => 0,
            BmpPixelFormat::ABGR => 4,
            BmpPixelFormat::OBGR => 4,
            BmpPixelFormat::BGRA => 4,
            BmpPixelFormat::BGRO => 4,
            BmpPixelFormat::ARGB => 4,
            BmpPixelFormat::ORGB => 4,
            BmpPixelFormat::RGBA => 4,
            BmpPixelFormat::RGB0 => 4,
            BmpPixelFormat::RGB555 => 3,
            BmpPixelFormat::RGB565 => 3,
            BmpPixelFormat::RGB444 => 3,
            BmpPixelFormat::PAL8 =>
            {
                if is_alpha
                {
                    4
                }
                else
                {
                    3
                }
            }
            BmpPixelFormat::GRAY8 => 1,
            BmpPixelFormat::RGB => 3
        }
    }
    pub fn into_colorspace(self, is_alpha: bool) -> ColorSpace
    {
        match self
        {
            BmpPixelFormat::None => ColorSpace::Unknown,
            BmpPixelFormat::ABGR => ColorSpace::RGBA,
            BmpPixelFormat::OBGR => ColorSpace::BGR,
            BmpPixelFormat::BGRA => ColorSpace::RGB,
            BmpPixelFormat::BGRO => ColorSpace::BGR,
            BmpPixelFormat::ARGB => ColorSpace::RGBA,
            BmpPixelFormat::ORGB => ColorSpace::RGB,
            BmpPixelFormat::RGBA => ColorSpace::RGBA,
            BmpPixelFormat::RGB0 => ColorSpace::RGBA,
            BmpPixelFormat::RGB555 => ColorSpace::RGB,
            BmpPixelFormat::RGB565 => ColorSpace::RGB,
            BmpPixelFormat::RGB444 => ColorSpace::RGB,
            BmpPixelFormat::PAL8 =>
            {
                if is_alpha
                {
                    ColorSpace::RGBA
                }
                else
                {
                    ColorSpace::RGB
                }
            }
            BmpPixelFormat::GRAY8 => ColorSpace::Luma,
            BmpPixelFormat::RGB => ColorSpace::RGB
        }
    }
}
