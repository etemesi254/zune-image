pub fn generate_all_sig_ctx_maps() -> Vec<Vec<Vec<Vec<Vec<u8>>>>> {
    // [log2w - 2][cIdx][scanIdx][prevCsbf]
    let mut size_vec = Vec::with_capacity(4);

    for log2w in 2..=5 {
        let mut chroma_vec = Vec::with_capacity(2);
        for c_idx in 0..2 {
            let mut scan_vec = Vec::with_capacity(3);
            for scan_idx in 0..3 {
                let mut csbf_vec = Vec::with_capacity(4);
                for prev_csbf in 0..4 {
                    csbf_vec.push(generate_libde265_sig_map(log2w, c_idx, scan_idx, prev_csbf));
                }
                scan_vec.push(csbf_vec);
            }
            chroma_vec.push(scan_vec);
        }
        size_vec.push(chroma_vec);
    }
    size_vec
}

fn generate_libde265_sig_map(log2w: u8, c_idx: usize, scan_idx: u8, prev_csbf: u8) -> Vec<u8> {
    let w = 1 << log2w;
    let sb_width = w >> 2;
    let mut map = vec![0u8; w * w];

    // libde265 4x4 constant map
    let ctx_idx_map_4x4 = [0, 1, 4, 5, 2, 3, 8, 9, 6, 7, 10, 11, 12, 13, 14, 15];

    for yc in 0..w {
        for xc in 0..w {
            let mut sig_ctx: i32;

            if sb_width == 1 {
                // 4x4 block
                sig_ctx = ctx_idx_map_4x4[(yc << 2) + xc] as i32;
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
                    0 => if xp + yp >= 3 { 0 } else if xp + yp > 0 { 1 } else { 2 },
                    1 => if yp == 0 { 2 } else if yp == 1 { 1 } else { 0 },
                    2 => if xp == 0 { 2 } else if xp == 1 { 1 } else { 0 },
                    _ => 2, // default (prevCsbf == 3)
                };

                if c_idx == 0 {
                    // Luma logic
                    if xs + ys > 0 { sig_ctx += 3; }

                    if sb_width == 2 {
                        // 8x8 Luma: ctxIdx depends on scan (Diagonal vs Horizontal/Vertical)
                        sig_ctx += if scan_idx == 0 { 9 } else { 15 };
                    } else {
                        // 16x16 and 32x32 Luma
                        sig_ctx += 21;
                    }
                } else {
                    // Chroma logic
                    if sb_width == 2 {
                        sig_ctx += 9;
                    } else {
                        sig_ctx += 12;
                    }
                }
            }

            // Final Context Index Increment
            let ctx_idx_inc = if c_idx == 0 {
                sig_ctx as u8
            } else {
                27 + sig_ctx as u8
            };

            map[xc + (yc << log2w)] = ctx_idx_inc;
        }
    }
    map
}