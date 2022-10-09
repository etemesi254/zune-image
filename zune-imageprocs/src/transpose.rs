use log::debug;

use crate::transpose::scalar::transpose_scalar;

pub(crate) mod scalar;
pub(crate) mod sse41;

pub fn transpose(in_matrix: &[u8], out_matrix: &mut [u8], width: usize, height: usize)
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(feature = "sse41")]
        {
            use crate::transpose::sse41::transpose_sse41;

            if is_x86_feature_detected!("sse4.1")
            {
                debug!("Using SSE4.1 transpose algorithm");
                unsafe { return transpose_sse41(in_matrix, out_matrix, width, height) }
            }
        }
    }
    debug!("Using scalar transpose algorithm");
    transpose_scalar(in_matrix, out_matrix, width, height);
}

#[cfg(all(feature = "benchmarks", feature = "sse41"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    #[bench]
    fn transpose_sse(b: &mut test::Bencher)
    {
        use crate::transpose::sse41::transpose_sse41;
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255; dimensions];
        let mut out_vec = vec![0; dimensions];
        b.iter(|| {
            unsafe {
                transpose_sse41(&in_vec, &mut out_vec, width, height);
            };
        })
    }
    #[bench]
    fn transpose_scalar(b: &mut test::Bencher)
    {
        use crate::transpose::scalar::transpose_scalar;
        let width = 800;
        let height = 800;
        let dimensions = width * height;
        let in_vec = vec![255; dimensions];
        let mut out_vec = vec![0; dimensions];
        b.iter(|| {
            transpose_scalar(&in_vec, &mut out_vec, width, height);
        })
    }
}
