#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
#[inline]
fn blinn_8x8(in_val: u8, y: u8) -> u8 {
    let t = i32::from(in_val) * i32::from(y) + 128;
    ((t + (t >> 8)) >> 8) as u8
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
pub fn cmyk_to_rgb_u8(c_to_r: &mut [u8], m_to_g: &mut [u8], y_to_b: &mut [u8], k: &[u8]) {
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
fn rgb_to_cmyk_inner_u8(r: u8, g: u8, b: u8) -> [u8; 4] {
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

/// Convert RGB to CMYK
///
/// # Arguments
/// - r_to_c: Contains Red Pixels, after operation will contain Cyan Pixels
/// - g_to_m: Contains green pixels, after operation will contain Magenta pixels
/// - b_to_y: Contains blue pixels, after operation, will contain Yellow  Pixels
/// - k: Will contain Key/Black Pixels
pub fn rgb_to_cmyk_u8(r_to_c: &mut [u8], g_to_m: &mut [u8], b_to_y: &mut [u8], k: &mut [u8]) {
    for (((r_c, g_m), b_y), k) in r_to_c.iter_mut().zip(g_to_m).zip(b_to_y).zip(k) {
        let result = rgb_to_cmyk_inner_u8(*r_c, *g_m, *b_y);
        *r_c = result[0];
        *g_m = result[1];
        *b_y = result[2];
        *k = result[3];
    }
}

#[inline(always)]
fn rgb_to_cmyk_inner_f32(r: f32, g: f32, b: f32) -> [f32; 4] {
    // from https://github.com/mozilla/mozjpeg/blob/master/cmyk.h
    let mut out = [0.; 4];

    let ctmp = 1.0 - r.clamp(0., 1.);
    let mtmp = 1.0 - g.clamp(0., 1.);
    let ytmp = 1.0 - b.clamp(0., 1.);

    let ktmp = ctmp.min(mtmp).min(ytmp);
    let kmtp_inv = 1.0 / (1.0 - ktmp);

    let c = (ctmp - ktmp) * kmtp_inv;
    let m = (mtmp - ktmp) * kmtp_inv;
    let y = (ytmp - ktmp) * kmtp_inv;

    out[0] = c;
    out[1] = m;
    out[2] = y;
    out[3] = ktmp;

    out
}

/// Convert RGB to CMYK
///
/// # Arguments
/// - r_to_c: Contains Red Pixels, after operation will contain Cyan Pixels
/// - g_to_m: Contains green pixels, after operation will contain Magenta pixels
/// - b_to_y: Contains blue pixels, after operation, will contain Yellow  Pixels
/// - k: Will contain Key/Black Pixels
pub fn rgb_to_cmyk_f32(r_to_c: &mut [f32], g_to_m: &mut [f32], b_to_y: &mut [f32], k: &mut [f32]) {
    for (((r_c, g_m), b_y), k) in r_to_c.iter_mut().zip(g_to_m).zip(b_to_y).zip(k) {
        let result = rgb_to_cmyk_inner_f32(*r_c, *g_m, *b_y);
        *r_c = result[0];
        *g_m = result[1];
        *b_y = result[2];
        *k = result[3];
    }
}
pub fn rgb_to_cmyk_u16(r_to_c: &mut [u16], g_to_m: &mut [u16], b_to_y: &mut [u16], k: &mut [u16]) {
    let v = f32::from(u16::MAX);
    let inv = 1.0 / v;
    for (((r_c, g_m), b_y), k) in r_to_c.iter_mut().zip(g_to_m).zip(b_to_y).zip(k) {
        // scale it to be between 0 and 1
        let result = rgb_to_cmyk_inner_f32(
            f32::from(*r_c) * inv,
            f32::from(*g_m) * inv,
            f32::from(*b_y) * inv
        );
        *r_c = (result[0] * v) as u16;
        *g_m = (result[1] * v) as u16;
        *b_y = (result[2] * v) as u16;
        *k = (result[3] * v) as u16;
    }
}
#[inline(always)]
fn cmyk_to_rgb_f32_inner(c: f32, m: f32, y: f32, k: f32) -> [f32; 3] {
    let k_inv = 1.0 / (k / 255.0 + 0.5);

    let r = c * k_inv;
    let g = m * k_inv;
    let b = y * k_inv;

    [r, g, b]
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
pub fn cmyk_to_rgb_f32(c_to_r: &mut [f32], m_to_g: &mut [f32], y_to_b: &mut [f32], k: &[f32]) {
    for (((c_r, m_g), y_b), k) in c_to_r.iter_mut().zip(m_to_g).zip(y_to_b).zip(k) {
        let c = *c_r;
        let m = *m_g;
        let y = *y_b;
        let k = *k;
        let [r, g, b] = cmyk_to_rgb_f32_inner(c, m, y, k);

        *c_r = r;
        *m_g = g;
        *y_b = b;
    }
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
pub fn cmyk_to_rgb_u16(c_to_r: &mut [u16], m_to_g: &mut [u16], y_to_b: &mut [u16], k: &[u16]) {
    let v = f32::from(u16::MAX);
    let inv = 1.0 / v;

    for (((c_r, m_g), y_b), k) in c_to_r.iter_mut().zip(m_to_g).zip(y_to_b).zip(k) {
        // scale the values to be between 0 and 1
        let c = f32::from(*c_r) * inv;
        let m = f32::from(*m_g) * inv;
        let y = f32::from(*y_b) * inv;
        let k = f32::from(*k) * inv;

        let [r, g, b] = cmyk_to_rgb_f32_inner(c, m, y, k);

        // scale back to be between 0 - 65535
        *c_r = (r * v) as u16;
        *m_g = (g * v) as u16;
        *y_b = (b * v) as u16;
    }
}
