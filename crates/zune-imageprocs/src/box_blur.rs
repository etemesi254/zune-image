use log::warn;

use crate::mathops::{compute_mod_u32, fastdiv_u32};
use crate::traits::NumOps;
use crate::transpose;

pub fn box_blur_u16(
    in_out_image: &mut [u16], scratch_space: &mut [u16], width: usize, height: usize, radius: usize
)
{
    if width == 0 || radius <= 1
    {
        warn!("Box blur with radius less than or equal to 1 does nothing");
        return;
    }
    box_blur_inner(in_out_image, scratch_space, width, height, radius);
    transpose::transpose_u16(scratch_space, in_out_image, width, height);
    box_blur_inner(in_out_image, scratch_space, height, width, radius);
    transpose::transpose_u16(scratch_space, in_out_image, height, width);
}

pub fn box_blur_u8(
    in_out_image: &mut [u8], scratch_space: &mut [u8], width: usize, height: usize, radius: usize
)
{
    if width == 0 || radius <= 1
    {
        warn!("Box blur with radius less than or equal to 1 does nothing");
        return;
    }
    box_blur_inner(in_out_image, scratch_space, width, height, radius);
    transpose::transpose_u8(scratch_space, in_out_image, width, height);
    box_blur_inner(in_out_image, scratch_space, height, width, radius);
    transpose::transpose_u8(scratch_space, in_out_image, height, width);
}

#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
fn box_blur_inner<T>(
    in_image: &[T], out_image: &mut [T], width: usize, height: usize, radius: usize
) where
    T: Copy + NumOps<T>,
    u32: std::convert::From<T>
{
    // 1D-Box blurs can be seen as the average of radius pixels iterating
    // through a window
    // A box blur therefore is
    //
    // pix[x,y]= (pix[x-r/2,y]...+pix[x,y]+...pix[x+r/2,y])/r
    //
    // The naive operation is slow, due to a lot of reasons, so here we use a slightly more
    // optimized version
    //
    // One thing to see is that 1D box blurs are independent per width stride
    // ie. calculating row y is independent of calculating row y+1, with this info
    // we can do a bit of loop unrolling to better utilize ILP.
    //
    // Furthermore, notice we are dividing by r, which is a constant across all rows,
    // But division is a slow instruction, hence we can replace it with multiplication by some
    // weird constant, that eliminates that in the inner loop, credits to Daniel Lemire's fastmod for that
    //
    // Further more there is no need to sum up a window per iteration, we can simply implement it by looking at what is changing
    // For any iteration the sum is window[n], and for n+1 sum is window[n+1], but what changed was nothing
    // i.e what changed was windows[n-r/2] was dropped and windows [n+r/2] was added.
    // So if we keep the terms window[n-r/2] and windows [n+r/2] the summing becomes
    // sum(windows[n]) = a - windows[r]-windows[0]
    // where a is sum of chunk[0..r], (first of the array), we can keep updating a during the loop
    // and we have a window sum!

    if width <= 1 || radius <= 1
    {
        // repeated here for the optimizer
        return;
    }
    let radius = radius.min(width);
    let m_radius = compute_mod_u32(radius as u64);

    {
        // handle pixels inside boundaries

        for (four_stride_in, four_stride_out) in in_image
            .chunks_exact(width * 4)
            .zip(out_image.chunks_exact_mut(width * 4))
        {
            // output strides
            let (os1, rem) = four_stride_out.split_at_mut(width);
            let (os2, rem) = rem.split_at_mut(width);
            let (os3, os4) = rem.split_at_mut(width);

            // input strides
            let (ws1, rem) = four_stride_in.split_at(width);
            let (ws2, rem) = rem.split_at(width);
            let (ws3, ws4) = rem.split_at(width);

            // do the first accumulation
            let (mut a1, mut a2, mut a3, mut a4) = (0, 0, 0, 0);
            let mut p = 1;

            for ((((pos, n1), n2), n3), n4) in ws1
                .iter()
                .enumerate()
                .zip(ws2.iter())
                .zip(ws3.iter())
                .zip(ws4.iter())
                .take(radius - 1)
            {
                a1 += u32::from(*n1);
                a2 += u32::from(*n2);
                a3 += u32::from(*n3);
                a4 += u32::from(*n4);

                // Handle edge pixels
                os1[pos] = T::from_u32(a1 / p);
                os2[pos] = T::from_u32(a2 / p);
                os3[pos] = T::from_u32(a3 / p);
                os4[pos] = T::from_u32(a4 / p);
                p += 1;
            }
            // some won't be handled explicitly by the loop
            // handle it here
            os1[radius - 1] = T::from_u32(a1 / p);
            os2[radius - 1] = T::from_u32(a2 / p);
            os3[radius - 1] = T::from_u32(a3 / p);
            os4[radius - 1] = T::from_u32(a4 / p);

            let mut r1 = 0;
            let mut r2 = 0;
            let mut r3 = 0;
            let mut r4 = 0;

            let mut pos = 0;

            for (((((((o1, o2), o3), o4), w1), w2), w3), w4) in os1[radius / 2..]
                .iter_mut()
                .zip(os2[radius / 2..].iter_mut())
                .zip(os3[radius / 2..].iter_mut())
                .zip(os4[radius / 2..].iter_mut())
                .zip(ws1.windows(radius))
                .zip(ws2.windows(radius))
                .zip(ws3.windows(radius))
                .zip(ws4.windows(radius))
            {
                a1 = a1.wrapping_add(u32::from(w1[radius - 1])).wrapping_sub(r1);
                *o1 = T::from_u32(fastdiv_u32(a1, m_radius));

                a2 = a2.wrapping_add(u32::from(w2[radius - 1])).wrapping_sub(r2);
                *o2 = T::from_u32(fastdiv_u32(a2, m_radius));

                a3 = a3.wrapping_add(u32::from(w3[radius - 1])).wrapping_sub(r3);
                *o3 = T::from_u32(fastdiv_u32(a3, m_radius));

                a4 = a4.wrapping_add(u32::from(w4[radius - 1])).wrapping_sub(r4);
                *o4 = T::from_u32(fastdiv_u32(a4, m_radius));

                r1 = u32::from(w1[0]);
                r2 = u32::from(w2[0]);
                r3 = u32::from(w3[0]);
                r4 = u32::from(w4[0]);

                pos += 1;
            }

            let mut p = (radius + 1) as u32 / 2;

            for (((n1, n2), n3), n4) in ws1
                .iter()
                .rev()
                .zip(ws2.iter().rev())
                .zip(ws3.iter().rev())
                .zip(ws4.iter().rev())
                .take((radius + 1) / 2)
            {
                a1 -= u32::from(*n1);
                a2 -= u32::from(*n2);
                a3 -= u32::from(*n3);
                a4 -= u32::from(*n4);

                // Handle edge pixels
                os1[pos] = T::from_u32(a1 / p);
                os2[pos] = T::from_u32(a2 / p);
                os3[pos] = T::from_u32(a3 / p);
                os4[pos] = T::from_u32(a4 / p);
                p -= 1;
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
                let mut p = 1;

                for (pos, i) in in_stride.iter().take(radius).enumerate()
                {
                    a1 += u32::from(*i);
                    out_stride[pos] = T::from_u32(a1 / p);
                    p += 1;
                }
                out_stride[radius - 1] = T::from_u32(a1 / p);

                let mut r1 = 0;

                for (w1, o1) in in_stride
                    .windows(radius)
                    .zip(out_stride[radius / 2..].iter_mut())
                {
                    a1 = a1.wrapping_add(u32::from(w1[radius - 1])).wrapping_sub(r1);
                    *o1 = T::from_u32(fastdiv_u32(a1, m_radius));
                    r1 = u32::from(w1[0]);
                }
            }
        }
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::box_blur::{box_blur_u16, box_blur_u8};

    #[bench]
    fn bench_box_blur_u16(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        b.iter(|| {
            box_blur_u16(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }

    #[bench]
    fn bench_box_blur_u8(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let radius = 10;
        let dimensions = width * height;
        let mut in_vec = vec![255; dimensions];
        let mut scratch_space = vec![0; dimensions];

        b.iter(|| {
            box_blur_u8(&mut in_vec, &mut scratch_space, width, height, radius);
        });
    }
}

#[test]
fn test_blur()
{
    let width = 800;
    let height = 800;
    let radius = 10;
    let dimensions = width * height;
    let mut in_vec = vec![255; dimensions];
    let mut scratch_space = vec![0; dimensions];

    box_blur_u16(&mut in_vec, &mut scratch_space, width, height, radius);
}
