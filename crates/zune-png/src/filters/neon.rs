#![cfg(target_arch = "aarch64")]

use core::arch::aarch64::*;

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn de_filter_sub_neon<const SIZE: usize>(raw: &[u8], current: &mut [u8]) {
    let len = raw.len();
    let full_chunks = len / SIZE;
    let remainder = len % SIZE;

    let mut a = vdupq_n_u8(0);

    // Chunks where we have >= 16 bytes from chunk start in both `raw` and `current`.
    let safe_chunks = if len >= 16 { (len - 16) / SIZE } else { 0 };

    // Safe interior: safe slicing guarantees we don't exceed the slice bounds.
    for i in 0..safe_chunks {
        let start = i * SIZE;

        let raw_slice = &raw[start..];
        let out_slice = &mut current[start..];

        unsafe {
            let d = vld1q_u8(raw_slice.as_ptr());
            a = vaddq_u8(d, a);
            vst1q_u8(out_slice.as_mut_ptr(), a);
        }
    }

    // Remaining chunks (last full chunk(s) + remainder): use staging buffer
    let mut zero = [0u8; 16];

    for i in safe_chunks..full_chunks {
        let start = i * SIZE;
        let end = start + SIZE;

        zero[..SIZE].copy_from_slice(&raw[start..end]);

        unsafe {
            let d = vld1q_u8(zero.as_ptr());
            a = vaddq_u8(d, a);
            vst1q_u8(zero.as_mut_ptr(), a);
        }

        current[start..end].copy_from_slice(&zero[..SIZE]);
    }

    // Remainder (< SIZE bytes)
    if remainder > 0 {
        zero = [0u8; 16];
        let start = full_chunks * SIZE;

        zero[..remainder].copy_from_slice(&raw[start..]);

        unsafe {
            let d = vld1q_u8(zero.as_ptr());
            a = vaddq_u8(d, a);
            vst1q_u8(zero.as_mut_ptr(), a);
        }

        current[start..].copy_from_slice(&zero[..remainder]);
    }
}

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn defilter_avg_neon<const SIZE: usize>(
    prev_row: &[u8], raw: &[u8], current: &mut [u8],
) {
    let mut x = [0u8; 16];
    let mut y = [0u8; 16];
    let mut a = vdupq_n_u8(0);

    for ((prev, raw_chunk), current_row) in prev_row
        .chunks_exact(SIZE)
        .zip(raw.chunks_exact(SIZE))
        .zip(current.chunks_exact_mut(SIZE))
    {
        x[0..SIZE].copy_from_slice(raw_chunk);
        y[0..SIZE].copy_from_slice(prev);

        let b = vld1q_u8(y.as_ptr());
        let d = vld1q_u8(x.as_ptr());

        // NEON MAGIC: vhaddq_u8 natively computes floor((a+b)/2)!
        // This avoids the XOR rounding fix required in SSE.
        let avg = vhaddq_u8(a, b);

        a = vaddq_u8(d, avg);
        vst1q_u8(x.as_mut_ptr(), a);

        current_row.copy_from_slice(&x[0..SIZE]);
    }
}


