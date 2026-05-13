/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Sse capable defilter routines.
//!
//! These techniques enable faster png de-filtering in
//! situations where sse is available i.e on x86 arch
//!
//! They are derived from the amazing [spng](https://github.com/randy408/libspng)
//!
//! which derived them from libpng and hence they are governed by that license

// COPYRIGHT NOTICE, DISCLAIMER, and LICENSE
// =========================================
//
// PNG Reference Library License version 2
// ---------------------------------------
//
//  * Copyright (c) 1995-2022 The PNG Reference Library Authors.
//  * Copyright (c) 2018-2022 Cosmin Truta.
//  * Copyright (c) 2000-2002, 2004, 2006-2018 Glenn Randers-Pehrson.
//  * Copyright (c) 1996-1997 Andreas Dilger.
//  * Copyright (c) 1995-1996 Guy Eric Schalnat, Group 42, Inc.
//
// The software is supplied "as is", without warranty of any kind,
// express or implied, including, without limitation, the warranties
// of merchantability, fitness for a particular purpose, title, and
// non-infringement.  In no event shall the Copyright owners, or
// anyone distributing the software, be liable for any damages or
// other liability, whether in contract, tort or otherwise, arising
// from, out of, or in connection with the software, or the use or
// other dealings in the software, even if advised of the possibility
// of such damage.
//
// Permission is hereby granted to use, copy, modify, and distribute
// this software, or portions hereof, for any purpose, without fee,
// subject to the following restrictions:
//
//  1. The origin of this software must not be misrepresented; you
//     must not claim that you wrote the original software.  If you
//     use this software in a product, an acknowledgment in the product
//     documentation would be appreciated, but is not required.
//
//  2. Altered source versions must be plainly marked as such, and must
//     not be misrepresented as being the original software.
//
//  3. This Copyright notice may not be removed or altered from any
//     source or altered source distribution.
//
//
// PNG Reference Library License version 1 (for libpng 0.5 through 1.6.35)
// -----------------------------------------------------------------------
//
// libpng versions 1.0.7, July 1, 2000, through 1.6.35, July 15, 2018 are
// Copyright (c) 2000-2002, 2004, 2006-2018 Glenn Randers-Pehrson, are
// derived from libpng-1.0.6, and are distributed according to the same
// disclaimer and license as libpng-1.0.6 with the following individuals
// added to the list of Contributing Authors:
//
//     Simon-Pierre Cadieux
//     Eric S. Raymond
//     Mans Rullgard
//     Cosmin Truta
//     Gilles Vollant
//     James Yu
//     Mandar Sahastrabuddhe
//     Google Inc.
//     Vadim Barkov
//
// and with the following additions to the disclaimer:
//
//     There is no warranty against interference with your enjoyment of
//     the library or against infringement.  There is no warranty that our
//     efforts or the library will fulfill any of your particular purposes
//     or needs.  This library is provided with all faults, and the entire
//     risk of satisfactory quality, performance, accuracy, and effort is
//     with the user.
//
// Some files in the "contrib" directory and some configure-generated
// files that are distributed with libpng have other copyright owners, and
// are released under other open source licenses.
//
// libpng versions 0.97, January 1998, through 1.0.6, March 20, 2000, are
// Copyright (c) 1998-2000 Glenn Randers-Pehrson, are derived from
// libpng-0.96, and are distributed according to the same disclaimer and
// license as libpng-0.96, with the following individuals added to the
// list of Contributing Authors:
//
//     Tom Lane
//     Glenn Randers-Pehrson
//     Willem van Schaik
//
// libpng versions 0.89, June 1996, through 0.96, May 1997, are
// Copyright (c) 1996-1997 Andreas Dilger, are derived from libpng-0.88,
// and are distributed according to the same disclaimer and license as
// libpng-0.88, with the following individuals added to the list of
// Contributing Authors:
//
//     John Bowler
//     Kevin Bracey
//     Sam Bushell
//     Magnus Holmgren
//     Greg Roelofs
//     Tom Tanner
//
// Some files in the "scripts" directory have other copyright owners,
// but are released under this license.
//
// libpng versions 0.5, May 1995, through 0.88, January 1996, are
// Copyright (c) 1995-1996 Guy Eric Schalnat, Group 42, Inc.
//
// For the purposes of this copyright and license, "Contributing Authors"
// is defined as the following set of individuals:
//
//     Andreas Dilger
//     Dave Martindale
//     Guy Eric Schalnat
//     Paul Schmidt
//     Tim Wegner
//
// The PNG Reference Library is supplied "AS IS".  The Contributing
// Authors and Group 42, Inc. disclaim all warranties, expressed or
// implied, including, without limitation, the warranties of
// merchantability and of fitness for any purpose.  The Contributing
// Authors and Group 42, Inc. assume no liability for direct, indirect,
// incidental, special, exemplary, or consequential damages, which may
// result from the use of the PNG Reference Library, even if advised of
// the possibility of such damage.
//
// Permission is hereby granted to use, copy, modify, and distribute this
// source code, or portions hereof, for any purpose, without fee, subject
// to the following restrictions:
//
//  1. The origin of this source code must not be misrepresented.
//
//  2. Altered versions must be plainly marked as such and must not
//     be misrepresented as being the original source.
//
//  3. This Copyright notice may not be removed or altered from any
//     source or altered source distribution.
//
// The Contributing Authors and Group 42, Inc. specifically permit,
// without fee, and encourage the use of this source code as a component
// to supporting the PNG file format in commercial products.  If you use
// this source code in a product, acknowledgment is not required but would
// be appreciated.

//  Safety of Platform specific intrinsics
// ---------------------------------------------
//
// To correctly support non std variants we depend on compilation strategies
// , when zune-core is compiled, it either chooses dynamic platform detection using
// is_x86_feature_detected!!() if we link to std or we choose based on compilation strategies
//
// Here we offer another check to see if we have the features, but that only works for non-std variants
// since the feature detection needs linking to std.
//
//

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse")]

#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[allow(unused_assignments)]
#[target_feature(enable = "sse2")]
unsafe fn de_filter_sub_generic_sse2<const SIZE: usize>(raw: &[u8], current: &mut [u8]) {
    let mut zero = [0; 16];
    let (mut a, mut d) = (_mm_setzero_si128(), _mm_setzero_si128());

    for (raw, out) in raw.chunks_exact(SIZE).zip(current.chunks_exact_mut(SIZE)) {
        zero[0..SIZE].copy_from_slice(raw);

        a = d;
        d = _mm_loadu_si128(zero.as_ptr().cast());
        d = _mm_add_epi8(d, a);
        _mm_storeu_si128(zero.as_mut_ptr().cast(), d);

        out.copy_from_slice(&zero[0..SIZE]);
    }
}

pub fn de_filter_sub_sse2<const SIZE: usize>(raw: &[u8], current: &mut [u8]) {
    #[cfg(feature = "std")]
    {
        if !is_x86_feature_detected!("sse2") {
            panic!("Internal error, calling platform specific function where not supported")
        }
    }
    unsafe { de_filter_sub_generic_sse2::<SIZE>(raw, current) }
}

#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn if_then_else(c: __m128i, t: __m128i, e: __m128i) -> __m128i {
    _mm_blendv_epi8(e, t, c)

    // SSE 2
    //return _mm_or_si128(_mm_and_si128(c, t), _mm_andnot_si128(c, e));
}

// 16 bpp RGBA SSE filtering code
#[target_feature(enable = "sse4.1")]
#[allow(unused_assignments)]
unsafe fn de_filter_paeth_sse41_inner<const SIZE: usize>(
    prev_row: &[u8],
    raw: &[u8],
    current: &mut [u8],
) {
    let zero = _mm_setzero_si128();

    // c = upper-left
    // b = upper
    // a = left
    // d = current/raw
    let (mut c, mut b, mut a, mut d) = (zero, zero, zero, zero);

    let (mut f, mut g) = ([0u8; 16], [0u8; 16]);

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(SIZE)
        .zip(raw.chunks_exact(SIZE))
        .zip(current.chunks_exact_mut(SIZE))
    {
        f[..SIZE].copy_from_slice(prev);
        g[..SIZE].copy_from_slice(raw);

        // Slide previous vectors
        c = b;
        a = d;

        // Load next upper/raw bytes as widened i16 lanes
        b = _mm_unpacklo_epi8(
            _mm_loadu_si128(f.as_ptr().cast()),
            zero,
        );

        d = _mm_unpacklo_epi8(
            _mm_loadu_si128(g.as_ptr().cast()),
            zero,
        );

        //
        // stb-style Paeth predictor:
        //
        // thresh = 3*c - (a+b)
        //
        // if hi <= thresh => lo
        // else if thresh <= lo => hi
        // else => c
        //

        let lo = _mm_min_epi16(a, b);
        let hi = _mm_max_epi16(a, b);

        let cc = _mm_add_epi16(c, c);
        let three_c = _mm_add_epi16(cc, c);

        let ab = _mm_add_epi16(a, b);

        let thresh = _mm_sub_epi16(three_c, ab);

        // thresh <= lo
        let choose_hi = _mm_cmpeq_epi16(
            _mm_min_epi16(thresh, lo),
            thresh,
        );

        // hi <= thresh
        let choose_lo = _mm_cmpeq_epi16(
            _mm_min_epi16(hi, thresh),
            hi,
        );

        let nearest = if_then_else(
            choose_lo,
            lo,
            if_then_else(
                choose_hi,
                hi,
                c,
            ),
        );

        // modulo-256 reconstruction add
        d = _mm_add_epi8(d, nearest);

        // pack back to bytes
        _mm_storeu_si128(
            f.as_mut_ptr().cast(),
            _mm_packus_epi16(d, d),
        );

        current_row.copy_from_slice(&f[..SIZE]);
    }
}

pub fn de_filter_paeth_sse41<const SIZE: usize>(prev_row: &[u8], raw: &[u8], current: &mut [u8]) {
    #[cfg(feature = "std")]
    {
        if !is_x86_feature_detected!("sse4.1") {
            panic!("Internal error, calling platform specific function where not supported")
        }
    }
    unsafe {
        de_filter_paeth_sse41_inner::<SIZE>(prev_row, raw, current);
    }
}

#[cfg(target_feature = "sse2")]
#[target_feature(enable = "sse2")]
unsafe fn defilter_avg_sse2_inner<const SIZE: usize>(
    prev_row: &[u8], raw: &[u8], current: &mut [u8]
) {
    /* The Avg filter predicts each pixel as the (truncated) average of a and b.
     * There's no pixel to the left of the first pixel.  Luckily, it's
     * predicted to be half of the pixel above it.  So again, this works
     * perfectly with our loop if we make sure a starts at zero.
     */

    let zero = _mm_setzero_si128();

    let (mut x, mut y) = ([0; 16], [0; 16]);

    let (mut a, mut b);
    let mut d = zero;
    let mut avg;

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(SIZE)
        .zip(raw.chunks_exact(SIZE))
        .zip(current.chunks_exact_mut(SIZE))
    {
        x[0..SIZE].copy_from_slice(raw);
        y[0..SIZE].copy_from_slice(prev);

        //b = load3(prev.try_into().unwrap());
        b = _mm_loadu_si128(y.as_ptr().cast());
        a = d;
        //d = load3(raw.try_into().unwrap());
        d = _mm_loadu_si128(x.as_ptr().cast());
        /* PNG requires a truncating average, so we can't just use _mm_avg_epu8 */
        avg = _mm_avg_epu8(a, b);
        /* ...but we can fix it up by subtracting off 1 if it rounded up. */
        avg = _mm_sub_epi8(avg, _mm_and_si128(_mm_xor_si128(a, b), _mm_set1_epi8(1)));

        d = _mm_add_epi8(d, avg);
        _mm_storeu_si128(x.as_mut_ptr().cast(), d);

        // store3(current,d)
        current_row.copy_from_slice(&x[0..SIZE]);
    }
}

pub fn defilter_avg_sse<const SIZE: usize>(prev_row: &[u8], raw: &[u8], current: &mut [u8]) {
    #[cfg(feature = "std")]
    {
        if !is_x86_feature_detected!("sse2") {
            panic!("Internal error, calling platform specific function where not supported")
        }
    }
    unsafe {
        defilter_avg_sse2_inner::<SIZE>(prev_row, raw, current);
    }
}
