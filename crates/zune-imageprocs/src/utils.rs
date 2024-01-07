/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_image::image::Image;

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
#[allow(dead_code, unused_variables)]
pub fn z_prefetch<T>(data: &[T], position: usize) {
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

/// The position of the source image on the destination
#[derive(Copy, Clone, Debug)]
pub enum Gravity {
    /// Place the image so that it seems like it's from the
    /// center of the canvas
    Center,
    /// Place the image so that it is from the top end of the canvas
    TopLeft,
    /// Place the src image so that it appears from the right of the canvas
    TopRight,
    /// Place the image so that it appears from the bottom left of the canvas
    BottomLeft,
    /// Place the image so that it appears from the bottom right of the canvas
    BottomRight
}

pub fn calculate_gravity(src_image: &Image, dst_image: &Image, gravity: Gravity) -> (usize, usize) {
    let (src_width, src_height) = src_image.dimensions();
    let (dst_width, dst_height) = dst_image.dimensions();

    return match gravity {
        Gravity::Center => {
            let dst_center_x = dst_width / 2;
            let dst_center_y = dst_height / 2;

            let src_center_x = src_width / 2;
            let src_center_y = src_height / 2;

            let orig_y = dst_center_y.saturating_sub(src_center_y);
            let orig_x = dst_center_x.saturating_sub(src_center_x);

            (orig_x, orig_y)
        }
        Gravity::TopLeft => (0, 0),
        Gravity::TopRight => (dst_width.saturating_sub(src_width), 0),
        Gravity::BottomLeft => (0, dst_height.saturating_sub(src_height)),
        Gravity::BottomRight => (
            dst_width.saturating_sub(src_width),
            dst_height.saturating_sub(src_height)
        )
    };
}
