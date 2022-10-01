pub(crate) fn convert_rgb_to_grayscale_scalar((r, g, b): (&[u8], &[u8], &[u8]), gr: &mut [u8])
{
    let r_coef = (0.2989 * 32768.0 + 0.5) as u32;
    let g_coef = (0.5870 * 32768.0 + 0.5) as u32;
    let b_coef = (0.1140 * 32768.0 + 0.5) as u32;

    for (((r_v, g_v), b_v), g_out) in r.iter().zip(g.iter()).zip(b.iter()).zip(gr.iter_mut())
    {
        // Multiply input elements by 64 for improved accuracy.

        let r = u32::from(*r_v) * 64;
        let g = u32::from(*g_v) * 64;
        let b = u32::from(*b_v) * 64;

        let g1 = ((r * r_coef) + (1 << 14)) >> 15;
        let g2 = ((g * g_coef) + (1 << 14)) >> 15;
        let g3 = ((b * b_coef) + (1 << 14)) >> 15;

        let g = (g1 + g2 + g3) / 64;

        *g_out = g.min(255) as u8;
    }
}
