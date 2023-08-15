/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse2")]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;
#[cfg(target_arch = "x86")]
use std::arch::x86_64::*;
use std::mem::size_of;

use crate::premul_alpha::unpremultiply_f32_scalar;

/// Un-premultiply the channel with the alpha channel
pub(crate) unsafe fn unpremultiply_sse_f32(input: &mut [f32], alpha: &[f32]) {
    const VECTOR_SIZE: usize = size_of::<__m128>() / size_of::<f32>();

    let in_chunk = input.chunks_exact_mut(VECTOR_SIZE);
    let alpha_chunk = alpha.chunks_exact(VECTOR_SIZE);

    for (chunk, alpha_values) in in_chunk.zip(alpha_chunk) {
        // load items
        let ab = _mm_loadu_ps(chunk.as_ptr());
        let al = _mm_loadu_ps(alpha_values.as_ptr());
        // create mask
        let mask = _mm_cmpeq_ps(_mm_set1_ps(0.0), al);

        // divide
        let div_result = _mm_div_ps(ab, al);
        // remove effects of div by zero
        let result = _mm_andnot_ps(mask, div_result);
        // store
        _mm_storeu_ps(chunk.as_mut_ptr(), result)
    }
    // handle remainder
    unpremultiply_f32_scalar(
        input.chunks_exact_mut(VECTOR_SIZE).into_remainder(),
        alpha.chunks_exact(VECTOR_SIZE).remainder()
    );
}

#[test]
fn test_inverse_sse_scalar() {
    use nanorand::Rng;

    let mut in_array = [0.0f32; 256];

    nanorand::WyRand::new().fill(&mut in_array);
    let mut in_copy = in_array;

    let mut in_alpha = [0.0; 256];
    nanorand::WyRand::new().fill(&mut in_alpha);

    unpremultiply_f32_scalar(&mut in_array, &in_alpha);

    unsafe {
        unpremultiply_sse_f32(&mut in_copy, &in_alpha);
    }
    for (a, b) in in_array.iter().zip(&in_copy) {
        let diff = a - b;

        assert!(a.is_finite());
        assert!(b.is_finite());

        assert!(diff < f32::EPSILON, "{a} {b}");
    }
}
