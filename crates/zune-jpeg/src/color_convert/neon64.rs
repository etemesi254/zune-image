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
    uint8x16_t, uint8x16x3_t, uint8x16x4_t, vaddq_s32, vcombine_u16, vcombine_u8,
    vdupq_n_s16, vdupq_n_s32, vdupq_n_u8, vget_low_s16, vld1q_s16, vmlaq_n_s32,
    vmovl_high_s16, vmovl_s16, vqmovn_u16, vqshrun_n_s32, vshlq_n_s32, vst3q_u8,
    vst4q_u8, vsubq_s16
};

#[inline(always)]
unsafe fn ycbcr_to_rgb_baseline_no_clamp(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16]
) -> (uint8x16_t, uint8x16_t, uint8x16_t) {
    // SAFETY: Each input contains the 16 i16 values loaded below, and this module
    // is compiled only for AArch64 targets where NEON is available.
    unsafe {
        let y0 = vld1q_s16(y.as_ptr());
        let y1 = vld1q_s16(y[8..].as_ptr());
        let bias = vdupq_n_s16(128);
        let cb0 = vsubq_s16(vld1q_s16(cb.as_ptr()), bias);
        let cb1 = vsubq_s16(vld1q_s16(cb[8..].as_ptr()), bias);
        let cr0 = vsubq_s16(vld1q_s16(cr.as_ptr()), bias);
        let cr1 = vsubq_s16(vld1q_s16(cr[8..].as_ptr()), bias);

        // Apply the native 16-bit coefficients and half-up bias directly in wide lanes.
        let convert = |y, cb, cr| {
            let base = vaddq_s32(vshlq_n_s32::<16>(y), vdupq_n_s32(32_768));
            let red = vmlaq_n_s32(base, cr, 91_881);
            let green = vmlaq_n_s32(vmlaq_n_s32(base, cr, -46_802), cb, -22_554);
            let blue = vmlaq_n_s32(base, cb, 116_130);
            (
                vqshrun_n_s32::<16>(red),
                vqshrun_n_s32::<16>(green),
                vqshrun_n_s32::<16>(blue)
            )
        };
        let (r0, g0, b0) = convert(
            vmovl_s16(vget_low_s16(y0)), vmovl_s16(vget_low_s16(cb0)), vmovl_s16(vget_low_s16(cr0))
        );
        let (r1, g1, b1) = convert(vmovl_high_s16(y0), vmovl_high_s16(cb0), vmovl_high_s16(cr0));
        let (r2, g2, b2) = convert(
            vmovl_s16(vget_low_s16(y1)), vmovl_s16(vget_low_s16(cb1)), vmovl_s16(vget_low_s16(cr1))
        );
        let (r3, g3, b3) = convert(vmovl_high_s16(y1), vmovl_high_s16(cb1), vmovl_high_s16(cr1));
        let pack = |a, b, c, d| vcombine_u8(
            vqmovn_u16(vcombine_u16(a, b)), vqmovn_u16(vcombine_u16(c, d))
        );
        (pack(r0, r1, r2, r3), pack(g0, g1, g2, g3), pack(b0, b1, b2, b3))
    }
}

#[inline(always)]
pub fn ycbcr_to_rgb_neon(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], out: &mut [u8], offset: &mut usize
) {
    // check if we have enough space to write.
    let out: &mut [u8; 48] = out.get_mut(*offset..*offset + 48).expect("Slice to small cannot write").try_into().unwrap();

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
    let out: &mut [u8; 64] = out.get_mut(*offset..*offset + 64).expect("Slice to small cannot write").try_into().unwrap();

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
