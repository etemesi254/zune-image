#[macro_export]
macro_rules! simd_fn {
    (
        $(#[$meta:meta])*
        fn $name:ident $(<$($gen:tt)*>)? (
            $($arg:ident : $argty:ty),* $(,)?
        ) $body:block
    ) => {
        $(#[$meta])*
       pub (crate) fn $name $(<$($gen)*>)? (
            $($arg : $argty),*
        ) {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                if std::arch::is_x86_feature_detected!("avx2") {
                    #[target_feature(enable = "avx2")]
                    unsafe fn inner $(<$($gen)*>)? (
                        $($arg : $argty),*
                    ) {
                        $body
                    }

                    return unsafe {
                        inner($($arg),*)
                    };
                }
            }

            #[cfg(target_arch = "aarch64")]
            {
                if std::arch::is_aarch64_feature_detected!("neon") {
                    #[target_feature(enable = "neon")]
                    unsafe fn inner $(<$($gen)*>)? (
                        $($arg : $argty),*
                    ) {
                        $body
                    }

                    return unsafe {
                        inner($($arg),*)
                    };
                }
            }

            #[inline(always)]
            fn fallback $(<$($gen)*>)? (
                $($arg : $argty),*
            ) {
                $body
            }

            fallback($($arg),*)
        }
    };
}


