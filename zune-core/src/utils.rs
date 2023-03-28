//! Utilities required by multiple implementations
//! that help to do small things
use crate::bit_depth::ByteEndian;

mod sse;

/// scalar impl of big-endian to native endian
fn convert_be_to_ne_scalar(out: &mut [u8])
{
    out.chunks_exact_mut(2).for_each(|chunk| {
        let value: [u8; 2] = chunk.try_into().unwrap();
        let pix = u16::from_be_bytes(value);
        chunk.copy_from_slice(&pix.to_ne_bytes());
    });
}

/// Convert big endian to little endian for u16 samples
///
/// This is a no-op if the system is already in big-endian
///
/// # Arguments
///
/// * `out`:  The output array for which we will convert in place
/// * `use_sse4`:  Whether to use SSE intrinsics for conversion
///
fn convert_be_to_le_u16(out: &mut [u8], use_sse4: bool)
{
    #[cfg(feature = "std")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if use_sse4 && is_x86_feature_detected!("ssse3")
        {
            unsafe {
                return sse::convert_be_to_ne_sse4(out);
            }
        }
    }
    convert_be_to_ne_scalar(out)
}

/// Convert u16 big endian samples to target endian
///
/// # Arguments
///
/// * `sample`:  The raw samples assumed to be in big endian
/// * `endian`:  The target endianness for which to convert samples
/// * `use_intrinsics`:  Whether to use sse intrinsics to speed up
///
///
/// sample array is modified in place
///
#[inline]
pub fn convert_be_to_target_endian_u16(sample: &mut [u8], endian: ByteEndian, use_intrinsics: bool)
{
    // if target is BE no conversion
    if endian == ByteEndian::BE
    {
        return;
    }
    // if system is BE, no conversion
    // poor man's check
    if u16::from_be_bytes([234, 231]) == u16::from_ne_bytes([234, 231])
    {
        return;
    }
    // convert then
    convert_be_to_le_u16(sample, use_intrinsics);
}

/// Return true if the system is little endian
pub fn is_le() -> bool
{
    // see if le and be conversion return the same number
    u16::from_le_bytes([234, 231]) == u16::from_ne_bytes([234, 231])
}

/// Return true if the system is big endian
pub fn is_be() -> bool
{
    !is_le()
}
