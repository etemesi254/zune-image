#![cfg(feature = "portable-simd")]

use core::simd::prelude::*;
use std::mem::size_of;

/// Divides input by alpha, on encountering zero,
/// divides alpha by zero
fn unpremultiply_std_simd(input: &mut [f32], alpha: &[f32]) {
    // Compiler explorer
    // url:  https://rust.godbolt.org/z/5Y3qs1vvr

    const VECTOR_SIZE: usize = size_of::<Simd<f32, 8>>() / size_of::<f32>();

    let in_chunk = input.chunks_exact_mut(VECTOR_SIZE);
    let alpha_chunk = alpha.chunks_exact(VECTOR_SIZE);
    let zeros = f32x8::splat(0.0);

    for (chunk, alpha_values) in in_chunk.zip(alpha_chunk) {
        let ab = f32x8::from_slice(chunk);
        let al = f32x8::from_slice(alpha_values);
        let mask = al.simd_eq(zeros);
        // divide
        let div_result = ab / al;
        // remove effects of division bu zero
        // analogous to _mm_andnot_ps or (!mask & b)
        let result = (!mask).select(div_result, zeros);
        // store
        result.copy_to_slice(chunk);
    }
}

#[test]
fn test_unpremultiply() {
    use nanorand::Rng;

    use crate::premul_alpha::unpremultiply_f32_scalar;

    let mut in_array = [0.0f32; 256];

    nanorand::WyRand::new().fill(&mut in_array);
    let mut in_copy = in_array;

    let mut in_alpha = [0.0; 256];
    nanorand::WyRand::new().fill(&mut in_alpha);

    unpremultiply_f32_scalar(&mut in_array, &in_alpha);
    unpremultiply_std_simd(&mut in_copy, &in_alpha);

    for (a, b) in in_array.iter().zip(&in_copy) {
        let diff = a - b;

        assert!(a.is_finite());
        assert!(b.is_finite());

        assert!(diff < f32::EPSILON, "{a} {b}");
    }
}
