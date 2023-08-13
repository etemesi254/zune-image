/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![allow(clippy::upper_case_acronyms)]

pub const PSD_IDENTIFIER_BE: u32 = 0x38425053;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ColorModes {
    Bitmap = 0,
    Grayscale = 1,
    IndexedColor = 2,
    RGB = 3,
    CYMK = 4,
    MultiChannel = 7,
    DuoTone = 8,
    LabColor = 9
}

impl ColorModes {
    pub fn from_int(int: u16) -> Option<ColorModes> {
        use crate::constants::ColorModes::{
            Bitmap, DuoTone, Grayscale, IndexedColor, LabColor, CYMK, RGB
        };

        match int {
            0 => Some(Bitmap),
            1 => Some(Grayscale),
            2 => Some(IndexedColor),
            3 => Some(RGB),
            4 => Some(CYMK),
            7 => Some(DuoTone),
            9 => Some(LabColor),
            _ => None
        }
    }
}

#[derive(Copy, Clone)]
pub enum CompressionMethod {
    NoCompression = 0,
    RLE = 1
}

impl CompressionMethod {
    pub fn from_int(int: u16) -> Option<CompressionMethod> {
        match int {
            0 => Some(Self::NoCompression),
            1 => Some(Self::RLE),
            _ => None
        }
    }
}
