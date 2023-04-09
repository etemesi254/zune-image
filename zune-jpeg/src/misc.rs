//!Miscellaneous stuff
#![allow(dead_code)]

use alloc::format;
use core::cmp::max;
use core::fmt;

use zune_core::bytestream::ZByteReader;

use crate::components::SampleRatios;
use crate::errors::DecodeErrors;
use crate::JpegDecoder;

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
    T: Default
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
    T: Default
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
    LosslessArithmetic
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
            _ => None
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
            Self::LosslessArithmetic => write!(f, "Lossless (sequential) arithmetic coding")
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

/// Set up component parameters.
///
/// This modifies the components in place setting up details needed by other
/// parts fo the decoder.
pub(crate) fn setup_component_params(img: &mut JpegDecoder) -> Result<(), DecodeErrors>
{
    let img_width = img.width();
    let img_height = img.height();

    for component in &mut img.components
    {
        // compute interleaved image info
        // h_max contains the maximum horizontal component
        img.h_max = max(img.h_max, component.horizontal_sample);
        // v_max contains the maximum vertical component
        img.v_max = max(img.v_max, component.vertical_sample);
        img.mcu_width = img.h_max * 8;
        img.mcu_height = img.v_max * 8;
        // Number of MCU's per width
        img.mcu_x = (usize::from(img.info.width) + img.mcu_width - 1) / img.mcu_width;
        // Number of MCU's per height
        img.mcu_y = (usize::from(img.info.height) + img.mcu_height - 1) / img.mcu_height;

        if img.h_max != 1 || img.v_max != 1
        {
            // interleaved images have horizontal and vertical sampling factors
            // not equal to 1.
            img.is_interleaved = true;
        }
        // Extract quantization tables from the arrays into components
        let qt_table = *img.qt_tables[component.quantization_table_number as usize]
            .as_ref()
            .ok_or_else(|| {
                DecodeErrors::DqtError(format!(
                    "No quantization table for component {:?}",
                    component.component_id
                ))
            })?;

        let x = (usize::from(img_width) * component.horizontal_sample + img.h_max - 1) / img.h_max;
        let y = (usize::from(img_height) * component.horizontal_sample + img.h_max - 1) / img.v_max;
        component.x = x;
        component.w2 = img.mcu_x * component.horizontal_sample * 8;
        // probably not needed. :)
        component.y = y;
        component.quantization_table = qt_table;
        // initially stride contains its horizontal sub-sampling
        component.width_stride *= img.mcu_x * 8;
    }
    if img.is_interleaved
        && img.components[0].horizontal_sample == 1
        && img.components[0].vertical_sample == 1
    {
        return Err(DecodeErrors::FormatStatic(
            "Unsupported unsampled Y component with sampled Cb / Cr components"
        ));
    }

    // delete quantization tables, we'll extract them from the components when
    // needed
    img.qt_tables = [None, None, None, None];

    Ok(())
}

///Calculate number of fill bytes added to the end of a JPEG image
/// to fill the image
///
/// JPEG usually inserts padding bytes if the image width cannot be evenly divided into
/// 8 , 16 or 32 chunks depending on the sub sampling ratio. So given a sub-sampling ratio,
/// and the actual width, this calculates the padded bytes that were added to the image
///
///  # Params
/// -actual_width: Actual width of the image
/// -sub_sample: Sub sampling factor of the image
///
/// # Returns
/// The padded width, this is how long the width is for a particular image
pub fn calculate_padded_width(actual_width: usize, sub_sample: SampleRatios) -> usize
{
    match sub_sample
    {
        SampleRatios::None | SampleRatios::V =>
        {
            // None+V sends one MCU row, so that's a simple calculation
            ((actual_width + 7) / 8) * 8
        }
        SampleRatios::H | SampleRatios::HV =>
        {
            // sends two rows, width can be expanded by up to 15 more bytes
            ((actual_width + 15) / 16) * 16
        }
    }
}
