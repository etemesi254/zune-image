//! Sse capable defilter routines.
//!
//! These techniques enable faster png de-filtering in
//! situations where sse is available i.e on x86 arch
//!
//! They are derived from the amazing spng at https://github.com/randy408/libspng
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

#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#![cfg(feature = "sse")]

#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[target_feature(enable = "sse2")]
#[inline]
unsafe fn store3(x: &mut [u8; 3], v: __m128i)
{
    let tmp = _mm_cvtsi128_si32(v) as u32;
    let tmp_x = tmp.to_le_bytes();
    x[0..3].copy_from_slice(&tmp_x[0..3]);
}

unsafe fn store4(x: &mut [u8; 4], v: __m128i)
{
    let tmp = _mm_cvtsi128_si32(v);
    x.copy_from_slice(&tmp.to_le_bytes());
}

unsafe fn load3(x: &[u8; 3]) -> __m128i
{
    let mut tmp_bytes = [0_u8; 4];
    tmp_bytes[0..3].copy_from_slice(x);

    let tmp = i32::from_le_bytes(tmp_bytes);
    _mm_cvtsi32_si128(tmp)
}

unsafe fn load4(x: &[u8; 4]) -> __m128i
{
    let tmp = i32::from_le_bytes(*x);
    _mm_cvtsi32_si128(tmp)
}

unsafe fn load8(x: &[u8; 8]) -> __m128i
{
    let tmp = i64::from_le_bytes(*x);
    _mm_cvtsi64_si128(tmp)
}

unsafe fn store8(x: &mut [u8; 8], v: __m128i)
{
    let tmp = _mm_cvtsi128_si64x(v);
    x.copy_from_slice(&tmp.to_le_bytes());
}

#[allow(unused_assignments)]
#[target_feature(enable = "sse2")]
unsafe fn defilter_sub3_sse2_inner(raw: &[u8], current: &mut [u8])
{
    let (mut a, mut d) = (_mm_setzero_si128(), _mm_setzero_si128());

    for (raw, out) in raw.chunks_exact(3).zip(current.chunks_exact_mut(3))
    {
        a = d;
        d = load3(raw.try_into().unwrap());
        d = _mm_add_epi8(d, a);
        store3(out.try_into().unwrap(), d);
    }
}

pub fn de_filter_sub3_sse2(raw: &[u8], current: &mut [u8])
{
    unsafe { defilter_sub3_sse2_inner(raw, current) }
}

#[allow(unused_assignments)]
#[target_feature(enable = "sse2")]
unsafe fn de_filter_sub4_sse2_inner(raw: &[u8], current: &mut [u8])
{
    let (mut a, mut d) = (_mm_setzero_si128(), _mm_setzero_si128());

    for (raw, out) in raw.chunks_exact(4).zip(current.chunks_exact_mut(4))
    {
        a = d;
        d = load4(raw.try_into().unwrap());
        d = _mm_add_epi8(d, a);
        store4(out.try_into().unwrap(), d);
    }
}

pub fn de_filter_sub4_sse2(raw: &[u8], current: &mut [u8])
{
    unsafe { de_filter_sub4_sse2_inner(raw, current) }
}

#[inline]
#[target_feature(enable = "sse4.1")]
unsafe fn if_then_else(c: __m128i, t: __m128i, e: __m128i) -> __m128i
{
    _mm_blendv_epi8(e, t, c)

    // SSE 2
    //return _mm_or_si128(_mm_and_si128(c, t), _mm_andnot_si128(c, e));
}

#[allow(unused_assignments)]
#[target_feature(enable = "sse4.1")]
unsafe fn de_filter_paeth3_sse41_inner(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    /* Paeth tries to predict pixel d using the pixel to the left of it, a,
     * and two pixels from the previous row, b and c:
     *   prev: c b
     *   row:  a d
     * The Paeth function predicts d to be whichever of a, b, or c is nearest to
     * p=a+b-c.
     *
     * The first pixel has no left context, and so uses an Up filter, p = b.
     * This works naturally with our main loop's p = a+b-c if we force a and c
     * to zero.
     * Here we zero b and d, which become c and a respectively at the start of
     * the loop.
     */

    let zero = _mm_setzero_si128();

    let (mut c, mut b, mut a, mut d) = (zero, zero, zero, zero);

    let (mut pa, mut pb, mut pc, mut smallest, mut nearest);

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(3)
        .zip(raw.chunks_exact(3))
        .zip(current.chunks_exact_mut(3))
    {
        /*
         * It's easiest to do this math (particularly, deal with pc) with 16-bit
         * intermediates.
         */
        c = b;
        b = _mm_unpacklo_epi8(load3(prev.try_into().unwrap()), zero);
        a = d;
        d = _mm_unpacklo_epi8(load3(raw.try_into().unwrap()), zero);

        /* (p-a) == (a+b-c - a) == (b-c) */
        pa = _mm_sub_epi16(b, c);

        /* (p-b) == (a+b-c - b) == (a-c) */
        pb = _mm_sub_epi16(a, c);

        /* (p-c) == (a+b-c - c) == (a+b-c-c) == (b-c)+(a-c) */
        pc = _mm_add_epi16(pa, pb);

        pa = _mm_abs_epi16(pa); /* |p-a| */
        pb = _mm_abs_epi16(pb); /* |p-b| */
        pc = _mm_abs_epi16(pc); /* |p-c| */

        smallest = _mm_min_epi16(pc, _mm_min_epi16(pa, pb));

        /* Paeth breaks ties favoring a over b over c. */
        nearest = if_then_else(
            _mm_cmpeq_epi16(smallest, pa),
            a,
            if_then_else(_mm_cmpeq_epi16(smallest, pb), b, c)
        );

        /* Note `_epi8`: we need addition to wrap modulo 255. */
        d = _mm_add_epi8(d, nearest);

        store3(current_row.try_into().unwrap(), _mm_packus_epi16(d, d));
    }
}

#[target_feature(enable = "sse4.1")]
#[allow(unused_assignments)]
unsafe fn de_filter_paeth4_sse41_inner(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    let zero = _mm_setzero_si128();

    let (mut c, mut b, mut a, mut d) = (zero, zero, zero, zero);

    let (mut pa, mut pb, mut pc, mut smallest, mut nearest);

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(4)
        .zip(raw.chunks_exact(4))
        .zip(current.chunks_exact_mut(4))
    {
        c = b;
        b = _mm_unpacklo_epi8(load4(prev.try_into().unwrap()), zero);
        a = d;
        d = _mm_unpacklo_epi8(load4(raw.try_into().unwrap()), zero);

        /* (p-a) == (a+b-c - a) == (b-c) */
        pa = _mm_sub_epi16(b, c);

        /* (p-b) == (a+b-c - b) == (a-c) */
        pb = _mm_sub_epi16(a, c);

        /* (p-c) == (a+b-c - c) == (a+b-c-c) == (b-c)+(a-c) */
        pc = _mm_add_epi16(pa, pb);

        pa = _mm_abs_epi16(pa); /* |p-a| */
        pb = _mm_abs_epi16(pb); /* |p-b| */
        pc = _mm_abs_epi16(pc); /* |p-c| */

        smallest = _mm_min_epi16(pc, _mm_min_epi16(pa, pb));

        /* Paeth breaks ties favoring a over b over c. */
        nearest = if_then_else(
            _mm_cmpeq_epi16(smallest, pa),
            a,
            if_then_else(_mm_cmpeq_epi16(smallest, pb), b, c)
        );

        /* Note `_epi8`: we need addition to wrap modulo 255. */
        d = _mm_add_epi8(d, nearest);

        store4(current_row.try_into().unwrap(), _mm_packus_epi16(d, d));
    }
}

// 16 bpp RGBA SSE filtering code
#[target_feature(enable = "sse4.1")]
#[allow(unused_assignments)]
unsafe fn de_filter_paeth8_sse41_inner(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    let zero = _mm_setzero_si128();

    let (mut c, mut b, mut a, mut d) = (zero, zero, zero, zero);

    let (mut pa, mut pb, mut pc, mut smallest, mut nearest);

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(8)
        .zip(raw.chunks_exact(8))
        .zip(current.chunks_exact_mut(8))
    {
        c = b;
        b = _mm_unpacklo_epi8(load8(prev.try_into().unwrap()), zero);
        a = d;
        d = _mm_unpacklo_epi8(load8(raw.try_into().unwrap()), zero);

        /* (p-a) == (a+b-c - a) == (b-c) */
        pa = _mm_sub_epi16(b, c);

        /* (p-b) == (a+b-c - b) == (a-c) */
        pb = _mm_sub_epi16(a, c);

        /* (p-c) == (a+b-c - c) == (a+b-c-c) == (b-c)+(a-c) */
        pc = _mm_add_epi16(pa, pb);

        pa = _mm_abs_epi16(pa); /* |p-a| */
        pb = _mm_abs_epi16(pb); /* |p-b| */
        pc = _mm_abs_epi16(pc); /* |p-c| */

        smallest = _mm_min_epi16(pc, _mm_min_epi16(pa, pb));

        /* Paeth breaks ties favoring a over b over c. */
        nearest = if_then_else(
            _mm_cmpeq_epi16(smallest, pa),
            a,
            if_then_else(_mm_cmpeq_epi16(smallest, pb), b, c)
        );

        /* Note `_epi8`: we need addition to wrap modulo 255. */
        d = _mm_add_epi8(d, nearest);

        store8(current_row.try_into().unwrap(), _mm_packus_epi16(d, d));
    }
}

/// Carries out de-filtering of a paeth filtered scanline using SSE
///
/// # Panics
/// If sse4.1 feature isn't present
pub fn de_filter_paeth3_sse41(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    unsafe {
        if !is_x86_feature_detected!("sse4.1")
        {
            panic!("SSE feature not found, this is unsound, please file an issue")
        }

        de_filter_paeth3_sse41_inner(prev_row, raw, current);
    }
}

pub fn de_filter_paeth4_sse41(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    unsafe {
        if !is_x86_feature_detected!("sse4.1")
        {
            panic!("SSE feature not found, this is unsound,please file an issue")
        }

        de_filter_paeth4_sse41_inner(prev_row, raw, current);
    }
}

pub fn de_filter_paeth8_sse41(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    unsafe {
        if !is_x86_feature_detected!("sse4.1")
        {
            panic!("SSE feature not found, this is unsound,please file an issue")
        }
        de_filter_paeth8_sse41_inner(prev_row, raw, current);
    }
}

#[target_feature(enable = "sse2")]
unsafe fn defilter_avg4_sse2_inner(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    /* The Avg filter predicts each pixel as the (truncated) average of a and b.
     * There's no pixel to the left of the first pixel.  Luckily, it's
     * predicted to be half of the pixel above it.  So again, this works
     * perfectly with our loop if we make sure a starts at zero.
     */

    let zero = _mm_setzero_si128();
    let (mut a, mut b);
    let mut d = zero;
    let mut avg;

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(4)
        .zip(raw.chunks_exact(4))
        .zip(current.chunks_exact_mut(4))
    {
        b = load4(prev.try_into().unwrap());
        a = d;
        d = load4(raw.try_into().unwrap());

        /* PNG requires a truncating average, so we can't just use _mm_avg_epu8 */
        avg = _mm_avg_epu8(a, b);
        /* ...but we can fix it up by subtracting off 1 if it rounded up. */
        avg = _mm_sub_epi8(avg, _mm_and_si128(_mm_xor_si128(a, b), _mm_set1_epi8(1)));

        d = _mm_add_epi8(d, avg);
        store4(current_row.try_into().unwrap(), d);
    }
}

#[cfg(target_feature = "sse2")]
unsafe fn defilter_avg3_sse2_inner(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    /* The Avg filter predicts each pixel as the (truncated) average of a and b.
     * There's no pixel to the left of the first pixel.  Luckily, it's
     * predicted to be half of the pixel above it.  So again, this works
     * perfectly with our loop if we make sure a starts at zero.
     */

    let zero = _mm_setzero_si128();
    let (mut a, mut b);
    let mut d = zero;
    let mut avg;

    for ((prev, raw), current_row) in prev_row
        .chunks_exact(3)
        .zip(raw.chunks_exact(3))
        .zip(current.chunks_exact_mut(3))
    {
        b = load3(prev.try_into().unwrap());
        a = d;
        d = load3(raw.try_into().unwrap());

        /* PNG requires a truncating average, so we can't just use _mm_avg_epu8 */
        avg = _mm_avg_epu8(a, b);
        /* ...but we can fix it up by subtracting off 1 if it rounded up. */
        avg = _mm_sub_epi8(avg, _mm_and_si128(_mm_xor_si128(a, b), _mm_set1_epi8(1)));

        d = _mm_add_epi8(d, avg);
        store3(current_row.try_into().unwrap(), d);
    }
}

pub fn defilter_avg3_sse(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    unsafe {
        if !is_x86_feature_detected!("sse2")
        {
            panic!("SSE feature not found, this is unsound,please file an issue")
        }

        defilter_avg3_sse2_inner(prev_row, raw, current);
    }
}

pub fn defilter_avg4_sse(prev_row: &[u8], raw: &[u8], current: &mut [u8])
{
    unsafe {
        if !is_x86_feature_detected!("sse2")
        {
            panic!("SSE feature not found, this is unsound,please file an issue")
        }

        defilter_avg4_sse2_inner(prev_row, raw, current);
    }
}
