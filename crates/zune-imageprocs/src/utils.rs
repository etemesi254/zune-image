/// Prefetch data at offset position
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
