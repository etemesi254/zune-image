//!Miscellaneous stuff
#![allow(dead_code)]

use std::fmt;

use zune_core::bytestream::ZByteReader;

use crate::errors::DecodeErrors;

/// Start of baseline DCT Huffman coding

pub const START_OF_FRAME_BASE: u16 = 0xffc0;

/// Start of another frame

pub const START_OF_FRAME_EXT_SEQ: u16 = 0xffc1;

/// Start of progressive DCT encoding

pub const START_OF_FRAME_PROG_DCT: u16 = 0xffc2;

/// Start of Lossless sequential Huffman coding

pub const START_OF_FRAME_LOS_SEQ: u16 = 0xffc3;

/// Start of extended sequential DCT arithmetic coding

pub const START_OF_FRAME_EXT_AR: u16 = 0xffc9;

/// Start of Progressive DCT arithmetic coding

pub const START_OF_FRAME_PROG_DCT_AR: u16 = 0xffca;

/// Start of Lossless sequential Arithmetic coding

pub const START_OF_FRAME_LOS_SEQ_AR: u16 = 0xffcb;

/// Undo run length encoding of coefficients by placing them in natural order
#[rustfmt::skip]
pub const UN_ZIGZAG: [usize; 64 + 16] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
    // Prevent overflowing
    63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63, 63, 63, 63
];

/// Align data to a 16 byte boundary
#[repr(align(16))]
#[derive(Clone)]

pub struct Aligned16<T: ?Sized>(pub T);

impl<T> Default for Aligned16<T>
where
    T: Default,
{
    fn default() -> Self
    {
        Aligned16(T::default())
    }
}

/// Align data to a 32 byte boundary
#[repr(align(32))]
#[derive(Clone)]
pub struct Aligned32<T: ?Sized>(pub T);

impl<T> Default for Aligned32<T>
where
    T: Default,
{
    fn default() -> Self
    {
        Aligned32(T::default())
    }
}

/// Markers that identify different Start of Image markers
/// They identify the type of encoding and whether the file use lossy(DCT) or
/// lossless compression and whether we use Huffman or arithmetic coding schemes
#[derive(Eq, PartialEq, Copy, Clone)]
#[allow(clippy::upper_case_acronyms)]

pub enum SOFMarkers
{
    /// Baseline DCT markers
    BaselineDct,
    /// SOF_1 Extended sequential DCT,Huffman coding
    ExtendedSequentialHuffman,
    /// Progressive DCT, Huffman coding
    ProgressiveDctHuffman,
    /// Lossless (sequential), huffman coding,
    LosslessHuffman,
    /// Extended sequential DEC, arithmetic coding
    ExtendedSequentialDctArithmetic,
    /// Progressive DCT, arithmetic coding,
    ProgressiveDctArithmetic,
    /// Lossless ( sequential), arithmetic coding
    LosslessArithmetic,
}

impl Default for SOFMarkers
{
    fn default() -> Self
    {
        Self::BaselineDct
    }
}

impl SOFMarkers
{
    /// Check if a certain marker is sequential DCT or not

    pub fn is_sequential_dct(self) -> bool
    {
        matches!(
            self,
            Self::BaselineDct
                | Self::ExtendedSequentialHuffman
                | Self::ExtendedSequentialDctArithmetic
        )
    }

    /// Check if a marker is a Lossles type or not

    pub fn is_lossless(self) -> bool
    {
        matches!(self, Self::LosslessHuffman | Self::LosslessArithmetic)
    }

    /// Check whether a marker is a progressive marker or not

    pub fn is_progressive(self) -> bool
    {
        matches!(
            self,
            Self::ProgressiveDctHuffman | Self::ProgressiveDctArithmetic
        )
    }

    /// Create a marker from an integer

    pub fn from_int(int: u16) -> Option<SOFMarkers>
    {
        match int
        {
            START_OF_FRAME_BASE => Some(Self::BaselineDct),
            START_OF_FRAME_PROG_DCT => Some(Self::ProgressiveDctHuffman),
            START_OF_FRAME_PROG_DCT_AR => Some(Self::ProgressiveDctArithmetic),
            START_OF_FRAME_LOS_SEQ => Some(Self::LosslessHuffman),
            START_OF_FRAME_LOS_SEQ_AR => Some(Self::LosslessArithmetic),
            START_OF_FRAME_EXT_SEQ => Some(Self::ExtendedSequentialHuffman),
            START_OF_FRAME_EXT_AR => Some(Self::ExtendedSequentialDctArithmetic),
            _ => None,
        }
    }
}

impl fmt::Debug for SOFMarkers
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result
    {
        match &self
        {
            Self::BaselineDct => write!(f, "Baseline DCT"),
            Self::ExtendedSequentialHuffman =>
            {
                write!(f, "Extended sequential DCT, Huffman Coding")
            }
            Self::ProgressiveDctHuffman => write!(f, "Progressive DCT,Huffman Encoding"),
            Self::LosslessHuffman => write!(f, "Lossless (sequential) Huffman encoding"),
            Self::ExtendedSequentialDctArithmetic =>
            {
                write!(f, "Extended sequential DCT, arithmetic coding")
            }
            Self::ProgressiveDctArithmetic => write!(f, "Progressive DCT, arithmetic coding"),
            Self::LosslessArithmetic => write!(f, "Lossless (sequential) arithmetic coding"),
        }
    }
}

/// Read `buf.len()*2` data from the underlying `u8` buffer and convert it into
/// u16, and store it into `buf`
///
/// # Arguments
/// - reader: A mutable reference to the underlying reader.
/// - buf: A mutable reference to a slice containing u16's
#[inline]

pub fn read_u16_into(reader: &mut ZByteReader, buf: &mut [u16]) -> Result<(), DecodeErrors>
{
    for i in buf
    {
        *i = reader.get_u16_be_err()?;
    }

    Ok(())
}
