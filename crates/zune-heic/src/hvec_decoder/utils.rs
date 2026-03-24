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
