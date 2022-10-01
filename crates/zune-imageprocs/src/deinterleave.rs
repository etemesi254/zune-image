#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "avx2")]
use crate::deinterleave::avx2::de_interleave_3_channels_avx2;
use crate::deinterleave::scalar::de_interleave_3_channels_scalar;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "sse2")]
use crate::deinterleave::sse2::de_interleave_3_channels_sse2;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "sse3")]
use crate::deinterleave::sse3::de_interleave_3_channels_sse3;

mod avx2;
mod scalar;
mod sse2;
mod sse3;

pub fn de_interleave_3_channels(source: &[u8], (c1, c2, c3): (&mut [u8], &mut [u8], &mut [u8]))
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx2")]
        {
            if is_x86_feature_detected!("avx2")
            {
                unsafe {
                    return de_interleave_3_channels_avx2(source, (c1, c2, c3));
                }
            }
        }
        #[cfg(feature = "sse3")]
        {
            if is_x86_feature_detected!("sse3")
            {
                unsafe {
                    return de_interleave_3_channels_sse3(source, (c1, c2, c3));
                }
            }
        }
        #[cfg(feature = "sse2")]
        {
            if is_x86_feature_detected!("sse2")
            {
                unsafe {
                    return de_interleave_3_channels_sse2(source, (c1, c2, c3));
                }
            }
        }
    }
    de_interleave_3_channels_scalar(source, (c1, c2, c3))
}
