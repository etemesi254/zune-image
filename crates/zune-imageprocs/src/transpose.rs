use log::{debug, trace};

use crate::transpose::scalar::transpose_scalar;

mod scalar;
mod sse41;

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
