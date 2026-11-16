/*
 * Copyright (c) 2025.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Aarch64 color conversion routines
//! NEON is mandatory on aarch64.

#![cfg(all(feature = "neon", target_arch = "aarch64"))]
use core::arch::aarch64::{
    int16x8_t, uint8x16_t, uint8x16x3_t, uint8x16x4_t, uint8x8_t, vaddq_s16, vcombine_u8,
    vdupq_n_s16, vdupq_n_u8, vget_low_s16, vld1q_s16, vmlal_high_n_s16, vmlal_n_s16,
    vmull_high_n_s16, vmull_n_s16, vqaddq_s16, vqmovun_s16, vrshrn_high_n_s32, vrshrn_n_s32,
    vst3q_u8, vst4q_u8, vsubq_s16
};

use super::scalar::{CB_FRAC, CR_FRAC, C_G_CB_FRAC, C_G_CR_FRAC};

/// `(a * cr + 32768) >> 16` for the eight lanes of `cr`.
///
/// `vrshrn` adds the rounding constant before the shift, so this matches the scalar
/// code exactly while only needing a 16x16->32 multiply per lane.
#[inline(always)]
unsafe fn mul_round_shr16(cr: int16x8_t, a: i16) -> int16x8_t {
    unsafe {
        vrshrn_high_n_s32::<16>(
            vrshrn_n_s32::<16>(vmull_n_s16(vget_low_s16(cr), a)),
            vmull_high_n_s16(cr, a)
        )
    }
}

/// Convert eight pixels (`cb` and `cr` still biased by 128) to R, G, B.
///
/// Computes the same integers as the scalar code. Its 16-bit coefficients are
/// `1 + CR_FRAC / 65536` etc., so the whole-number part of each chroma contribution
/// (`cr`, `-cr`, `2 * cb`) is added directly and only the fractional part is
/// multiplied, which fits a signed 16-bit multiply with a rounding narrowing shift.
#[inline(always)]
unsafe fn convert8(
    y: int16x8_t, cb: int16x8_t, cr: int16x8_t
) -> (uint8x8_t, uint8x8_t, uint8x8_t) {
    unsafe {
        let bias = vdupq_n_s16(128);
        let (cb, cr) = (vsubq_s16(cb, bias), vsubq_s16(cr, bias));

        let r = vqaddq_s16(y, vaddq_s16(cr, mul_round_shr16(cr, CR_FRAC)));

        let g_lo = vmlal_n_s16(
            vmull_n_s16(vget_low_s16(cr), C_G_CR_FRAC),
            vget_low_s16(cb),
            C_G_CB_FRAC
        );
        let g_hi = vmlal_high_n_s16(vmull_high_n_s16(cr, C_G_CR_FRAC), cb, C_G_CB_FRAC);
        let g_frac = vrshrn_high_n_s32::<16>(vrshrn_n_s32::<16>(g_lo), g_hi);
        let g = vqaddq_s16(y, vsubq_s16(g_frac, cr));

        let b = vqaddq_s16(
            y,
            vaddq_s16(vaddq_s16(cb, cb), mul_round_shr16(cb, CB_FRAC))
        );

        (vqmovun_s16(r), vqmovun_s16(g), vqmovun_s16(b))
    }
}

#[inline(always)]
unsafe fn ycbcr_to_rgb_baseline_no_clamp(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16]
) -> (uint8x16_t, uint8x16_t, uint8x16_t) {
    // SAFETY: Each input contains the 16 i16 values loaded below, and this module
    // is compiled only for AArch64 targets where NEON is available.
    unsafe {
        let (r0, g0, b0) = convert8(
            vld1q_s16(y.as_ptr()),
            vld1q_s16(cb.as_ptr()),
            vld1q_s16(cr.as_ptr())
        );
        let (r1, g1, b1) = convert8(
            vld1q_s16(y.as_ptr().add(8)),
            vld1q_s16(cb.as_ptr().add(8)),
            vld1q_s16(cr.as_ptr().add(8))
        );
        (
            vcombine_u8(r0, r1),
            vcombine_u8(g0, g1),
            vcombine_u8(b0, b1)
        )
    }
}

#[inline(always)]
pub fn ycbcr_to_rgb_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    // check if we have enough space to write.
    let out: &mut [u8; 48] = out
        .get_mut(*offset..*offset + 48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    // SAFETY: `out` is a validated 48-byte destination, and `vst3q_u8` writes
    // exactly three interleaved 16-byte vectors.
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst3q_u8(out.as_mut_ptr(), uint8x16x3_t(r, g, b));
    }
    *offset += 48;
}

#[inline(always)]
pub fn ycbcr_to_bgr_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    let out: &mut [u8; 48] = out
        .get_mut(*offset..*offset + 48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    // SAFETY: `out` is a validated 48-byte destination, and `vst3q_u8` writes
    // exactly three interleaved 16-byte vectors.
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst3q_u8(out.as_mut_ptr(), uint8x16x3_t(b, g, r));
    }
    *offset += 48;
}

#[inline(always)]
pub fn ycbcr_to_rgba_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    // check if we have enough space to write.
    let out: &mut [u8; 64] = out
        .get_mut(*offset..*offset + 64)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    // SAFETY: `out` is a validated 64-byte destination, and `vst4q_u8` writes
    // exactly four interleaved 16-byte vectors.
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst4q_u8(out.as_mut_ptr(), uint8x16x4_t(r, g, b, vdupq_n_u8(255)));
    }
    *offset += 64;
}

#[inline(always)]
pub fn ycbcr_to_bgra_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    let out: &mut [u8; 64] = out
        .get_mut(*offset..*offset + 64)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    // SAFETY: `out` is a validated 64-byte destination, and `vst4q_u8` writes
    // exactly four interleaved 16-byte vectors.
    unsafe {
        let (r, g, b) = ycbcr_to_rgb_baseline_no_clamp(y, cb, cr);
        vst4q_u8(out.as_mut_ptr(), uint8x16x4_t(b, g, r, vdupq_n_u8(255)));
    }
    *offset += 64;
}
