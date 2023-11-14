fn _rgb_to_hsv(_r: u8, _g: u8, _b: u8) {
    // https://github.com/opencv/opencv/blob/ed52f7feeaae6f0e96ee4dc36ae5f9fb4a6d959c/modules/imgproc/src/color_hsv.simd.hpp#L336
    // let vc = r.max(g).max(b);
    // if vc == 0 {
    //     return (0.0, 0.0, 0.0);
    // }
    //
    // let v = f32::from(vc);
    // let min = f32::from(r.min(g).min(b));
    //
    // let mt = v - min;
    //
    // let s = mt / v;
    // let mt_inv = 1. / mt;
    // let diff = 60.0 * mt_inv;
    // // let h;
    // // if vc == r {
    // //     h = (g - b) * v;
    // // } else if vc == g {
    // //     h = (b - r);
    // // }
    // let h = if r == g && g == b {
    //     0.0
    // } else if vc == b {
    //     240.0 + (60.0 * (f32::from(r - g)) * mt_inv)
    // } else if vc == g {
    //     120.0 + (60.0 * (f32::from(b - r)) * mt_inv)
    // } else {
    //     60.0 * (f32::from(g - b)) * mt_inv
    // };
    // return (h, s, v);
}
