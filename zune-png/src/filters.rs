//! A set of optimized filter functions for de-filtering png
//! scanlines.
//!
//!
//! There exist two types of filter functions here,
//! special filter functions for the first scanline which has special conditions
//! and normal filter functions,
//!
//! The special first scanlines have a suffix _first on them and are only called
//! for the first scanline.

mod sse4;

#[allow(clippy::manual_memcpy)]
pub fn handle_avg(prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // use sse features where applicable
        if is_x86_feature_detected!("sse2")
        {
            if components == 3
            {
                return crate::filters::sse4::defilter_avg3_sse(prev_row, raw, current);
            }
            if components == 4
            {
                return crate::filters::sse4::defilter_avg4_sse(prev_row, raw, current);
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
        let a = u16::from(current[i - components]);
        let b = u16::from(prev_row[i]);

        let c = (((a + b) >> 1) & 0xFF) as u8;

        current[i] = raw[i].wrapping_add(c);
    }
}

#[allow(clippy::manual_memcpy)]
pub fn handle_sub(raw: &[u8], current: &mut [u8], components: usize)
{
    if current.len() < components || raw.len() < components
    {
        return;
    }
    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse2")
        {
            if components == 3
            {
                return crate::filters::sse4::de_filter_sub3_sse2(raw, current);
            }
            if components == 4
            {
                return crate::filters::sse4::de_filter_sub4_sse2(raw, current);
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
pub fn handle_paeth(prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize)
{
    if raw.len() < components || current.len() < components
    {
        return;
    }

    #[cfg(feature = "sse")]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse4.1")
        {
            if components == 3
            {
                return crate::filters::sse4::de_filter_paeth3_sse41(prev_row, raw, current);
            }
            if components == 4
            {
                return crate::filters::sse4::de_filter_paeth4_sse41(prev_row, raw, current);
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
