#![allow(unreachable_code)]

mod std_simd;

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
    [ 70, -43, -87,  -9,  90,  25, -80, -57],
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
        input[0] as i32,
        input[1] as i32,
        input[2] as i32,
        input[3] as i32
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
        input[0] as i32,
        input[1] as i32,
        input[2] as i32,
        input[3] as i32
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
            even_in[0] as i32,
            even_in[1] as i32,
            even_in[2] as i32,
            even_in[3] as i32
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
        input[1] as i32,
        input[3] as i32,
        input[5] as i32,
        input[7] as i32
    ];
    let o: [i32; 4] = std::array::from_fn(|i| {
        let r = &T8[i];
        i1 * r[0] as i32 + i3 * r[1] as i32 + i5 * r[2] as i32 + i7 * r[3] as i32
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
    let odds: [i32; 8] = std::array::from_fn(|j| input[j * 2 + 1] as i32);
    let o: [i32; 8] = std::array::from_fn(|i| {
        let r = &T16[i];
        odds.iter().zip(r.iter()).map(|(&x, &b)| x * b as i32).sum()
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
    let odds: [i32; 16] = std::array::from_fn(|j| input[j * 2 + 1] as i32);
    let o: [i32; 16] = std::array::from_fn(|i| {
        let r = &T32[i];
        odds.iter().zip(r.iter()).map(|(&x, &b)| x * b as i32).sum()
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
    let shift2: i32 = 20 - bit_depth as i32;

    // --- Pass 1: Vertical (Columns of block -> rows of intermediate) ---
    for c in 0..N {
        let mut col = [0i16; 32];
        for r in 0..N {
            col[r] = block[r * N + c];
        }

        if is_all_zero(&col[..N]) {
            for r in 0..N { intermediate[r * N + c] = 0; }
            continue;
        }

        if !is_dst && col[0] != 0 && is_all_zero(&col[1..N]) {
            let dc_val = shift_clip(col[0] as i32 * 64, shift1);
            for r in 0..N { intermediate[r * N + c] = dc_val; }
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
            for c in 0..N { block[r * N + c] = 0; }
            continue;
        }

        if !is_dst && row[0] != 0 && is_all_zero(&row[1..N]) {
            let val = shift_clip(row[0] as i32 * DC_VERTICAL_SCALE, shift2);
            for c in 0..N { block[r * N + c] = val; }
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
    idct_2d_scalar::<4>(block, scratchpad, bit_depth, true, transform4_dst_1d);
}

pub fn idct_4x4_hevc(block: &mut [i16; 16], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_4x4_hevc(block, scratchpad, bit_depth);
    }
    idct_2d_scalar::<4>(block, scratchpad, bit_depth, false, transform4_1d);
}

pub fn idct_8x8_hevc(block: &mut [i16; 64], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_8x8_hevc(block, scratchpad, bit_depth);
    }
    idct_2d_scalar::<8>(block, scratchpad, bit_depth, false, transform8_1d);
}

pub fn idct_16x16_hevc(block: &mut [i16; 256], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_16x16_hevc(block, scratchpad, bit_depth);
    }
    idct_2d_scalar::<16>(block, scratchpad, bit_depth, false, transform16_1d);
}

pub fn idct_32x32_hevc(block: &mut [i16; 1024], scratchpad: &mut [i16; 1024], bit_depth: u8) {
    #[cfg(feature = "simd")]
    {
        return std_simd::idct_32x32_hevc(block, scratchpad, bit_depth);
    }
    idct_2d_scalar::<32>(block, scratchpad, bit_depth, false, transform32_1d);
}
