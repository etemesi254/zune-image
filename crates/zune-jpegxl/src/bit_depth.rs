/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Bit depth operations
//!
//! Apparently jpeg xl has to handle  different bit
//! depths with different bits, from 8--14 use u/i16, from
//! 14 and above use i32.
//!
//! This module provides those capabilities via a trait
//! and the ability to handle all interesting things that
//! come with how they chose to support jpeg xl
//!
//!
use core::fmt::Debug;
use core::ops::{Add, BitXor, Shr, Sub};

use crate::bit_writer::{encode_hybrid_unit_000, BitWriter};
use crate::encoder::PrefixCode;

/// This trait encapsulates shared functions for the
/// encoder which are generically initialized depending
/// on the decoder type.
///
/// Each supported bit depth initializes this with varied
/// values which are later used generically in the encoder
pub(crate) trait JxlBitEncoder {
    /// Number of input core::mem::size_of(depth_type);
    /// E.g 1 for u8,2 for u16's
    const K_INPUT_BYTES: usize;
    /// Minimum raw length for bit types
    const KMIN_RAW_LENGTH: [u8; 20];
    /// Maximum raw length for bit types
    const KMAX_RAW_LENGTH: [u8; 20];

    /// This encapsulates a signed pixel type
    /// used to represent an image pixel
    /// it can either be i16 or i32 depending on the bit
    /// depth
    type Pixel: Copy
        + Sub<Output = Self::Pixel>
        + Add<Output = Self::Pixel>
        + BitXor<Output = Self::Pixel>
        + PartialOrd
        + Into<i32>
        + TryInto<usize>
        + Shr<u8, Output = Self::Pixel>
        + From<u8>
        + From<i16>
        + TryFrom<u16>
        + TryFrom<i32>
        + Default
        + Debug;

    /// This represents an unsigned pixel type
    /// used to represent an image pixel
    ///
    /// Can either be u16 or u32 depending on
    /// bit depth
    type Upixel: Copy
        + Sub<Output = Self::Upixel>
        + Add<Output = Self::Upixel>
        + BitXor<Output = Self::Upixel>
        + PartialOrd
        + Into<u32>;

    /// Return a default [`Upixel`] type
    fn upixel_default() -> Self::Upixel;
    /// Return 0 as [`Pixel`] type
    fn pixel_zero() -> Self::Pixel;
    /// Return 0 as [`Upixel`] type
    fn upixel_zero() -> Self::Upixel;
    /// Convert a [`u32`] to a [`Upixel`] type using a
    /// lossy cast
    fn from_u32(value: u32) -> Self::Upixel;
    /// Return the maximum bits per sample for this
    /// implementation
    fn max_encoded_bits_per_sample(&self) -> usize;

    /// Encode a chunk writing it to output
    ///
    /// # Arguments 
    ///
    /// * `residual`:  
    /// * `n`:  Maximum position for which we encode no more residuals
    /// * `skip`:  Initial position
    /// * `code`:  Prefix code to use for encoding residuals
    /// * `output`:  The output to which we will write encoded residuals from
    ///  n to skip
    ///
    #[rustfmt::skip]
    #[allow(clippy::needless_range_loop)]
    fn encode_chunk(
        &self, residual: &[Self::Upixel], n: usize, skip: usize, code: &PrefixCode,
        output: &mut BitWriter,
    )
    {
        for ix in skip..n
        {
            let (mut token, mut nbits, mut bits) = (0, 0, 0);

            encode_hybrid_unit_000(residual[ix].into(), &mut token, &mut nbits, &mut bits);

            let token_x = token as usize;
            let nbits_x = (nbits & 0xFF) as u8;

            let bits = u64::from(code.raw_bits[token_x])
                | u64::from(bits << code.raw_nbits[token_x]);

            let nbits = code.raw_nbits[token_x] + nbits_x;

            output.put_bits(nbits, bits);
        }
    }
    /// Return number of bit symbols
    /// needed to encode the bits
    fn num_symbols(&self, _: bool) -> usize;
}

pub(crate) struct UpTo8Bits();

impl JxlBitEncoder for UpTo8Bits {
    // Here we can fit up to 9 extra bits + 7 Huffman bits in a u16; for all other
    // symbols, we could actually go up to 8 Huffman bits as we have at most 8
    // extra bits; however, the SIMD bit merging logic for AVX2 assumes that no
    // Huffman length is 8 or more, so we cap at 8 anyway. Last symbol is used for
    // LZ77 lengths and has no limitations except allowing to represent 32 symbols
    // in total.
    const K_INPUT_BYTES: usize = 1;

    const KMIN_RAW_LENGTH: [u8; 20] = [0; 20];
    const KMAX_RAW_LENGTH: [u8; 20] = [
        7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 10, 255, 255, 255, 255, 255, 255, 255, 255
    ];

    type Pixel = i16;
    type Upixel = u16;

    #[inline(always)]
    fn upixel_default() -> Self::Upixel {
        u16::default()
    }
    fn pixel_zero() -> Self::Pixel {
        0
    }

    fn upixel_zero() -> Self::Upixel {
        0
    }

    fn from_u32(value: u32) -> Self::Upixel {
        value as Self::Upixel
    }

    fn max_encoded_bits_per_sample(&self) -> usize {
        16
    }

    fn num_symbols(&self, doing_ycocg: bool) -> usize {
        if doing_ycocg {
            8 + 3
        } else {
            8 + 2
        }
    }
}

pub struct MoreThan14Bits();

impl JxlBitEncoder for MoreThan14Bits {
    const K_INPUT_BYTES: usize = 2;
    // Force LZ77 symbols to have at least 8 bits, and raw symbols 13 to 18 to
    // have exactly 8, and no other symbol to have 8 or more. This ensures that
    // the representation for (13, 14), (15, 16), (17, 18) is identical up to one
    // bit.
    const KMIN_RAW_LENGTH: [u8; 20] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 8, 8, 8, 8, 8, 7];

    const KMAX_RAW_LENGTH: [u8; 20] = [7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 8, 8, 8, 8, 8, 8, 10];

    type Pixel = i32;
    type Upixel = u32;

    fn upixel_default() -> Self::Upixel {
        u32::default()
    }

    fn pixel_zero() -> Self::Pixel {
        0
    }

    fn upixel_zero() -> Self::Upixel {
        0
    }

    fn from_u32(value: u32) -> Self::Upixel {
        value
    }

    fn max_encoded_bits_per_sample(&self) -> usize {
        24
    }
    fn num_symbols(&self, _: bool) -> usize {
        19
    }
}
