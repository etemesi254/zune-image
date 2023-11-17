/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Overloadable operators
//!
//! This module provides overloadable operators for the image struct
//!
//! The operations panic in case of the incompatibility between two images
//! so it is best you ensure the image struct is the same
//!
use core::ops::*;

use zune_core::bit_depth::BitType;

use crate::image::Image;

#[track_caller]
fn check_full_compatibility(img1: &Image, img2: &Image) {
    assert_eq!(
        img1.depth(),
        img2.depth(),
        "Image depth mismatch cannot continue {:?}!={:?}",
        img1.depth(),
        img2.depth()
    );

    assert_eq!(
        img1.dimensions(),
        img2.dimensions(),
        "Dimensions mismatch, {:?} != {:?}",
        img1.dimensions(),
        img2.dimensions()
    );

    assert_eq!(
        img1.colorspace(),
        img2.colorspace(),
        "Colorspace mismatch, {:?} != {:?}",
        img1.colorspace(),
        img2.colorspace()
    );
}

impl Add for Image {
    type Output = Image;

    fn add(self, rhs: Image) -> Self::Output {
        check_full_compatibility(&self, &rhs);

        let mut new_img = self;

        match new_img.depth().bit_type() {
            BitType::U8 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<u8>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<u8>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x = x.saturating_add(*y));
                }
            }
            BitType::U16 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<u16>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<u16>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x = x.saturating_add(*y));
                }
            }
            BitType::F32 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<f32>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<f32>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x += *y);
                }
            }
            d => unimplemented!("Unimplemented for {:?}", d)
        }
        new_img
    }
}

impl Sub for Image {
    type Output = Image;

    fn sub(self, rhs: Image) -> Self::Output {
        check_full_compatibility(&self, &rhs);

        let mut new_img = self;

        match new_img.depth().bit_type() {
            BitType::U8 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<u8>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<u8>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x = x.saturating_sub(*y));
                }
            }
            BitType::U16 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<u16>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<u16>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x = x.saturating_sub(*y));
                }
            }
            BitType::F32 => {
                for (prev, rhs_c) in new_img
                    .channels_mut(true)
                    .iter_mut()
                    .zip(rhs.channels_ref(true))
                {
                    let channel_px = prev.reinterpret_as_mut::<f32>().unwrap();
                    let channel_rhs = rhs_c.reinterpret_as::<f32>().unwrap();

                    channel_px
                        .iter_mut()
                        .zip(channel_rhs.iter())
                        .for_each(|(x, y)| *x -= *y);
                }
            }
            d => unimplemented!("Unimplemented for {:?}", d)
        }
        new_img
    }
}

#[test]
fn add() {
    let im = Image::fill(0_u8, zune_core::colorspace::ColorSpace::RGBA, 100, 100);
    let _ = im.clone() + im;
}
