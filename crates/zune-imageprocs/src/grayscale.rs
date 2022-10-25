mod avx2;
mod scalar;
mod sse41;

use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;

pub fn rgb_to_grayscale((r, g, b): (&[u8], &[u8], &[u8]), out: &mut [u8])
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx2")]
        {
            use crate::grayscale::avx2::convert_rgb_to_grayscale_avx2;

            if is_x86_feature_detected!("avx2")
            {
                unsafe {
                    return convert_rgb_to_grayscale_avx2((r, g, b), out);
                }
            }
        }

        #[cfg(feature = "sse41")]
        {
            use crate::grayscale::sse41::convert_rgb_to_grayscale_sse41;

            if is_x86_feature_detected!("sse4.1")
            {
                unsafe {
                    return convert_rgb_to_grayscale_sse41((r, g, b), out);
                }
            }
        }
    }
    convert_rgb_to_grayscale_scalar((r, g, b), out);
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    #[cfg(feature = "sse41")]
    #[bench]
    fn convert_rgb_to_grayscale_sse41_bench(b: &mut test::Bencher)
    {
        use crate::grayscale::sse41::convert_rgb_to_grayscale_sse41;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0; dimensions];
        let c2 = vec![0; dimensions];
        let c3 = vec![0; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            unsafe {
                convert_rgb_to_grayscale_sse41((&c1, &c2, &c3), &mut c4);
            };
        });
    }

    #[cfg(feature = "avx2")]
    #[bench]
    fn convert_rgb_to_grayscale_avx2_bench(b: &mut test::Bencher)
    {
        use crate::grayscale::avx2::convert_rgb_to_grayscale_avx2;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0; dimensions];
        let c2 = vec![0; dimensions];
        let c3 = vec![0; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            unsafe {
                convert_rgb_to_grayscale_avx2((&c1, &c2, &c3), &mut c4);
            };
        });
    }

    #[bench]
    fn convert_rgb_to_grayscale_scalar_bench(b: &mut test::Bencher)
    {
        use crate::grayscale::scalar::convert_rgb_to_grayscale_scalar;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let c1 = vec![0; dimensions];
        let c2 = vec![0; dimensions];
        let c3 = vec![0; dimensions];

        let mut c4 = vec![255; dimensions];
        b.iter(|| {
            convert_rgb_to_grayscale_scalar((&c1, &c2, &c3), &mut c4);
        });
    }
}
