//! A set of miscellaneous functions that are good to have
use std::cmp::min;

use zune_core::bytestream::ZByteReaderTrait;

use crate::channel::Channel;
use crate::errors::ImageErrors;
use crate::metadata::ImageMetadata;

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
fn swizzle_three_channels_fallback<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
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
fn swizzle_four_channels_fallback<T: Copy + Default>(r: &[&[T]], y: &mut [T]) {
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

/// Combine separate channels into one contiguous array
///
/// This interleaves each individual channels pixels into one contiguous array
///
/// it supports up to 4 channels (library default).
///
/// # Arguments:
///  - channels: A Slice of channels , the count must be less than 4 anf greater than 1.
/// -  output: Output array of type `T`, the length should be greater or equal to
///   `channels[0].len()/size_of::<T> * channels.len()`, but the library doesn't check this is held
///    In case it's smaller, the function will ignore
///
/// # Returns:
/// The bytes written
///
/// # Examples
/// - create a RGB image and convert it to interleaved
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_image::utils::swizzle_channels;
/// use zune_image::errors::ImageErrors;
///
/// fn main()->Result<(),ImageErrors>{
///     let mut image = Image::fill::<u8>(128,ColorSpace::RGB,100,100);
///     // output must be w*h*color_channels
///     // so create that length
///     let mut storage= vec![0_u8;100*image.colorspace().num_components()];
///     
///     for frame in image.frames_mut(){
///         let channels = frame.channels_vec();
///         swizzle_channels(&channels,&mut storage)?;
///     }
///     Ok(())
/// }
/// ```
pub fn swizzle_channels<T: Copy + Default + 'static>(
    channels: &[Channel], output: &mut [T]
) -> Result<usize, ImageErrors> {
    match channels.len() {
        // copy
        1 => {
            output.copy_from_slice(channels[0].reinterpret_as()?);
            Ok(output.len())
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
            let size = min(channels[0].reinterpret_as::<T>()?.len() * 2, output.len());
            Ok(size)
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

            let size = min(channels[0].reinterpret_as::<T>()?.len() * 3, output.len());
            Ok(size)
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

            let size = min(channels[0].reinterpret_as::<T>()?.len() * 4, output.len());
            Ok(size)
        }
        _ => Err(ImageErrors::GenericStr(
            "Image channels not in supported count, the library supports images from 1-4 channels"
        ))
    }
}

pub fn decode_info<T: ZByteReaderTrait>(bytes: T) -> Option<ImageMetadata> {
    match crate::codecs::guess_format(bytes) {
        None => None,
        Some((format, bytes)) => {
            let mut decoder = format.decoder(bytes).ok()?;
            decoder.read_headers().ok()?
        }
    }
}
