#![cfg(target_arch = "aarch64")]

use core::arch::aarch64::*;

#[inline]
#[target_feature(enable = "neon")]
pub unsafe fn de_filter_sub_neon<const SIZE: usize>(raw: &[u8], current: &mut [u8]) {
    let mut zero = [0u8; 16];
    let mut a = vdupq_n_u8(0);

    for (raw_chunk, out_chunk) in raw.chunks_exact(SIZE).zip(current.chunks_exact_mut(SIZE)) {
        zero[0..SIZE].copy_from_slice(raw_chunk);

        let d = vld1q_u8(zero.as_ptr());
        a = vaddq_u8(d, a);
        vst1q_u8(zero.as_mut_ptr(), a);

        out_chunk.copy_from_slice(&zero[0..SIZE]);
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


