#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "avx2")]

use crate::deinterleave::scalar::de_interleave_three_channels_scalar;

#[target_feature(enable = "avx2")]
pub(crate) unsafe fn de_interleave_three_channels_avx2(
    source: &[u8], (c1, c2, c3): (&mut [u8], &mut [u8], &mut [u8]),
)
{
    // Rely on the auto-vectorizer
    // it does a decent work, i.e https://godbolt.org/z/W8brrdv4K
    // and I'm too lazy to figure out mine.
    de_interleave_three_channels_scalar(source, (c1, c2, c3));
}
