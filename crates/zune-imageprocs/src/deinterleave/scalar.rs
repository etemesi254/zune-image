#[inline(always)]
pub(crate) fn de_interleave_three_channels_scalar(
    source: &[u8], (c1, c2, c3): (&mut [u8], &mut [u8], &mut [u8]),
)
{
    for (((val, a), b), c) in source
        .chunks_exact(3)
        .zip(c1.iter_mut())
        .zip(c2.iter_mut())
        .zip(c3.iter_mut())
    {
        *a = val[0];
        *b = val[1];
        *c = val[2];
    }
}

pub fn de_interleave_four_channels_scalar(
    source: &[u8], (c1, c2, c3, c4): (&mut [u8], &mut [u8], &mut [u8], &mut [u8]),
)
{
    for ((((src, c11), c22), c33), c44) in source
        .chunks_exact(4)
        .zip(c1.iter_mut())
        .zip(c2.iter_mut())
        .zip(c3.iter_mut())
        .zip(c4.iter_mut())
    {
        *c11 = src[0];
        *c22 = src[1];
        *c33 = src[2];
        *c44 = src[3];
    }
}
