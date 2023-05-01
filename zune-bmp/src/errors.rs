/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use alloc::string::String;
use core::fmt::{Debug, Formatter};

pub enum BmpDecoderErrors
{
    InvalidMagicBytes,
    TooSmallBuffer(usize, usize),
    GenericStatic(&'static str),
    Generic(String),
    TooLargeDimensions(&'static str, usize, usize)
}

impl Debug for BmpDecoderErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result
    {
        match self
        {
            Self::InvalidMagicBytes =>
            {
                writeln!(f, "Invalid magic bytes, file does not start with BM")
            }
            Self::TooSmallBuffer(expected, found) =>
            {
                writeln!(
                    f,
                    "Too small of buffer, expected {} but found {}",
                    expected, found
                )
            }
            Self::GenericStatic(header) =>
            {
                writeln!(f, "{}", header)
            }
            Self::TooLargeDimensions(dimension, expected, found) =>
            {
                writeln!(
                    f,
                    "Too large dimensions for {dimension} , {found} exceeds {expected}"
                )
            }
            Self::Generic(message) =>
            {
                writeln!(f, "{}", message)
            }
        }
    }
}
