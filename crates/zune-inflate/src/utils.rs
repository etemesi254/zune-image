/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use core::cell::Cell;

/// make_decode_table_entry() creates a decode table entry for the given symbol
/// by combining the static part 'decode_results[sym]' with the dynamic part
/// 'len', which is the remaining codeword length (the codeword length for main
/// table entries, or the codeword length minus TABLEBITS for subtable entries).
///
/// In all cases, we add 'len' to each of the two low-order bytes to create the
/// appropriately-formatted decode table entry.  See the definitions of the
/// *_decode_results[] arrays below, where the entry format is described.
pub(crate) fn make_decode_table_entry(decode_results: &[u32], sym: usize, len: u32) -> u32 {
    decode_results[sym] + (len << 8) + len
}

/// A safe version of src.copy_within that helps me because I tend to always
/// confuse the arguments
pub fn fixed_copy_within<const SIZE: usize>(
    dest: &mut [u8], src_offset: usize, dest_offset: usize
) {
    // for debug builds ensure we don't go out of bounds
    debug_assert!(
        dest_offset + SIZE <= dest.len(),
        "[dst]: End position {} out of range for slice of length {}",
        dest_offset + SIZE,
        dest.len()
    );

    dest.copy_within(src_offset..src_offset + SIZE, dest_offset);
}

#[inline(always)]
pub fn copy_rep_matches(dest: &mut [u8], src_offset: usize, dest_offset: usize, length: usize) {
    // Overlapping/repeating match copy. The source pattern starts at src_offset
    // and has a period of `diff` bytes. We expand it forward by copying one byte
    // at a time through a sliding window, so each written byte is immediately
    // available as a source for later bytes in the same match.
    //
    // Invariant: window[0] is always a valid already-written source byte,
    // and window.last() is always the next destination byte to fill.
    //
    // Slice bounds: we need `length` windows of size `diff`.
    // The last window occupies indices [length-1 .. length-1+diff],
    // i.e. the slice must end at src_offset + (length - 1) + diff
    //                             = src_offset + length - 1 + (dest_offset - src_offset + 1)
    //                             = dest_offset + length.
    // So the slice is dest[src_offset .. dest_offset + length + 1].
    //                                                           ^^^
    //                                     +1 because ..end is exclusive

    let diff = dest_offset - src_offset + 1;

    for window in Cell::from_mut(&mut dest[src_offset..dest_offset + length + 1])
        .as_slice_of_cells()
        .windows(diff)
    {
        window.last().unwrap().set(window[0].get());
    }
}
#[inline(always)]
pub fn copy_rep_matches_slow(dest: &mut [u8], src_offset: usize, dest_offset: usize, length: usize) {
    // Overlapping copy: period is `dest_offset - src_offset`, so we can't
    // bulk-copy. Write one byte at a time; each write is immediately
    // readable as source for later bytes in the same match.
    let period = dest_offset - src_offset;
    for i in 0..length {
        dest[dest_offset + i] = dest[src_offset + i % period];
    }
}

/// Return the minimum of two usizes in a const context
#[rustfmt::skip]
pub const fn const_min_usize(a: usize, b: usize) -> usize
{
    if a < b { a } else { b }
}

/// Calculate the adler hash of a piece of data.
#[inline(never)]
#[cfg(feature = "zlib")]
pub fn calc_adler_hash(data: &[u8]) -> u32 {
    use simd_adler32::Adler32;
    let mut hasher = Adler32::new();

    hasher.write(data);

    hasher.finish()
}
