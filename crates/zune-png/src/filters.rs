#![allow(unused_variables)]
//! A set of optimized filter functions for de-filtering png
//! scanlines.
//!

use crate::enums::FilterMethod;

mod sse4;

#[allow(clippy::manual_memcpy)]
pub fn handle_avg(
    prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize, use_sse4: bool
)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // use sse features where applicable
        if use_sse4
        {
            if components == 3
            {
                return sse4::defilter_avg_sse::<3>(prev_row, raw, current);
            }
            if components == 4
            {
                return sse4::defilter_avg_sse::<4>(prev_row, raw, current);
            }
            if components == 6
            {
                return sse4::defilter_avg_sse::<6>(prev_row, raw, current);
            }
            if components == 8
            {
                return sse4::defilter_avg_sse::<8>(prev_row, raw, current);
            }
        }
    }

    // no simd, so just do it the old fashioned way

    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i].wrapping_add(prev_row[i] >> 1);
    }
    // raw length is one row,so always keep it in check
    let end = current.len().min(raw.len()).min(prev_row.len());

    if components > 8
    {
        // optimizer hint to tell the compiler that we don't see this ever happening
        return;
    }

    for i in components..end
    {
        let a = current[i - components];
        let b = prev_row[i];

        // find average, with overflow handling
        // from standford bit-hacks.
        // This lets us keep the implementations using
        // 8 bits, hence easier to vectorize
        let c = (a & b) + ((a ^ b) >> 1);

        current[i] = raw[i].wrapping_add(c);
    }
}

#[allow(clippy::manual_memcpy)]
pub fn handle_sub(raw: &[u8], current: &mut [u8], components: usize, use_sse2: bool)
{
    if current.len() < components || raw.len() < components
    {
        return;
    }
    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if use_sse2
        {
            if components == 3
            {
                return sse4::de_filter_sub_sse2::<3>(raw, current);
            }
            if components == 4
            {
                return sse4::de_filter_sub_sse2::<4>(raw, current);
            }
            if components == 6
            {
                return sse4::de_filter_sub_sse2::<6>(raw, current);
            }
            if components == 8
            {
                return sse4::de_filter_sub_sse2::<8>(raw, current);
            }
        }
    }
    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i];
    }
    // raw length is one row,so always keep it in check
    let end = current.len().min(raw.len());

    for i in components..end
    {
        let a = current[i - components];
        current[i] = raw[i].wrapping_add(a);
    }
}

#[allow(clippy::manual_memcpy)]
pub fn handle_paeth(
    prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize, use_sse4: bool
)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if use_sse4
        {
            if components == 3
            {
                return sse4::de_filter_paeth_sse41::<3>(prev_row, raw, current);
            }
            if components == 4
            {
                return sse4::de_filter_paeth_sse41::<4>(prev_row, raw, current);
            }
            if components == 6
            {
                return sse4::de_filter_paeth_sse41::<6>(prev_row, raw, current);
            }
            if components == 8
            {
                return sse4::de_filter_paeth_sse41::<8>(prev_row, raw, current);
            }
        }
    }

    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i].wrapping_add(paeth(0, prev_row[i], 0));
    }
    // raw length is one row,so always keep it in check
    let end = current.len().min(raw.len()).min(prev_row.len());

    if components > 8
    {
        // optimizer hint to tell the CPU that we don't see this ever happening
        return;
    }

    for i in components..end
    {
        let paeth_res = paeth(
            current[i - components],
            prev_row[i],
            prev_row[i - components]
        );
        current[i] = raw[i].wrapping_add(paeth_res)
    }
}

pub fn handle_up(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    for ((filt, recon), up) in raw.iter().zip(current).zip(prev_row)
    {
        *recon = (*filt).wrapping_add(*up)
    }
}

/// Handle images with the first scanline as paeth scanline
///
/// Special in that the above row is treated as zero
#[allow(clippy::manual_memcpy)]
pub fn handle_paeth_first(raw: &[u8], current: &mut [u8], components: usize)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i];
    }
    // raw length is one row,so always keep it in check
    let end = current.len().min(raw.len());

    for i in components..end
    {
        let paeth_res = paeth(current[i - components], 0, 0);
        current[i] = raw[i].wrapping_add(paeth_res)
    }
}

/// Handle images with the fast scanline as an average scanline
///
/// The above row is treated as zero
#[allow(clippy::manual_memcpy)]
pub fn handle_avg_first(raw: &[u8], current: &mut [u8], components: usize)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i];
    }
    // raw length is one row,so always keep it in check
    let end = current.len().min(raw.len());

    for i in components..end
    {
        let avg = current[i - components] >> 1;
        current[i] = raw[i].wrapping_add(avg)
    }
}

#[inline(always)]
pub fn paeth(a: u8, b: u8, c: u8) -> u8
{
    let a = i16::from(a);
    let b = i16::from(b);
    let c = i16::from(c);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();

    if pa <= pb && pa <= pc
    {
        return a as u8;
    }
    if pb <= pc
    {
        return b as u8;
    }
    c as u8
}

pub fn choose_compression_filter(_previous_row: &[u8], _current_row: &[u8]) -> FilterMethod
{
    if _previous_row.is_empty()
    {
        // first row
        return FilterMethod::None;
    }
    return FilterMethod::None;
}

pub fn filter_scanline(input: &[u8], previous_row: &[u8], output: &mut [u8], filter: FilterMethod)
{
    let (filter_byte, filter_scanline) = output.split_at_mut(1);
    // add
    filter_byte[0] = filter.to_int();

    match filter
    {
        FilterMethod::None =>
        {
            // copy input to output
            filter_scanline.copy_from_slice(input);
        }
        _ => unreachable!("Unexpected input")
    }
}
