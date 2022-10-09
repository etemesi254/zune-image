use crate::deinterleave::scalar::de_interleave_3_channels_scalar;

mod avx2;
mod scalar;
mod sse2;
mod sse41;

pub fn de_interleave_3_channels(source: &[u8], (c1, c2, c3): (&mut [u8], &mut [u8], &mut [u8]))
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "avx2")]
        {
            use crate::deinterleave::avx2::de_interleave_3_channels_avx2;

            if is_x86_feature_detected!("avx2")
            {
                unsafe {
                    return de_interleave_3_channels_avx2(source, (c1, c2, c3));
                }
            }
        }
        #[cfg(feature = "sse3")]
        {
            use crate::deinterleave::sse41::de_interleave_3_channels_sse3;

            if is_x86_feature_detected!("sse3")
            {
                unsafe {
                    return de_interleave_3_channels_sse3(source, (c1, c2, c3));
                }
            }
        }
        #[cfg(feature = "sse2")]
        {
            use crate::deinterleave::sse2::de_interleave_3_channels_sse2;

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

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    #[cfg(feature = "sse2")]
    #[bench]
    fn de_interleave_3_channels_sse2_bench(b: &mut test::Bencher)
    {
        use crate::deinterleave::sse2::de_interleave_3_channels_sse2;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0; dimensions];
        let mut c2 = vec![0; dimensions];
        let mut c3 = vec![0; dimensions];

        let c4 = vec![255; dimensions * 3];
        b.iter(|| {
            unsafe {
                de_interleave_3_channels_sse2((&c4), (&mut c1, &mut c2, &mut c3));
            };
        })
    }
    #[cfg(feature = "sse41")]
    #[bench]
    fn de_interleave_3_channels_sse41_bench(b: &mut test::Bencher)
    {
        use crate::deinterleave::sse41::de_interleave_3_channels_sse3;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0; dimensions];
        let mut c2 = vec![0; dimensions];
        let mut c3 = vec![0; dimensions];

        let c4 = vec![255; dimensions * 3];
        b.iter(|| {
            unsafe {
                de_interleave_3_channels_sse3((&c4), (&mut c1, &mut c2, &mut c3));
            };
        })
    }
    #[cfg(feature = "avx2")]
    #[bench]
    fn de_interleave_3_channels_avx2_bench(b: &mut test::Bencher)
    {
        use crate::deinterleave::avx2::de_interleave_3_channels_avx2;
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0; dimensions];
        let mut c2 = vec![0; dimensions];
        let mut c3 = vec![0; dimensions];

        let c4 = vec![255; dimensions * 3];
        b.iter(|| {
            unsafe {
                de_interleave_3_channels_avx2((&c4), (&mut c1, &mut c2, &mut c3));
            };
        })
    }
}
