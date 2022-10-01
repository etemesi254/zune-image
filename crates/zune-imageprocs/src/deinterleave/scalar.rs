#[inline(always)]
pub (crate) fn de_interleave_3_channels_scalar(
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