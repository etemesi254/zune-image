#![allow(unreachable_code)]


#[allow(unsafe_code)] // NEON intrinsics
mod aarch64;
mod std_simd;
#[allow(unsafe_code)] // SSE intrinsics
mod x86_64;

// ---------------------------------------------------------------------------
// Tuning Knob: DC Vertical Scale Factor
// ---------------------------------------------------------------------------
const DC_VERTICAL_SCALE: i32 = 64;

// ---------------------------------------------------------------------------
// Constants: Basis Matrices (Odd Parts)
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const T8: [[i16; 4]; 4] = [
    [ 89,  75,  50,  18],
    [ 75, -18, -89, -50],
    [ 50, -89,  18,  75],
    [ 18, -50,  75, -89],
];

#[rustfmt::skip]
const T16: [[i16; 8]; 8] = [
    [ 90,  87,  80,  70,  57,  43,  25,   9],
    [ 87,  57,   9, -43, -80, -90, -70, -25],
    [ 80,   9, -70, -87, -25,  57,  90,  43],
    [ 70, -43, -87,   9,  90,  25, -80, -57],
    [ 57, -80, -25,  90,  -9, -87,  43,  70],
    [ 43, -90,  57,  25, -87,  70,   9, -80],
    [ 25, -70,  90, -80,  43,   9, -57,  87],
    [  9, -25,  43, -57,  70, -80,  87, -90],
];

#[rustfmt::skip]
const T32: [[i16; 16]; 16] = [
    [ 90,  90,  88,  85,  82,  78,  73,  67,  61,  54,  46,  38,  31,  22,  13,   4],
    [ 90,  82,  67,  46,  22,  -4, -31, -54, -73, -85, -90, -88, -78, -61, -38, -13],
    [ 88,  67,  31, -13, -54, -82, -90, -78, -46,  -4,  38,  73,  90,  85,  61,  22],
    [ 85,  46, -13, -67, -90, -73, -22,  38,  82,  88,  54,  -4, -61, -90, -78, -31],
    [ 82,  22, -54, -90, -61,  13,  78,  85,  31, -46, -90, -67,   4,  73,  88,  38],
    [ 78,  -4, -82, -73,  13,  85,  67, -22, -88, -61,  31,  90,  54, -38, -90, -46],
    [ 73, -31, -90, -22,  78,  67, -38, -90, -13,  82,  61, -46, -88,  -4,  85,  54],
    [ 67, -54, -78,  38,  85, -22, -90,   4,  90,  13, -88, -31,  82,  46, -73, -61],
    [ 61, -73, -46,  82,  31, -88, -13,  90,  -4, -90,  22,  85, -38, -78,  54,  67],
    [ 54, -85,  -4,  88, -46, -61,  82,  13, -90,  38,  67, -78, -22,  90, -31, -73],
    [ 46, -90,  38,  54, -90,  31,  61, -88,  22,  67, -85,  13,  73, -82,   4,  78],
    [ 38, -88,  73,  -4, -67,  90, -46, -31,  85, -78,  13,  61, -90,  54,  22, -82],
    [ 31, -78,  90, -61,   4,  54, -88,  82, -38, -22,  73, -90,  67, -13, -46,  85],
    [ 22, -61,  85, -90,  73, -38,  -4,  46, -78,  90, -82,  54, -13, -31,  67, -88],
    [ 13, -38,  61, -78,  88, -90,  85, -73,  54, -31,   4,  22, -46,  67, -82,  90],
    [  4, -13,  22, -31,  38, -46,  54, -61,  67, -73,  78, -82,  85, -88,  90, -90],
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[inline(always)]
fn shift_clip(val: i32, shift: i32) -> i16 {
    // Branchless: (1 << shift) >> 1 == 0 when shift == 0
    let offset = (1i32 << shift) >> 1;
    ((val + offset) >> shift).clamp(-32768, 32767) as i16
}

/// Scalar zero-check — the compiler will auto-vectorize this loop easily.
#[inline(always)]
fn is_all_zero(s: &[i16]) -> bool {
    s.iter().all(|&v| v == 0)
}

// ---------------------------------------------------------------------------
// 1D Transform Kernels
// ---------------------------------------------------------------------------

#[inline(always)]
fn transform4_dst_1d(input: &[i16], output: &mut [i32]) {
    let [c0, c1, c2, c3] = [
        i32::from(input[0]),
        i32::from(input[1]),
        i32::from(input[2]),
        i32::from(input[3])
    ];

    // Correct HEVC DST-VII transposed matrix (T^T * v)
    output[0] = c0 * 29 + c1 * 74 + c2 * 84 + c3 * 55;
    output[1] = c0 * 55 + c1 * 74 - c2 * 29 - c3 * 84;
    output[2] = c0 * 74 - c2 * 74 + c3 * 74; // c1 * 0 is omitted
    output[3] = c0 * 84 - c1 * 74 + c2 * 55 - c3 * 29;
}

#[inline(always)]
fn transform4_1d(input: &[i16], output: &mut [i32]) {
    let [c0, c1, c2, c3] = [
        i32::from(input[0]),
        i32::from(input[1]),
        i32::from(input[2]),
        i32::from(input[3])
    ];
    let e0 = (c0 + c2) * 64;
    let e1 = (c0 - c2) * 64;
    let o0 = c1 * 83 + c3 * 36;
    let o1 = c1 * 36 - c3 * 83;

    output[0] = e0 + o0;
    output[1] = e1 + o1;
    output[2] = e1 - o1;
    output[3] = e0 - o0;
}

#[inline(always)]
fn transform8_1d(input: &[i16], output: &mut [i32]) {
    // Even part — reuse transform4_1d butterfly structure
    let even_in = [input[0], input[2], input[4], input[6]];
    let mut e = [0i32; 4];
    {
        let [c0, c1, c2, c3] = [
            i32::from(even_in[0]),
            i32::from(even_in[1]),
            i32::from(even_in[2]),
            i32::from(even_in[3])
        ];
        let ee0 = (c0 + c2) * 64;
        let ee1 = (c0 - c2) * 64;
        let eo0 = c1 * 83 + c3 * 36;
        let eo1 = c1 * 36 - c3 * 83;
        e[0] = ee0 + eo0;
        e[1] = ee1 + eo1;
        e[2] = ee1 - eo1;
        e[3] = ee0 - eo0;
    }

    // Odd part
    let [i1, i3, i5, i7] = [
        i32::from(input[1]),
        i32::from(input[3]),
        i32::from(input[5]),
        i32::from(input[7])
    ];
    let o: [i32; 4] = std::array::from_fn(|i| {
        let r = &T8[i];
        i1 * i32::from(r[0]) + i3 * i32::from(r[1]) + i5 * i32::from(r[2]) + i7 * i32::from(r[3])
    });

    for i in 0..4 {
        output[i] = e[i] + o[i];
        output[7 - i] = e[i] - o[i];
    }
}

#[inline(always)]
fn transform16_1d(input: &[i16], output: &mut [i32]) {
    // Even part via transform8 on even-indexed inputs
    let even_in: [i16; 8] = std::array::from_fn(|i| input[i * 2]);
    let mut e = [0i32; 8];
    transform8_1d(&even_in, &mut e);

    // Odd part — hoist odd inputs once, dot with each basis row
    let odds: [i32; 8] = std::array::from_fn(|j| i32::from(input[j * 2 + 1]));
    let o: [i32; 8] = std::array::from_fn(|i| {
        let r = &T16[i];
        odds.iter()
            .zip(r.iter())
            .map(|(&x, &b)| x * i32::from(b))
            .sum()
    });

    for i in 0..8 {
        output[i] = e[i] + o[i];
        output[15 - i] = e[i] - o[i];
    }
}

#[inline(always)]
fn transform32_1d(input: &[i16], output: &mut [i32]) {
    // Even part via transform16 on even-indexed inputs
    let even_in: [i16; 16] = std::array::from_fn(|i| input[i * 2]);
    let mut e = [0i32; 16];
    transform16_1d(&even_in, &mut e);

    // Odd part — hoist odd inputs once, dot with each basis row
    let odds: [i32; 16] = std::array::from_fn(|j| i32::from(input[j * 2 + 1]));
    let o: [i32; 16] = std::array::from_fn(|i| {
        let r = &T32[i];
        odds.iter()
            .zip(r.iter())
            .map(|(&x, &b)| x * i32::from(b))
            .sum()
    });

    for i in 0..16 {
        output[i] = e[i] + o[i];
        output[31 - i] = e[i] - o[i];
    }
}

// ---------------------------------------------------------------------------
// 2D Wrapper
// ---------------------------------------------------------------------------

pub fn idct_2d_scalar<const N: usize>(
    block: &mut [i16], intermediate: &mut [i16; 1024], bit_depth: u8, is_dst: bool,
    transform_1d: fn(&[i16], &mut [i32])
) {
    let shift1: i32 = 7;
    let shift2: i32 = 20 - i32::from(bit_depth);

    // --- Pass 1: Vertical (Columns of block -> rows of intermediate) ---
    for c in 0..N {
        let mut col = [0i16; 32];
        for r in 0..N {
            col[r] = block[r * N + c];
        }

        if is_all_zero(&col[..N]) {
            for r in 0..N {
                intermediate[r * N + c] = 0;
            }
            continue;
        }

        if !is_dst && col[0] != 0 && is_all_zero(&col[1..N]) {
            let dc_val = shift_clip(i32::from(col[0]) * 64, shift1);
            for r in 0..N {
                intermediate[r * N + c] = dc_val;
            }
            continue;
        }

        let mut col_out = [0i32; 32];
        transform_1d(&col[..N], &mut col_out);
        for r in 0..N {
            // Write to intermediate such that rows of intermediate
            // represent the columns of the block.
            intermediate[r * N + c] = shift_clip(col_out[r], shift1);
        }
    }

    // --- Pass 2: Horizontal (Rows of intermediate -> rows of block) ---
    for r in 0..N {
        let row_start = r * N;
        let row = &intermediate[row_start..row_start + N];

        if is_all_zero(row) {
            for c in 0..N {
                block[r * N + c] = 0;
            }
            continue;
        }

        if !is_dst && row[0] != 0 && is_all_zero(&row[1..N]) {
            let val = shift_clip(i32::from(row[0]) * DC_VERTICAL_SCALE, shift2);
            for c in 0..N {
                block[r * N + c] = val;
            }
            continue;
        }

        let mut row_out = [0i32; 32];
        transform_1d(row, &mut row_out);
        for c in 0..N {
            block[r * N + c] = shift_clip(row_out[c], shift2);
        }
    }
}

// ---------------------------------------------------------------------------
// Public Entry Points
// ---------------------------------------------------------------------------

pub fn idst_4x4_hevc(block: &mut [i16; 16], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idst_4x4_hevc(block, scratchpad, bit_depth);
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.1") {
            return x86_64::idst_4x4_hevc(block, scratchpad, bit_depth);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return aarch64::idst_4x4_hevc(block, scratchpad, bit_depth);
        }
    }

    idct_2d_scalar::<4>(block, scratchpad, bit_depth, true, transform4_dst_1d);
}

pub fn idct_4x4_hevc(block: &mut [i16; 16], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_4x4_hevc(block, scratchpad, bit_depth);
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.1") {
            return x86_64::idct_4x4_hevc(block, scratchpad, bit_depth);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;
        if is_aarch64_feature_detected!("neon") {
            return aarch64::idct_4x4_hevc(block, scratchpad, bit_depth);
        }
    }

    idct_2d_scalar::<4>(block, scratchpad, bit_depth, false, transform4_1d);
}

pub fn idct_8x8_hevc(block: &mut [i16; 64], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_8x8_hevc(block, scratchpad, bit_depth);
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.1") {
            return x86_64::idct_8x8_hevc(block, scratchpad, bit_depth);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return aarch64::idct_8x8_hevc(block, scratchpad, bit_depth);
        }
    }

    idct_2d_scalar::<8>(block, scratchpad, bit_depth, false, transform8_1d);
}

pub fn idct_16x16_hevc(block: &mut [i16; 256], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_16x16_hevc(block, scratchpad, bit_depth);
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.1") {
            return x86_64::idct_16x16_hevc(block, scratchpad, bit_depth);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return aarch64::idct_16x16_hevc(block, scratchpad, bit_depth);
        }
    }

    idct_2d_scalar::<16>(block, scratchpad, bit_depth, false, transform16_1d);
}

pub fn idct_32x32_hevc(block: &mut [i16; 1024], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_32x32_hevc(block, scratchpad, bit_depth);
    }
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("sse4.1") {
            return x86_64::idct_32x32_hevc(block, scratchpad, bit_depth);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        use std::arch::is_aarch64_feature_detected;

        if is_aarch64_feature_detected!("neon") {
            return aarch64::idct_32x32_hevc(block, scratchpad, bit_depth);
        }
    }

    idct_2d_scalar::<32>(block, scratchpad, bit_depth, false, transform32_1d);
}

#[cfg(test)]
mod spec_tests {
    //! Bit-exact checks of every inverse transform against a direct
    //! matrix-multiply implementation of spec 8.6.4.2.
    use super::*;

    /// 32x32 HEVC transMatrix, rows = basis k, cols = sample n.
    /// Built from the first column using the cosine symmetry of the DCT.
    fn trans_matrix() -> [[i32; 32]; 32] {
        const COL0: [i32; 32] = [
            64, 90, 90, 90, 89, 88, 87, 85, 83, 82, 80, 78, 75, 73, 70, 67, 64, 61, 57, 54, 50, 46,
            43, 38, 36, 31, 25, 22, 18, 13, 9, 4
        ];
        let mut m = [[0i32; 32]; 32];
        for k in 0..32 {
            for n in 0..32 {
                // angle in units of pi/64
                let a = ((2 * n + 1) * k) % 128;
                let (idx, sign) = match a {
                    0..=32 => (a, 1),
                    33..=64 => (64 - a, -1),
                    65..=96 => (a - 64, -1),
                    _ => (128 - a, 1)
                };
                let v = if idx == 32 { 0 } else { COL0[idx] };
                m[k][n] = if k == 0 { 64 } else { sign * v };
            }
        }
        m
    }

    fn reference(coeffs: &[i16], n: usize, dst: bool) -> Vec<i16> {
        let big = trans_matrix();
        let dstm = [[29, 55, 74, 84], [74, 74, 0, -74], [84, -29, -74, 55], [55, -84, 74, -29]];
        let t = |k: usize, i: usize| -> i64 {
            if dst { i64::from(dstm[k][i]) } else { i64::from(big[k * (32 / n)][i]) }
        };
        let clip = |v: i64| v.clamp(-32768, 32767);
        let mut tmp = vec![0i64; n * n];
        for x in 0..n {
            for y in 0..n {
                let s: i64 = (0..n).map(|k| t(k, y) * i64::from(coeffs[k * n + x])).sum();
                tmp[y * n + x] = clip((s + 64) >> 7);
            }
        }
        let mut out = vec![0i16; n * n];
        for y in 0..n {
            for x in 0..n {
                let s: i64 = (0..n).map(|k| t(k, x) * tmp[y * n + k]).sum();
                out[y * n + x] = clip((s + (1 << 11)) >> 12) as i16;
            }
        }
        out
    }

    fn rng(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    fn check<const N: usize>(f: fn(&mut [i16; N], &mut [i16; 1024], u8), n: usize, dst: bool) {
        let mut seed = 0x1234_5678_9abc_def1u64;
        for iter in 0..20000usize {
            let mut b = [0i16; N];
            let density = 1 + (iter % 6) as u64;
            let mag = [8u64, 64, 512, 4096, 32767][iter % 5];
            for v in &mut b {
                if rng(&mut seed) % 6 < density {
                    *v = ((rng(&mut seed) % (2 * mag + 1)) as i64 - mag as i64) as i16;
                }
            }
            if iter % 7 == 0 {
                b = [0; N];
                b[0] = ((rng(&mut seed) % 20001) as i64 - 10000) as i16;
            }
            let want = reference(&b, n, dst);
            let mut got = b;
            let mut scratch = [0i16; 1024];
            f(&mut got, &mut scratch, 8);
            assert_eq!(&got[..], &want[..], "{n}x{n} (dst={dst}) mismatch for input {b:?}");
        }
    }

    #[test]
    fn basis_16_and_32_match_spec() {
        let m = trans_matrix();
        for k in 0..16 {
            let mut inp = [0i16; 16];
            inp[k] = 1;
            let mut out = [0i32; 32];
            transform16_1d(&inp, &mut out);
            let want: Vec<i32> = (0..16).map(|n| m[k * 2][n]).collect();
            assert_eq!(&out[..16], &want[..], "16-point basis {k}");
        }
        for k in 0..32 {
            let mut inp = [0i16; 32];
            inp[k] = 1;
            let mut out = [0i32; 32];
            transform32_1d(&inp, &mut out);
            let want: Vec<i32> = (0..32).map(|n| m[k][n]).collect();
            assert_eq!(&out[..], &want[..], "32-point basis {k}");
        }
    }

    #[test]
    fn idst4_bit_exact() { check::<16>(idst_4x4_hevc, 4, true); }
    #[test]
    fn idct4_bit_exact() { check::<16>(idct_4x4_hevc, 4, false); }
    #[test]
    fn idct8_bit_exact() { check::<64>(idct_8x8_hevc, 8, false); }
    #[test]
    fn idct16_bit_exact() { check::<256>(idct_16x16_hevc, 16, false); }
    #[test]
    fn idct32_bit_exact() { check::<1024>(idct_32x32_hevc, 32, false); }

    /// The dispatchers above may pick a SIMD path; also pin the scalar path.
    #[test]
    fn scalar_paths_bit_exact() {
        check::<256>(|b, s, d| idct_2d_scalar::<16>(b, s, d, false, transform16_1d), 16, false);
        check::<1024>(|b, s, d| idct_2d_scalar::<32>(b, s, d, false, transform32_1d), 32, false);
        check::<64>(|b, s, d| idct_2d_scalar::<8>(b, s, d, false, transform8_1d), 8, false);
        check::<16>(|b, s, d| idct_2d_scalar::<4>(b, s, d, false, transform4_1d), 4, false);
        check::<16>(|b, s, d| idct_2d_scalar::<4>(b, s, d, true, transform4_dst_1d), 4, true);
    }
}
