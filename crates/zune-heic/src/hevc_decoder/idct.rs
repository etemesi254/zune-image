/// HEVC (H.265) Inverse Transform — Full Correctness & Optimized Performance.

// ---------------------------------------------------------------------------
// Constants: Basis Matrices (Odd Parts)
// ---------------------------------------------------------------------------
mod std_simd;
#[rustfmt::skip]
const T8: [[i32; 4]; 4] = [
    [89,  75,  50,  18],
    [75, -18, -89, -50],
    [50, -89,  18,  75],
    [18, -50,  75, -89],
];

#[rustfmt::skip]
const T16: [[i32; 8]; 8] = [
    [90,  87,  80,  70,  57,  43,  25,   9],
    [87,  57,   9, -43, -80, -90, -70, -25],
    [80,   9, -70, -87, -25,  57,  90,  43],
    [70, -43, -87,  -9,  90,  25, -80, -57],
    [57, -80, -25,  90,  -9, -87,  43,  70],
    [43, -90,  57,  25, -87,  70,   9, -80],
    [25, -70,  90, -80,  43,   9, -57,  87],
    [ 9, -25,  43, -57,  70, -80,  87, -90],
];

#[rustfmt::skip]
const T32: [[i32; 16]; 16] = [
    [90, 90, 88, 85, 82, 78, 73, 67, 61, 54, 46, 38, 31, 22, 13,  4],
    [90, 82, 67, 46, 22, -4,-31,-54,-73,-85,-90,-88,-78,-61,-38,-13],
    [88, 67, 31,-13,-54,-82,-90,-78,-46, -4, 38, 73, 90, 85, 61, 22],
    [85, 46,-13,-67,-90,-73,-22, 38, 82, 88, 54, -4,-61,-90,-78,-31],
    [82, 22,-54,-90,-61, 13, 78, 85, 31,-46,-90,-67,  4, 73, 88, 38],
    [78, -4,-82,-73, 13, 85, 67,-22,-88,-61, 31, 90, 54,-38,-90,-46],
    [73,-31,-90,-22, 78, 67,-38,-90,-13, 82, 61,-46,-88, -4, 85, 54],
    [67,-54,-78, 38, 85,-22,-90,  4, 90, 13,-88,-31, 82, 46,-73,-61],
    [61,-73,-46, 82, 31,-88,-13, 90, -4,-90, 22, 85,-38,-78, 54, 67],
    [54,-85, -4, 88,-46,-61, 82, 13,-90, 38, 67,-78,-22, 90,-31,-73],
    [46,-90, 38, 54,-90, 31, 61,-88, 22, 67,-85, 13, 73,-82,  4, 78],
    [38,-88, 73, -4,-67, 90,-46,-31, 85,-78, 13, 61,-90, 54, 22,-82],
    [31,-78, 90,-61,  4, 54,-88, 82,-38,-22, 73,-90, 67,-13,-46, 85],
    [22,-61, 85,-90, 73,-38, -4, 46,-78, 90,-82, 54,-13,-31, 67,-88],
    [18,-50, 75,-89, 89,-75, 50,-18,-18, 50,-75, 89,-89, 75,-50, 18],
    [ 4,-13, 22,-31, 38,-46, 54,-61, 67,-73, 78,-82, 85,-88, 90,-90],
];

// ---------------------------------------------------------------------------
// Helpers & Clipping
// ---------------------------------------------------------------------------

#[inline(always)]
fn shift_clip(val: i32, shift: i32) -> i32 {
    let offset = if shift > 0 { 1 << (shift - 1) } else { 0 };
    (val + offset) >> shift
}

#[inline(always)]
fn intermediate_clip(val: i32) -> i32 {
    val.clamp(-32768, 32767)
}

#[inline(always)]
fn final_clip(val: i32, bit_depth: u8) -> i32 {
    let max = (1 << bit_depth) - 1;
    val.clamp(0, max)
}

#[inline(always)]
fn get_last_nonzero_idx(data: &[i32]) -> i32 {
    for i in (0..data.len()).rev() {
        if data[i] != 0 {
            return i as i32;
        }
    }
    -1
}

// ---------------------------------------------------------------------------
// 1D Transform Kernels
// ---------------------------------------------------------------------------

fn transform4_1d(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let (c0, c1, c2, c3) = (input[0], input[1], input[2], input[3]);
    let e0 = (c0 * 64) + (c2 * 64);
    let e1 = (c0 * 64) - (c2 * 64);
    let o0 = (c1 * 83) + (c3 * 36);
    let o1 = (c1 * 36) - (c3 * 83);

    let res = [e0 + o0, e1 + o1, e1 - o1, e0 - o0];
    for i in 0..4 {
        let v = shift_clip(res[i], shift);
        output[i] = if should_clip { intermediate_clip(v) } else { v };
    }
}

fn transform4_dst_1d(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let (c0, c1, c2, c3) = (input[0], input[1], input[2], input[3]);
    let s0 = (c0 * 29) + (c1 * 55) + (c2 * 74) + (c3 * 84);
    let s1 = (c0 * 74) + (c1 * 74) - (c3 * 74);
    let s2 = (c0 * 84) - (c1 * 29) - (c2 * 74) + (c3 * 55);
    let s3 = (c0 * 55) - (c1 * 84) + (c2 * 74) - (c3 * 29);

    let res = [s0, s1, s2, s3];
    for i in 0..4 {
        let v = shift_clip(res[i], shift);
        output[i] = if should_clip { intermediate_clip(v) } else { v };
    }
}

fn transform8_1d(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let ee0 = (input[0] * 64) + (input[4] * 64);
    let ee1 = (input[0] * 64) - (input[4] * 64);
    let eo0 = (input[2] * 83) + (input[6] * 36);
    let eo1 = (input[2] * 36) - (input[6] * 83);
    let e = [ee0 + eo0, ee1 + eo1, ee1 - eo1, ee0 - eo0];

    let c = |n: usize| input[n * 2 + 1];
    let mut o = [0i32; 4];

    for i in 0..4 {
        o[i] = (c(0) * T8[i][0]) + (c(1) * T8[i][1]) + (c(2) * T8[i][2]) + (c(3) * T8[i][3]);
    }

    for i in 0..4 {
        let v1 = shift_clip(e[i] + o[i], shift);
        let v2 = shift_clip(e[3 - i] - o[3 - i], shift);
        output[i] = if should_clip { intermediate_clip(v1) } else { v1 };
        output[7 - i] = if should_clip { intermediate_clip(v2) } else { v2 };
    }
}

fn transform16_1d(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let mut e = [0i32; 8];
    let mut even_in = [0i32; 8];
    for i in 0..8 {
        even_in[i] = input[i * 2];
    }
    transform8_1d(&even_in, &mut e, 0, false);

    let c = |n: usize| input[n * 2 + 1];
    let mut o = [0i32; 8];

    for i in 0..8 {
        o[i] = (c(0) * T16[i][0])
            + (c(1) * T16[i][1])
            + (c(2) * T16[i][2])
            + (c(3) * T16[i][3])
            + (c(4) * T16[i][4])
            + (c(5) * T16[i][5])
            + (c(6) * T16[i][6])
            + (c(7) * T16[i][7]);
    }

    for i in 0..8 {
        let v1 = shift_clip(e[i] + o[i], shift);
        let v2 = shift_clip(e[7 - i] - o[7 - i], shift);
        output[i] = if should_clip { intermediate_clip(v1) } else { v1 };
        output[15 - i] = if should_clip { intermediate_clip(v2) } else { v2 };
    }
}

fn transform32_1d(input: &[i32], output: &mut [i32], shift: i32, should_clip: bool) {
    let mut e = [0i32; 16];
    let mut even_in = [0i32; 16];
    for i in 0..16 {
        even_in[i] = input[i * 2];
    }
    transform16_1d(&even_in, &mut e, 0, false);

    let c = |n: usize| input[n * 2 + 1];
    let mut o = [0i32; 16];

    for i in 0..16 {
        let mut sum = 0;
        for j in 0..16 {
            sum += c(j) * T32[i][j];
        }
        o[i] = sum;
    }

    for i in 0..16 {
        let v1 = shift_clip(e[i] + o[i], shift);
        let v2 = shift_clip(e[15 - i] - o[15 - i], shift);
        output[i] = if should_clip { intermediate_clip(v1) } else { v1 };
        output[31 - i] = if should_clip { intermediate_clip(v2) } else { v2 };
    }
}

// ---------------------------------------------------------------------------
// Optimized 2D Wrapper
// ---------------------------------------------------------------------------
fn idct_2d_optimized<const N: usize>(
    block: &mut [i32], bit_depth: u8, is_dst: bool, transform_1d: fn(&[i32], &mut [i32], i32, bool)
) {
    let mut intermediate = [0i32; 1024];
    let shift1 = 7;
    let shift2 = 20 - bit_depth as i32;

    // Pass 1: Horizontal Rows
    for r in 0..N {
        let start = r * N;
        let row_in = &block[start..start + N];
        let last_idx = get_last_nonzero_idx(row_in);

        if last_idx < 0 {
            for c in 0..N {
                intermediate[c * N + r] = 0;
            }
            continue;
        }

        let mut row_out = [0i32; 32];
        if last_idx == 0 && !is_dst {
            let dc_val = intermediate_clip(shift_clip(row_in[0] * 64, shift1));
            for c in 0..N {
                row_out[c] = dc_val;
            }
        } else {
            transform_1d(row_in, &mut row_out[..N], shift1, true);
        }

        for c in 0..N {
            intermediate[c * N + r] = row_out[c];
        }
    }

    // Pass 2: Vertical (Columns are rows in intermediate buffer)
    for r in 0..N {
        let start = r * N;
        let col_in = &intermediate[start..start + N];
        let last_idx = get_last_nonzero_idx(col_in);

        if last_idx < 0 {
            for c in 0..N {
                block[c * N + r] = 0;
            }
            continue;
        }

        let mut col_out = [0i32; 32];
        // Note: For Pass 2, even if last_idx is 0, col_in[0] might differ across columns
        // because Pass 1 was already applied. The shortcut remains bit-exact.
        if last_idx == 0 && !is_dst {
            let dc_val = final_clip(shift_clip(col_in[0] * 64, shift2), bit_depth);
            for c in 0..N {
                col_out[c] = dc_val;
            }
        } else {
            transform_1d(col_in, &mut col_out[..N], shift2, false);
            for c in 0..N {
                col_out[c] = final_clip(col_out[c], bit_depth);
            }
        }

        for c in 0..N {
            block[c * N + r] = col_out[c];
        }
    }
}

// ---------------------------------------------------------------------------
// Public Entry Points
// ---------------------------------------------------------------------------
pub fn idst_4x4_hevc(block: &mut [i32; 16], bit_depth: u8) {
    idct_2d_optimized::<4>(block, bit_depth, true, transform4_dst_1d);
}

pub fn idct_4x4_hevc(block: &mut [i32; 16], bit_depth: u8) {
    idct_2d_optimized::<4>(block, bit_depth, false, transform4_1d);
}

pub fn idct_8x8_hevc(block: &mut [i32; 64], bit_depth: u8) {
    idct_2d_optimized::<8>(block, bit_depth, false, transform8_1d);
}

pub fn idct_16x16_hevc(block: &mut [i32; 256], bit_depth: u8) {
    idct_2d_optimized::<16>(block, bit_depth, false, transform16_1d);
}

pub fn idct_32x32_hevc(block: &mut [i32; 1024], bit_depth: u8) {
    idct_2d_optimized::<32>(block, bit_depth, false, transform32_1d);
}
