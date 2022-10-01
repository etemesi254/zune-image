mod avx2;
mod scalar;
mod sse41;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "avx2")]
use crate::grayscale::avx2::convert_rgb_to_grayscale_avx2;
use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(feature = "sse41")]
use crate::grayscale::sse41::convert_rgb_to_grayscale_sse41;

pub fn rgb_to_grayscale((r, g, b): (&[u8], &[u8], &[u8]), out: &mut [u8])
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx2")]
        {
            if is_x86_feature_detected!("avx2")
            {
                unsafe {
                    return convert_rgb_to_grayscale_avx2((r, g, b), out);
                }
            }
        }

        #[cfg(feature = "sse41")]
        {
            if is_x86_feature_detected!("sse4.1")
            {
                unsafe {
                    return convert_rgb_to_grayscale_sse41((r, g, b), out);
                }
            }
        }
    }
    convert_rgb_to_grayscale_scalar((r, g, b), out)
}
