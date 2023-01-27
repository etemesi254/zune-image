/// make_decode_table_entry() creates a decode table entry for the given symbol
/// by combining the static part 'decode_results[sym]' with the dynamic part
/// 'len', which is the remaining codeword length (the codeword length for main
/// table entries, or the codeword length minus TABLEBITS for subtable entries).
///
/// In all cases, we add 'len' to each of the two low-order bytes to create the
/// appropriately-formatted decode table entry.  See the definitions of the
/// *_decode_results[] arrays below, where the entry format is described.
pub(crate) fn make_decode_table_entry(decode_results: &[u32], sym: usize, len: u32) -> u32
{
    decode_results[sym] + (len << 8) + len
}

/// A safe version of src.copy_within that helps me because I tend to always
/// confuse the arguments
pub fn fixed_copy_within<const SIZE: usize>(dest: &mut [u8], src_offset: usize, dest_offset: usize)
{
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
pub fn copy_rep_matches(dest: &mut [u8], offset: usize, dest_offset: usize, length: usize)
{
    // REP MATCHES (LITERAL + REP MATCH).
    //
    // As in most LZ77-based compressors, the length can be larger than the offset,
    // yielding a form of run-length encoding (RLE). For instance,
    // "xababab" could be encoded as
    //
    //   <literal: "xab"> <copy: offset=2 length=4>
    //
    // To decode this, we want to always bump up dest_src by 1 on every 1 byte copy.
    //
    //
    // Iteration 1.
    // ┌───────────────────────────┐ ┌────────┐
    // │dest_src(len 3)            │ │dest_ptr│
    // └───────────────────────────┘ └────────┘
    //                               ┌───────────────┐
    //                               │ copy one byte │
    //                               └───────────────┘
    // Iteration 2.
    // ┌─────────────────────────────────────────────┐ ┌────────┐
    // │dest_src    (len 4)                          │ │dest_ptr│
    // └─────────────────────────────────────────────┘ └────────┘
    //                                                  ┌───────────────┐
    //                                                  │ copy one byte │
    //                                                  └───────────────┘
    // etc. etc.
    //
    // I.e after every byte copy of a match bump dest_src so that a future copy operation
    // will contain the byte.
    //
    // E.g for the example above
    //
    //  dest_src   │
    //─────────────│───────────────
    // [x{a}b]     │  [copy at = 1]
    // [xa{b}a]    │  [copy at = 2]
    // [xab{a}b]   │  [copy at = 3]
    // [xaba{b}a]  │  [copy at = 4]

    for i in 0..length
    {
        // Safety: We assert the worst possible length is in place
        // before the loop for both src and dest.
        //
        // Reason: This is perf sensitive, we can't afford such slowdowns
        // and it activates optimizations i.e see
        let byte = dest[offset + i];
        dest[dest_offset + i] = byte;
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
pub fn calc_adler_hash(data: &[u8]) -> u32
{
    use simd_adler32::Adler32;
    let mut hasher = Adler32::new();

    hasher.write(data);

    hasher.finish()
}
