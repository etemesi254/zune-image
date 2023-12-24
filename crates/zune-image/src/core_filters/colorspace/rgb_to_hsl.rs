//! RGB to Hue Saturation Lightness colorspace conversion routines
//!
//! This is based on Python's [colorsys](https://github.com/python/cpython/blob/3.9/Lib/colorsys.py) module
//! and should produce near identical values(not really identical due to floating point problems).
//!
//! This contains a mapping from RGB to HSL and back in floating points, values are expected to be between 0 and 1
//!
const ONE_THIRD: f32 = 1.0 / 3.0;
const ONE_SIXTH: f32 = 1.0 / 6.0;
const TWO_THIRD: f32 = 2.0 / 3.0;

#[inline]
fn python_mod(n: f32, base: f32) -> f32 {
    // we based our code on python, we have to match it's mod
    // function
    // see https://stackoverflow.com/questions/3883004/how-does-the-modulo-operator-work-on-negative-numbers-in-python
    n - (n / base).floor() * base
}
#[inline(always)]
fn rgb_to_hsl_inner(r: f32, g: f32, b: f32) -> [f32; 3] {
    // matches https://github.com/python/cpython/blob/3.9/Lib/colorsys.py
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);

    let l = (min_c + max_c) / 2.0;

    if min_c == max_c {
        return [0.0, 0.0, l];
    }

    let s = if l <= 0.5 {
        (max_c - min_c) / (max_c + min_c)
    } else {
        (max_c - min_c) / (2.0 - max_c - min_c)
    };
    let rc = (max_c - r) / (max_c - min_c);
    let gc = (max_c - g) / (max_c - min_c);
    let bc = (max_c - b) / (max_c - min_c);

    let mut h = if r == max_c {
        bc - gc
    } else if g == max_c {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    h = python_mod(h / 6.0, 1.0);

    [h, s, l]
}

#[inline(always)]
fn v(m1: f32, m2: f32, hue: f32) -> f32 {
    let hue = python_mod(hue, 1.0);

    if hue < ONE_SIXTH {
        return m1 + (m2 - m1) * hue * 6.0;
    }
    if hue < 0.5 {
        return m2;
    }
    if hue < TWO_THIRD {
        return m1 + (m2 - m1) * (TWO_THIRD - hue) * 6.0;
    }
    m1
}

fn hsl_to_rgb_inner(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s == 0.0 {
        return [l, l, l];
    }
    let m2 = if l <= 0.5 { l * (1.0 + s) } else { l + s - (l * s) };
    let m1 = 2.0 * l - m2;

    [
        v(m1, m2, h + ONE_THIRD),
        v(m1, m2, h),
        v(m1, m2, h - ONE_THIRD)
    ]
}

pub fn rgb_to_hsl(r_h: &mut [f32], g_s: &mut [f32], b_l: &mut [f32]) {
    r_h.iter_mut()
        .zip(g_s.iter_mut())
        .zip(b_l.iter_mut())
        .for_each(|((r, g), b)| {
            let result = rgb_to_hsl_inner(*r, *g, *b);
            // now convert to hsv, pardon the names
            *r = result[0];
            *g = result[1];
            *b = result[2];
        })
}

pub fn hsl_to_rgb(h_r: &mut [f32], s_g: &mut [f32], v_b: &mut [f32]) {
    h_r.iter_mut()
        .zip(s_g.iter_mut())
        .zip(v_b.iter_mut())
        .for_each(|((r, g), b)| {
            let result = hsl_to_rgb_inner(*r, *g, *b);
            // now convert to hsv, pardon the names
            *r = result[0];
            *g = result[1];
            *b = result[2];
        })
}
#[cfg(test)]
mod tests {
    use nanorand::Rng;

    use crate::core_filters::colorspace::rgb_to_hsl::{hsl_to_rgb_inner, rgb_to_hsl_inner};

    fn test_helper(a: f32, b: f32, c: f32) {
        let hsl_conv = rgb_to_hsl_inner(a, b, c);
        let roundtrip_conv = hsl_to_rgb_inner(hsl_conv[0], hsl_conv[1], hsl_conv[2]);
        let expected = [a, b, c];
        // error we can tolerate
        const T_EPSILON: f32 = 0.001;

        expected
            .iter()
            .zip(roundtrip_conv.iter())
            .for_each(|(e, f)| assert!((e - f).abs() <= T_EPSILON, "{}!={}", e, f));
    }
    #[test]
    fn test_round_trip_random() {
        let mut rand = nanorand::WyRand::new();
        for _i in 0..100 {
            test_helper(rand.generate(), rand.generate(), rand.generate());
        }
    }
}
