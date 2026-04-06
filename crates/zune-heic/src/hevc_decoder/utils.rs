use alloc::borrow::Cow;

/// Quickly searches for the HEVC Emulation Prevention Byte sequence (`0x00 0x00 0x03`)
/// using SWAR (SIMD Within A Register) for high performance.
#[inline(always)]
fn find_epb(src: &[u8], start: usize) -> Option<usize> {
    // Standard SWAR masks for detecting a 0x00 byte inside a u64
    const MAGIC_SUB: u64 = 0x0101010101010101;
    const MAGIC_MASK: u64 = 0x8080808080808080;

    let len = src.len();
    let mut i = start;

    // --- SWAR Fast Path ---
    // Scan 8 bytes at a time
    while i + 8 <= len {
        // Safe: we know the slice has at least 8 bytes left
        let chunk = u64::from_le_bytes(src[i..i + 8].try_into().unwrap());

        // If the chunk contains at least one zero byte
        if (chunk.wrapping_sub(MAGIC_SUB) & !chunk & MAGIC_MASK) != 0 {
            // Find exactly where the `00 00 03` sequence is.
            // We clamp the check to `len - 2` to prevent out-of-bounds on `j+2`.
            let check_end = std::cmp::min(i + 8, len.saturating_sub(2));
            for j in i..check_end {
                if src[j] == 0 && src[j + 1] == 0 && src[j + 2] == 3 {
                    return Some(j);
                }
            }
        }
        // Advance by 8. (Boundary crossing 00 00 | 03 is safely caught because
        // the first chunk contains the 00s, triggering the inner check up to i+8)
        i += 8;
    }

    // --- Tail Scan ---
    // Clean up any remaining bytes (< 8 bytes) at the end of the payload
    while i < len.saturating_sub(2) {
        if src[i] == 0 && src[i + 1] == 0 && src[i + 2] == 3 {
            return Some(i);
        }
        i += 1;
    }

    None
}

/// Strips Emulation Prevention Bytes from an RBSP payload.
///
/// Returns `Cow::Borrowed` if no EPB is found (zero-allocation).
/// Returns `Cow::Owned` with the cleaned vector if EPBs are removed.
pub fn extract_rbsp(src: &[u8]) -> Cow<'_, [u8]> {
    // 1. Initial Fast Scan
    if let Some(first_epb) = find_epb(src, 0) {
        // 2. The Slow Path (Allocation Required)
        // We know we must copy. Preallocate the max possible size to avoid reallocations.
        let mut dst = Vec::with_capacity(src.len());

        // Bulk copy the clean prefix
        dst.extend_from_slice(&src[..first_epb]);
        let mut si = first_epb;

        // 3. The Bulk-Copy Cleaning Loop
        while si < src.len() {
            // We know `si` points directly to a `0x00 0x00 0x03` here
            if si + 2 < src.len() && src[si] == 0 && src[si + 1] == 0 && src[si + 2] == 3 {
                dst.extend_from_slice(&[0, 0]); // Keep the two 0x00s
                si += 3; // Skip the 0x03!
            } else {
                break; // Failsafe, should rarely hit unless malformed NAL ends prematurely
            }

            // Fast-forward to the NEXT EPB using our SWAR scanner
            let next_epb = find_epb(src, si).unwrap_or(src.len());

            // Bulk copy all the clean data between the last EPB and the next one!
            dst.extend_from_slice(&src[si..next_epb]);
            si = next_epb;
        }

        Cow::Owned(dst)
    } else {
        // 4. The Zero-Allocation Fast Path
        Cow::Borrowed(src)
    }
}


pub(crate) const Y_CF: i16 = 16384;
pub(crate) const CR_CF: i16 = 22970;
pub(crate) const CB_CF: i16 = 29032;
pub(crate) const C_G_CR_COEF_1: i16 = -11700;
pub(crate) const C_G_CB_COEF_2: i16 = -5638;
pub(crate) const YUV_PREC: i16 = 14;
// Rounding const for YUV -> RGB conversion: floating equivalent 0.499(9).
pub(crate) const YUV_RND: i16 = (1 << (YUV_PREC - 1)) - 1;

fn clamp(a: i32) -> u8 {
    a.clamp(0, 255) as u8
}
/// Convert a batch of 16 YCbCr pixels to RGB.
///
/// This is a scalar fallback implementation used during pixel conversion.
///
/// # Parameters
///
/// - `BGRA`: If true, output is written as BGRA order instead of RGB.
/// - `y`, `cb`, `cr`: Input YUV components (16 pixels)
/// - `output`: Destination buffer
/// - `pos`: Current write offset (updated after writing)
///
/// # Panics
///
/// Panics if output buffer is too small.

pub fn ycbcr_to_rgb_inner_16_scalar<const BGRA: bool>(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize
) {
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 48 elements
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    for ((&y, (cb, cr)), out) in y
        .iter()
        .zip(cb.iter().zip(cr.iter()))
        .zip(opt.chunks_exact_mut(3))
    {
        let cr = cr - 128;
        let cb = cb - 128;

        let y0 = i32::from(y) * i32::from(Y_CF) + i32::from(YUV_RND);

        let r = (y0 + i32::from(cr) * i32::from(CR_CF)) >> YUV_PREC;
        let g = (y0
            + i32::from(cr) * i32::from(C_G_CR_COEF_1)
            + i32::from(cb) * i32::from(C_G_CB_COEF_2))
            >> YUV_PREC;
        let b = (y0 + i32::from(cb) * i32::from(CB_CF)) >> YUV_PREC;

        if BGRA {
            out[0] = clamp(b);
            out[1] = clamp(g);
            out[2] = clamp(r);
        } else {
            out[0] = clamp(r);
            out[1] = clamp(g);
            out[2] = clamp(b);
        }
    }

    // Increment pos
    *pos += 48;
}