#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn blinn_8x8(in_val: u8, y: u8) -> u8 {
    let t = i32::from(in_val) * i32::from(y) + 128;
    return ((t + (t >> 8)) >> 8) as u8;
}
/// Convert CMYK to RGB
///
/// This modifies array in place, in order to conserve memory
///
/// # Arguments
/// - c_to_r: Contains the Cyan  pixels, after this, they will contain the red pixels
/// - m_to_g: Contains the Magenta pixels, after this, they will contain the green pixels
/// - y_to_b: Contains the Yellow pixels, after this, they will contain the blue pixels
///- k : Contains the key or black pixels,    
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
pub fn cmyk_to_rgb(c_to_r: &mut [u8], m_to_g: &mut [u8], y_to_b: &mut [u8], k: &[u8]) {
    for (((c_r, m_g), y_b), k) in c_to_r.iter_mut().zip(m_to_g).zip(y_to_b).zip(k) {
        let c = *c_r;
        let m = *m_g;
        let y = *y_b;
        let k = *k;
        // use a fast u8 divider borrowed from stb

        *c_r = blinn_8x8(c, k);
        *m_g = blinn_8x8(m, k);
        *y_b = blinn_8x8(y, k);
    }
}
#[inline(always)]
fn rgb_to_cmyk_inner(r: u8, g: u8, b: u8) -> [u8; 4] {
    // from https://github.com/mozilla/mozjpeg/blob/master/cmyk.h
    let mut out = [0; 4];

    let mut ctmp = 1.0 - ((r as f32) / 255.0);
    let mut mtmp = 1.0 - ((g as f32) / 255.0);
    let mut ytmp = 1.0 - ((b as f32) / 255.0);

    let ktmp = ctmp.min(mtmp).min(ytmp);
    let kmtp_inv = 1.0 / (1.0 - ktmp);

    ctmp = (ctmp - ktmp) * kmtp_inv;
    mtmp = (mtmp - ktmp) * kmtp_inv;
    ytmp = (ytmp - ktmp) * kmtp_inv;

    let c = (255.0 - ctmp * 255.0 + 0.5) as u8;
    let m = (255.0 - mtmp * 255.0 + 0.5) as u8;
    let y = (255.0 - ytmp * 255.0 + 0.5) as u8;
    let k = (255.0 - ktmp * 255.0 + 0.5) as u8;

    out[0] = c;
    out[1] = m;
    out[2] = y;
    out[3] = k;

    out
}

pub fn rgb_to_cmyk(r_to_c: &mut [u8], g_to_m: &mut [u8], b_to_y: &mut [u8], k: &mut [u8]) {
    for (((r_c, g_m), b_y), k) in r_to_c.iter_mut().zip(g_to_m).zip(b_to_y).zip(k) {
        let result = rgb_to_cmyk_inner(*r_c, *g_m, *b_y);
        *r_c = result[0];
        *g_m = result[1];
        *b_y = result[2];
        *k = result[3];
    }
}
