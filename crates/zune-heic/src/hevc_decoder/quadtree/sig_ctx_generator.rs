//! Context index increments for `sig_coeff_flag` (spec 9.3.4.2.5), as in
//! libde265: one map per (transform size, luma/chroma, scan order, prevCsbf),
//! giving the increment for every coefficient position.
//!
//! All maps are computed at compile time into one flat table.

/// Offset of the first map of each transform size (4x4, 8x8, 16x16, 32x32);
/// every size has 2 * 3 * 4 = 24 maps of `w * w` entries.
const SIZE_OFFSETS: [usize; 4] = [0, 24 * 16, 24 * (16 + 64), 24 * (16 + 64 + 256)];
const TABLE_LEN: usize = 24 * (16 + 64 + 256 + 1024);

static SIG_CTX_MAPS: [u8; TABLE_LEN] = generate_all_sig_ctx_maps();

/// The map for a transform of size `4 << size_idx` (`size_idx` = log2 size - 2),
/// `chroma_idx` 0 = luma / 1 = chroma, `scan_idx` 0..3 and `prev_csbf` 0..4,
/// indexed by `x + (y << log2_size)`.
#[inline(always)]
pub fn sig_ctx_map(size_idx: usize, chroma_idx: usize, scan_idx: usize, prev_csbf: usize) -> &'static [u8] {
    let n = 16 << (2 * size_idx);
    let map = ((chroma_idx * 3 + scan_idx) * 4) + prev_csbf;
    let start = SIZE_OFFSETS[size_idx] + map * n;
    &SIG_CTX_MAPS[start..start + n]
}

// only ever evaluated at compile time (it initialises a `static`)
#[allow(clippy::large_stack_arrays)]
const fn generate_all_sig_ctx_maps() -> [u8; TABLE_LEN] {
    let mut table = [0u8; TABLE_LEN];
    let mut size_idx = 0;
    while size_idx < 4 {
        let log2w = size_idx + 2;
        let n = 1usize << (2 * log2w);
        let mut c_idx = 0;
        while c_idx < 2 {
            let mut scan_idx = 0;
            while scan_idx < 3 {
                let mut prev_csbf = 0;
                while prev_csbf < 4 {
                    let map = ((c_idx * 3 + scan_idx) * 4) + prev_csbf;
                    let start = SIZE_OFFSETS[size_idx] + map * n;
                    write_sig_map(&mut table, start, log2w, c_idx, scan_idx, prev_csbf);
                    prev_csbf += 1;
                }
                scan_idx += 1;
            }
            c_idx += 1;
        }
        size_idx += 1;
    }
    table
}

const fn write_sig_map(
    table: &mut [u8; TABLE_LEN], start: usize, log2w: usize, c_idx: usize, scan_idx: usize,
    prev_csbf: usize
) {
    #[rustfmt::skip]
    const CTX_IDX_MAP_4X4: [u8; 16] = [
        0, 1, 4, 5,
        2, 3, 4, 5,
        6, 6, 8, 8,
        7, 7, 8, 99
    ];

    let w = 1usize << log2w;
    let sb_width = w >> 2;

    let mut yc = 0;
    while yc < w {
        let mut xc = 0;
        while xc < w {
            let mut sig_ctx: u8;

            if sb_width == 1 {
                // 4x4 block
                sig_ctx = CTX_IDX_MAP_4X4[(yc << 2) + xc];
            } else if xc + yc == 0 {
                // DC component of larger blocks
                sig_ctx = 0;
            } else {
                let xp = xc & 3;
                let yp = yc & 3;
                let xs = xc >> 2;
                let ys = yc >> 2;

                // Match libde265 switch(prevCsbf)
                sig_ctx = match prev_csbf {
                    0 => {
                        if xp + yp >= 3 {
                            0
                        } else if xp + yp > 0 {
                            1
                        } else {
                            2
                        }
                    }
                    1 => {
                        if yp == 0 {
                            2
                        } else {
                            (yp == 1) as u8
                        }
                    }
                    2 => {
                        if xp == 0 {
                            2
                        } else {
                            (xp == 1) as u8
                        }
                    }
                    _ => 2 // default (prevCsbf == 3)
                };

                if c_idx == 0 {
                    // Luma logic
                    if xs + ys > 0 {
                        sig_ctx += 3;
                    }
                    if sb_width == 2 {
                        // 8x8 Luma: ctxIdx depends on scan (Diagonal vs Horizontal/Vertical)
                        sig_ctx += if scan_idx == 0 { 9 } else { 15 };
                    } else {
                        // 16x16 and 32x32 Luma
                        sig_ctx += 21;
                    }
                } else if sb_width == 2 {
                    // Chroma logic
                    sig_ctx += 9;
                } else {
                    sig_ctx += 12;
                }
            }

            // Final Context Index Increment
            let ctx_idx_inc = if c_idx == 0 { sig_ctx } else { 27 + sig_ctx };
            table[start + xc + (yc << log2w)] = ctx_idx_inc;
            xc += 1;
        }
        yc += 1;
    }
}
