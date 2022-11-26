/// Prefetch data at offset position
///
/// This uses prefetch intrinsics for a specific
/// platform to hint the CPU  that the data at that position
/// will be needed at a later time.
///
/// # Platform specific behaviour
/// - On x86, we use `_MM_HINT_T0` which prefetches to all levels of cache
/// hence it may cause cache pollution
///
/// # Arguments
///  - data: A long slice with some data not in the cache
///  - position: The position of data we expect to fetch that we think
/// is not in the cache.
#[inline(always)]
pub fn z_prefetch<T>(data: &[T], position: usize)
{
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86")]
        use core::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use core::arch::x86_64::*;
        unsafe {
            // we don't need to worry for this failing
            let ptr_position = data.as_ptr().add(position).cast::<i8>();

            _mm_prefetch::<_MM_HINT_T0>(ptr_position);
        }
    }
}
