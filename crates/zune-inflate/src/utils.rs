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

/// Copy SIZE amount of bytes from src, starting from `src_offset` into dest starting from
/// `dest_offset`.
///
/// # Unsafety
///
/// This function might read and write out  of bounds memory, if SAFE is true, it checks
/// both in debug and release builds that the reads and rights are in bounds at the cost of performance.
pub fn const_copy<const SIZE: usize, const SAFE: bool>(
    src: &[u8], dest: &mut [u8], src_offset: usize, dest_offset: usize
)
{
    // ensure we don't go out of bounds(only if SAFE is true)
    if SAFE
    {
        assert!(
            src_offset + SIZE - 1 <= src.len(),
            "[src]: End position {} out of range for slice of length {}",
            src_offset + SIZE,
            src.len()
        );
        assert!(
            dest_offset + SIZE <= dest.len(),
            "[dst]: End position {} out of range for slice of length {}",
            dest_offset + SIZE,
            dest.len()
        );
    }

    unsafe {
        dest.as_mut_ptr()
            .add(dest_offset)
            .copy_from(src.as_ptr().add(src_offset), SIZE);
        // do not generate calls to memcpy optimizer
        // I'm doing some exclusive shit
        // (If it's a loop, the optimizer may lift this to be a memcpy)
        #[cfg(not(any(target_arch = "asmjs", target_arch = "wasm32")))]
        {
            use std::arch::asm;
            asm!("");
        }
    }
}
#[inline(always)]
pub fn copy_rep_matches<const SAFE: bool>(
    dest: &mut [u8], offset: usize, dest_offset: usize, length: usize
)
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

    let mut counter = 0;

    let mut match_length = length;

    let (mut dest_src, mut dest_ptr) =
        unsafe { split_at_mut_unchecked::<SAFE, u8>(dest, dest_offset) };

    let mut dest_offset = dest_offset;
    // While this can be improved, it rarely occurs and the improvement is a bit ugly code wise
    // This simple byte wise implementation does what we need and fast enough
    while match_length > 0
    {
        // PS, this should remain. Removing it is a performance
        // fall of 300 Mb/s decode speeds. :)
        const_copy::<1, false>(dest_src, dest_ptr, offset + counter, 0);

        dest_offset += 1;
        // use unsafe here because we know that src and dest
        // are in bounds
        (dest_src, dest_ptr) = unsafe { split_at_mut_unchecked::<false, u8>(dest, dest_offset) };

        counter += 1;

        match_length -= 1;
    }
}

pub unsafe fn split_at_mut_unchecked<const SAFE: bool, T>(
    slice: &mut [T], mid: usize
) -> (&[T], &mut [T])
{
    let len = slice.len();
    let ptr = slice.as_mut_ptr();
    if SAFE
    {
        assert!(mid <= len, "{},{}", mid, len);
    }
    (
        std::slice::from_raw_parts(ptr, mid),
        std::slice::from_raw_parts_mut(ptr.add(mid), len - mid)
    )
}

/// Return the minimum of two usizes in a const context
pub const fn const_min_usize(a: usize, b: usize) -> usize
{
    if a < b
    {
        a
    }
    else
    {
        b
    }
}
