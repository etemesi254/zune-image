use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;

/// Swizzle three channels optionally using simd intrinsics where possible
fn swizzle_three_channels<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // Note that this `unsafe` block is safe because we're testing
        // that the `avx2` feature is indeed available on our CPU.
        if is_x86_feature_detected!("avx2") {
            return unsafe { swizzle_three_channels_avx(r, y) };
        }
    }
    swizzle_three_channels_fallback(r, y);
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn swizzle_three_channels_avx<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    swizzle_three_channels_fallback(r, y); // the function below is inlined here
}

#[inline(always)]
pub fn swizzle_three_channels_fallback<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    // now swizzle
    assert_eq!(r.len(), 3);

    for (((output, a), b), c) in y
        .chunks_exact_mut(3)
        .zip(r[0].iter())
        .zip(r[1].iter())
        .zip(r[2].iter())
    {
        output[0] = *a;
        output[1] = *b;
        output[2] = *c;
    }
}

fn swizzle_four_channels<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        // Note that this `unsafe` block is safe because we're testing
        // that the `avx2` feature is indeed available on our CPU.
        if is_x86_feature_detected!("avx2") {
            return unsafe { swizzle_four_channels_avx(r, y) };
        }
    }
    swizzle_four_channels_fallback(r, y);
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn swizzle_four_channels_avx<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    swizzle_four_channels_fallback(r, y); // the function below is inlined here
}

#[inline(always)]
pub fn swizzle_four_channels_fallback<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
    // now swizzle
    assert_eq!(r.len(), 4);

    for ((((output, a), b), c), d) in y
        .chunks_exact_mut(4)
        .zip(r[0].iter())
        .zip(r[1].iter())
        .zip(r[2].iter())
        .zip(r[3].iter())
    {
        output[0] = *a;
        output[1] = *b;
        output[2] = *c;
        output[3] = *d;
    }
}

/// Convert channel output to a linear representation.
///
/// Useful when you need to combine separate bands to be a single interleaved output
pub fn channels_to_linear<T: Copy + Default + 'static>(
    channels: &[Channel], output: &mut [T]
) -> Result<(), ImageErrors> {
    match channels.len() {
        // copy
        1 => {
            output.copy_from_slice(channels[0].reinterpret_as()?);
            Ok(())
        }
        2 => {
            for ((output, a), b) in output
                .chunks_exact_mut(2)
                .zip(channels[0].reinterpret_as()?.iter())
                .zip(channels[1].reinterpret_as()?.iter())
            {
                output[0] = *a;
                output[1] = *b;
            }
            Ok(())
        }
        3 => {
            // three components are usually quite common, so for those use one which can be
            // autovectorized.
            //
            // Image dimensions (w,h,colorspace)
            // (2384, 4240, 3)
            // Before:
            //  7.4 ms
            // After:
            //  4.5 ms
            let mut r = vec![];
            for c in channels {
                r.push(c.reinterpret_as()?);
            }
            swizzle_three_channels(&r, output);
            Ok(())
        }
        4 => {
            // now swizzle
            // also carry it in simd where applicable
            //
            // TODO: We should add ARM too for this.

            let mut r = vec![];
            for c in channels {
                r.push(c.reinterpret_as()?);
            }
            swizzle_four_channels(&r, output);
            Ok(())
        }
        _ => Err(ImageErrors::GenericStr(
            "Image channels not in supported count, the library supports images from 1-4 channels"
        ))
    }
}
