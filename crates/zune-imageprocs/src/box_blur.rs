use crate::transpose;

fn compute_mod_u32(d: u64) -> u64
{
    return (0xFFFF_FFFF_FFFF_FFFF_u64) / d.saturating_add(1);
}
fn mul128_u32(low_bits: u64, d: u32) -> u64
{
    return ((u128::from(low_bits) * u128::from(d)) >> 64) as u64;
}

#[allow(clippy::cast_possible_truncation)]
fn fastdiv_u32(a: u32, m: u64) -> u32
{
    (mul128_u32(m, a) & u64::from(u32::MAX)) as u32
}
pub fn box_blur(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, radius: usize,
)
{
    if width == 0 || radius == 0
    {
        return;
    }
    box_blur_inner(in_out_image, scratch_space, width, height, radius);
    in_out_image.copy_from_slice(scratch_space);
    transpose::transpose(scratch_space, in_out_image, width, height);
    box_blur_inner(in_out_image, scratch_space, height, width, radius);
    transpose::transpose(scratch_space, in_out_image, height, width);
}
#[allow(clippy::cast_possible_truncation)]
fn box_blur_inner(in_image: &[u8], out_image: &mut [u8], width: usize, height: usize, radius: usize)
{
    // Box blurs can be seen as the average of three pixels iterating
    // through a window
    // A box blur therefore is
    //
    // pix[x,y]= (pix[x-r/2,y]...+pix[x,y]+...pix[x+r/2,y])/r
    //
    // This is better than the previous one since we don't have to worry
    // about the addition overflowing and we can use u8s for it
    //
    // Another important optimization is that widths are independent of each other
    // we can use that to our advantage to increase the parallelism
    //

    if width == 0 || radius == 0
    {
        // repeated here for the optimizer
        return;
    }
    let radius = radius.min(width);
    let m_radius = compute_mod_u32(radius as u64);
    //
    // Handle data not in boundaries
    for (four_stride_in, four_stride_out) in in_image
        .chunks_exact(width * 4)
        .zip(out_image.chunks_exact_mut(width * 4))
    {
        let (os1, rem) = four_stride_out.split_at_mut(width);
        let (os2, rem) = rem.split_at_mut(width);
        let (os3, os4) = rem.split_at_mut(width);

        let (ws1, rem) = four_stride_in.split_at(width);
        let (ws2, rem) = rem.split_at(width);
        let (ws3, ws4) = rem.split_at(width);

        // do the first accumulation
        let (mut a1, mut a2, mut a3, mut a4) = (0, 0, 0, 0);

        for ((((pos, n1), n2), n3), n4) in ws1
            .iter()
            .enumerate()
            .zip(ws2.iter())
            .zip(ws3.iter())
            .zip(ws4.iter())
            .take(radius)
        {
            a1 += u32::from(*n1);
            a2 += u32::from(*n2);
            a3 += u32::from(*n3);
            a4 += u32::from(*n4);

            // Handle edge pixels
            let p = (pos as u32).saturating_add(1);
            os1[pos] = (a1 / p) as u8;
            os2[pos] = (a2 / p) as u8;
            os3[pos] = (a3 / p) as u8;
            os4[pos] = (a4 / p) as u8;

            unsafe {
                std::arch::asm!("");
            }
        }

        let r_window = radius.saturating_add(0);

        let mut r1 = u32::from(ws1[0]);
        let mut r2 = u32::from(ws2[0]);
        let mut r3 = u32::from(ws3[0]);
        let mut r4 = u32::from(ws4[0]);

        for (((((((o1, o2), o3), o4), w1), w2), w3), w4) in os1[radius..]
            .iter_mut()
            .zip(os2[radius..].iter_mut())
            .zip(os3[radius..].iter_mut())
            .zip(os4[radius..].iter_mut())
            .zip(ws1.windows(r_window))
            .zip(ws2.windows(r_window))
            .zip(ws3.windows(r_window))
            .zip(ws4.windows(r_window))
        {
            a1 = a1.wrapping_add(u32::from(w1[radius - 1]).wrapping_sub(r1));
            *o1 = fastdiv_u32(a1, m_radius) as u8;

            a2 = a2.wrapping_add(u32::from(w2[radius - 1]).wrapping_sub(r2));
            *o2 = fastdiv_u32(a2, m_radius) as u8;

            a3 = a3.wrapping_add(u32::from(w3[radius - 1]).wrapping_sub(r3));
            *o3 = fastdiv_u32(a3, m_radius) as u8;

            a4 = a4.wrapping_add(u32::from(w4[radius - 1]).wrapping_sub(r4));
            *o4 = fastdiv_u32(a4, m_radius) as u8;

            r1 = u32::from(w1[0]);
            r2 = u32::from(w2[0]);
            r3 = u32::from(w3[0]);
            r4 = u32::from(w4[0]);
        }
    }
    // do the bottom three that the inner loop may have failed to parse
    if height % 4 != 0
    {
        let rows_unhanded = (in_image.len() / width) % 4;

        for (in_stride, out_stride) in in_image
            .rchunks_exact(width)
            .zip(out_image.rchunks_exact_mut(width))
            .take(rows_unhanded)
        {
            let mut a1 = 0;

            for i in in_stride.iter().take(radius)
            {
                a1 += u32::from(*i);
            }
            let r1 = u32::from(in_stride[0]);

            for (w1, o1) in in_stride.windows(radius).zip(out_stride[2..].iter_mut())
            {
                a1 = a1.wrapping_add(u32::from(w1[radius - 1]).wrapping_sub(r1));
                *o1 = fastdiv_u32(a1, m_radius) as u8;
            }
        }
    }
}

#[test]
fn check_results()
{
    let mut t: Vec<u8> = (0..=255).collect();
    let mut u = vec![0; 256];
    let v: Vec<u8> = (0..255).collect();
    box_blur(&mut t, &mut u, 16, 16, 3);
    for s in t.chunks_exact(16)
    {
        println!("{:?}", s)
    }
}
