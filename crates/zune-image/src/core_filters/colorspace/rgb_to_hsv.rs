fn python_mod(n: f32, base: f32) -> f32 {
    n - (n / base).floor() * base
}
#[inline(always)]
pub fn rgb_to_hsv_inner(r: f32, g: f32, b: f32) -> [f32; 3] {
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    let v = max_c;
    if min_c == max_c {
        return [0.0, 0.0, v];
    }
    let s = (max_c - min_c) / max_c;

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

    [h, s, v]
}
#[inline(always)]
pub fn hsv_to_rgb_inner(h: f32, s: f32, v: f32) -> [f32; 3] {
    if s == 0.0 {
        return [v, v, v];
    }
    let i = (h * 6.0) as i32;
    let f = (h * 6.0) - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q] // match 5..infinity, we can never go beyond 6 but the compiler can't see that
    }
}

pub fn rgb_to_hsv(r_h: &mut [f32], g_s: &mut [f32], b_v: &mut [f32]) {
    r_h.iter_mut()
        .zip(g_s.iter_mut())
        .zip(b_v.iter_mut())
        .for_each(|((r, g), b)| {
            let result = rgb_to_hsv_inner(*r, *g, *b);
            // now convert to hsv, pardon the names
            *r = result[0];
            *g = result[1];
            *b = result[2];
        })
}

pub fn hsv_to_rgb(h_r: &mut [f32], s_g: &mut [f32], v_b: &mut [f32]) {
    h_r.iter_mut()
        .zip(s_g.iter_mut())
        .zip(v_b.iter_mut())
        .for_each(|((r, g), b)| {
            let result = hsv_to_rgb_inner(*r, *g, *b);
            // now convert to hsv, pardon the names
            *r = result[0];
            *g = result[1];
            *b = result[2];
        })
}

#[cfg(test)]
mod tests {
    use nanorand::Rng;

    use crate::core_filters::colorspace::rgb_to_hsv::{hsv_to_rgb_inner, rgb_to_hsv_inner};

    fn test_helper(a: f32, b: f32, c: f32) {
        let result = rgb_to_hsv_inner(a, b, c);
        let result2 = hsv_to_rgb_inner(result[0], result[1], result[2]);
        let expected = [a, b, c];
        // accuracy we can live with, difference shouldn't be more than this
        const T_EPSILON: f32 = 0.001;
        expected
            .iter()
            .zip(result2.iter())
            .for_each(|(e, f)| assert!((e - f).abs() <= T_EPSILON, "{}!={}", e, f));
    }
    #[test]
    fn test_round_trip_random() {
        let mut rand = nanorand::WyRand::new();
        for _ in 0..100 {
            test_helper(rand.generate(), rand.generate(), rand.generate());
        }
    }
}
