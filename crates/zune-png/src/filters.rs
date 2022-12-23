//! A set of optimized filter functions for de-filtering ong
//! scanlines.
//!

mod sse4;

pub fn handle_avg(prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize)
{
    match components
    {
        1 =>
        {
            // handle leftmost byte explicitly
            current[0] = raw[0].wrapping_add(prev_row[0] >> 1);
            let mut recon_a = current[0];

            for ((filt, recon_b), out_px) in raw[1..]
                .iter()
                .zip(&prev_row[1..])
                .zip(current[1..].iter_mut())
            {
                // this needs to be performed with at least 9 bits of precision, so bump
                // it up to 16.
                let recon_b_u16 = u16::from(*recon_b);
                let recon_a_u16 = u16::from(recon_a);

                // The addition can never flow, an 8 bit addition  <= 9 bits.
                let recon_x = (((recon_a_u16 + recon_b_u16) >> 1) & 0xFF) as u8;

                *out_px = (*filt).wrapping_add(recon_x);
                recon_a = *out_px;
            }
        }
        2 =>
        {
            const COMP: usize = 2;
            let mut recon_a: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }
            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                current[i] = raw[i].wrapping_add(prev_row[i] >> 1);
                recon_a[i] = current[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let recon_a_u16 = u16::from(recon_a[$pos]);
                        let recon_b_u16 = u16::from(recon_b[$pos]);

                        // The addition can never flow, ad 8 bit addition  <= 9 bits.
                        let recon_x = (((recon_a_u16 + recon_b_u16) >> 1) & 0xFF) as u8;

                        out_px[$pos] = (filt[$pos]).wrapping_add(recon_x);
                        recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
            }
        }
        3 =>
        {
            #[cfg(all(
                target_feature = "sse2",
                feature = "sse",
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            {
                if is_x86_feature_detected!("sse2")
                {
                    // use the sse capable one when possible
                    crate::filters::sse4::defilter_avg3_sse(prev_row, raw, current);
                    return;
                }
            }
            const COMP: usize = 3;
            let mut recon_a: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }
            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                current[i] = raw[i].wrapping_add(prev_row[i] >> 1);
                recon_a[i] = current[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let recon_a_u16 = u16::from(recon_a[$pos]);
                        let recon_b_u16 = u16::from(recon_b[$pos]);

                        // The addition can never flow, ad 8 bit addition  <= 9 bits.
                        let recon_x = (((recon_a_u16 + recon_b_u16) >> 1) & 0xFF) as u8;

                        out_px[$pos] = filt[$pos].wrapping_add(recon_x);
                        recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
                unroll!(2);
            }
        }
        4 =>
        {
            #[cfg(all(
                target_feature = "sse2",
                feature = "sse",
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            {
                if is_x86_feature_detected!("sse2")
                {
                    // use the sse capable one when possible
                    crate::filters::sse4::defilter_avg4_sse(prev_row, raw, current);
                    return;
                }
            }
            const COMP: usize = 4;
            let mut recon_a: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }
            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                current[i] = raw[i].wrapping_add(prev_row[i] >> 1);
                recon_a[i] = current[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let recon_a_u16 = u16::from(recon_a[$pos]);
                        let recon_b_u16 = u16::from(recon_b[$pos]);

                        // The addition can never flow, ad 8 bit addition  <= 9 bits.
                        let recon_x = (((recon_a_u16 + recon_b_u16) >> 1) & 0xFF) as u8;

                        out_px[$pos] = (filt[$pos]).wrapping_add(recon_x);
                        recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
                unroll!(2);
                unroll!(3);
            }
        }
        _ => unreachable!()
    }
}

pub fn handle_sub(raw: &[u8], current: &mut [u8], components: usize)
{
    match components
    {
        1 =>
        {
            let mut recon_a: u8 = 0;

            for (filt, orig) in raw.iter().zip(current)
            {
                *orig = (*filt).wrapping_add(recon_a);
                recon_a = *orig;
            }
        }
        2 =>
        {
            const COMP: usize = 2;
            let mut recon_a: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP
            {
                return;
            }

            for (filt, orig) in raw.chunks_exact(COMP).zip(current.chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        orig[$pos] = filt[$pos].wrapping_add(recon_a[$pos]);
                        recon_a[$pos] = orig[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
            }
        }
        3 =>
        {
            #[cfg(all(
                target_feature = "sse2",
                feature = "sse",
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            {
                if is_x86_feature_detected!("sse2")
                {
                    // use the sse capable one when possible
                    crate::filters::sse4::de_filter_sub3_sse2(raw, current);
                    return;
                }
            }

            {
                const COMP: usize = 3;
                let mut recon_a: [u8; COMP] = [0; COMP];

                if current.len() < COMP || raw.len() < COMP
                {
                    return;
                }
                for (filt, orig) in raw.chunks_exact(COMP).zip(current.chunks_exact_mut(COMP))
                {
                    macro_rules! unroll {
                        ($pos:tt) => {
                            orig[$pos] = filt[$pos].wrapping_add(recon_a[$pos]);
                            recon_a[$pos] = orig[$pos];
                        };
                    }
                    unroll!(0);
                    unroll!(1);
                    unroll!(2);
                }
            }
        }
        4 =>
        {
            #[cfg(all(
                target_feature = "sse2",
                feature = "sse",
                any(target_arch = "x86", target_arch = "x86_64")
            ))]
            {
                // use the sst capable one when possible
                if is_x86_feature_detected!("sse2")
                {
                    crate::filters::sse4::de_filter_sub4_sse2(raw, current);
                    return;
                }
            }

            {
                const COMP: usize = 4;
                let mut recon_a: [u8; COMP] = [0; COMP];

                if current.len() < COMP || raw.len() < COMP
                {
                    return;
                }

                for (filt, orig) in raw.chunks_exact(COMP).zip(current.chunks_exact_mut(COMP))
                {
                    macro_rules! unroll {
                        ($pos:tt) => {
                            orig[$pos] = filt[$pos].wrapping_add(recon_a[$pos]);
                            recon_a[$pos] = orig[$pos];
                        };
                    }
                    unroll!(0);
                    unroll!(1);
                    unroll!(2);
                    unroll!(3);
                }
            }
        }
        _ => unreachable!()
    }
}

pub fn handle_paeth(prev_row: &[u8], raw: &[u8], current: &mut [u8], components: usize)
{
    match components
    {
        1 =>
        {
            // handle leftmost byte explicitly

            current[0] = raw[0].wrapping_add(paeth(0, prev_row[0], 0));
            let mut max_recon = current[0];
            let mut max_recon_c = prev_row[0];

            for ((filt, recon_b), out_px) in raw[1..]
                .iter()
                .zip(prev_row[1..].iter())
                .zip(current[1..].iter_mut())
            {
                let paeth_res = paeth(max_recon, *recon_b, max_recon_c);

                *out_px = (*filt).wrapping_add(paeth_res);

                // setup for the following iteration
                max_recon_c = *recon_b;
                max_recon = *out_px;
            }
        }
        2 =>
        {
            const COMP: usize = 2;
            let mut max_recon_a: [u8; COMP] = [0; COMP];
            let mut max_recon_c: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }

            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                current[i] = raw[i].wrapping_add(paeth(0, prev_row[i], 0));
                max_recon_a[i] = current[i];
                max_recon_c[i] = prev_row[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let paeth_res = paeth(max_recon_a[$pos], recon_b[$pos], max_recon_c[$pos]);

                        out_px[$pos] = (filt[$pos]).wrapping_add(paeth_res);

                        // setup for the following iteration
                        max_recon_c[$pos] = recon_b[$pos];
                        max_recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
            }
        }
        3 =>
        {
            #[cfg(all(feature = "sse", any(target_arch = "x86", target_arch = "x86_64")))]
            {
                // use the sse capable one when possible
                if is_x86_feature_detected!("sse4.1")
                {
                    crate::filters::sse4::de_filter_paeth3_sse41(prev_row, raw, current);
                    return;
                }
            }

            const COMP: usize = 3;
            let mut max_recon_a: [u8; COMP] = [0; COMP];
            let mut max_recon_c: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }

            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                let paeth_x = paeth(0, prev_row[i], 0);
                current[i] = raw[i].wrapping_add(paeth_x);
                max_recon_a[i] = current[i];
                max_recon_c[i] = prev_row[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let paeth_res = paeth(max_recon_a[$pos], recon_b[$pos], max_recon_c[$pos]);

                        out_px[$pos] = (filt[$pos]).wrapping_add(paeth_res);

                        // setup for the following iteration
                        max_recon_c[$pos] = recon_b[$pos];
                        max_recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
                unroll!(2);
            }
        }
        4 =>
        {
            #[cfg(all(feature = "sse", any(target_arch = "x86", target_arch = "x86_64")))]
            {
                // use the sse capable one when possible
                if is_x86_feature_detected!("sse4.1")
                {
                    crate::filters::sse4::de_filter_paeth4_sse41(prev_row, raw, current);
                    return;
                }
            }
            const COMP: usize = 4;
            let mut max_recon_a: [u8; COMP] = [0; COMP];
            let mut max_recon_c: [u8; COMP] = [0; COMP];

            if current.len() < COMP || raw.len() < COMP || prev_row.len() < COMP
            {
                return;
            }

            // handle leftmost byte explicitly
            for i in 0..COMP
            {
                let paeth_x = paeth(0, prev_row[i], 0);
                current[i] = raw[i].wrapping_add(paeth_x);
                max_recon_a[i] = current[i];
                max_recon_c[i] = prev_row[i];
            }

            for ((filt, recon_b), out_px) in raw[COMP..]
                .chunks_exact(COMP)
                .zip(prev_row[COMP..].chunks_exact(COMP))
                .zip(current[COMP..].chunks_exact_mut(COMP))
            {
                macro_rules! unroll {
                    ($pos:tt) => {
                        let paeth_res = paeth(max_recon_a[$pos], recon_b[$pos], max_recon_c[$pos]);

                        out_px[$pos] = (filt[$pos]).wrapping_add(paeth_res);

                        // setup for the following iteration
                        max_recon_c[$pos] = recon_b[$pos];
                        max_recon_a[$pos] = out_px[$pos];
                    };
                }
                unroll!(0);
                unroll!(1);
                unroll!(2);
                unroll!(3);
            }
        }
        _ => unreachable!()
    }
}

#[inline(always)]
pub fn paeth(a: u8, b: u8, c: u8) -> u8
{
    let a = i16::from(a);
    let b = i16::from(b);
    let c = i16::from(c);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();

    if pa <= pb && pa <= pc
    {
        return a as u8;
    }
    if pb <= pc
    {
        return b as u8;
    }
    c as u8
}
