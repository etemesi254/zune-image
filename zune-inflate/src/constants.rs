/// Number of symbols in each Huffman code.  Note: for the literal/length
/// and offset codes, these are actually the maximum values; a given block
/// might use fewer symbols.
pub const DEFLATE_NUM_PRECODE_SYMS: usize = 19;
pub const DEFLATE_NUM_LITLEN_SYMS: usize = 288;
pub const DEFLATE_NUM_OFFSET_SYMS: usize = 32;

/// Maximum possible overrun when decoding codeword lengths
pub const DELFATE_MAX_LENS_OVERRUN: usize = 137;

/// Order which precode lengths are stored
pub static DEFLATE_PRECODE_LENS_PERMUTATION: [u8; DEFLATE_NUM_PRECODE_SYMS] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15
];

pub const PRECODE_ENOUGH: usize = 128;

/// Maximum codeword length across all codes.
pub const DEFLATE_MAX_CODEWORD_LENGTH: usize = 15;

pub const DEFLATE_MAX_OFFSET_CODEWORD_LENGTH: usize = 15;
pub const DEFLATE_MAX_LITLEN_CODEWORD_LENGTH: usize = 15;

pub const PRECODE_TABLE_BITS: usize = 7;

pub const LITLEN_TABLE_BITS: usize = 11;
pub const LITLEN_ENOUGH: usize = 2342;
/// Maximum bits found in the lookup table for offsets
/// offsets larger than this require a lookup into a sub-table
pub const OFFSET_TABLEBITS: usize = 8;
pub const OFFSET_ENOUGH: usize = 402;
/// Maximum number of symbols across all codes
pub const DEFLATE_MAX_NUM_SYMS: usize = 288;

///Maximum codeword length in bits for each precode
pub const DEFLATE_MAX_PRE_CODEWORD_LEN: u8 = 7;

/// Format for precode decode table entries, Bits not explicitly contain zeroes
///
/// 20-16: presym
/// 10-8 Codeword length(not used)
/// Bit 2-0 Codeword length
///
/// It never has sub-tables since we use PRECODE_TABLEBITS == MAX_PRECODEWORD_LENGTH
///
/// PRECODE_DECODE_RESULTS contains static parts of the entry for each symbol,
/// make_decode_table_entry produces the final results
pub static PRECODE_DECODE_RESULTS: [u32; 19] = make_precode_static_table();

const fn make_precode_static_table() -> [u32; 19]
{
    let mut table: [u32; 19] = [0; 19];
    let mut i = 0;

    while i < 19
    {
        table[i] = (i as u32) << 16;
        i += 1;
    }

    table
}

/// Presence of a literal entry
pub const HUFFDEC_LITERAL: u32 = 0x80000000;
/// Presence of HUFFDEC_SUITABLE_POINTER or HUFFDEC_END_OF_BLOCK
pub const HUFFDEC_EXCEPTIONAL: u32 = 0x00008000;
/// Pointer entry in the litlen or offset decode table
pub const HUFFDEC_SUITABLE_POINTER: u32 = 0x00004000;
/// End of block entry in litlen decode table
pub const HUFFDEC_END_OF_BLOCK: u32 = 0x00002000;

#[rustfmt::skip]
#[allow(clippy::zero_prefixed_literal)]
const fn construct_litlen_decode_table() -> [u32; 288]
{
    let mut results: [u32; 288] = [0; 288];
    let mut i = 0;

    while i < 256
    {
        results[i] = ((i as u32) << 16) | HUFFDEC_LITERAL;
        i += 1;
    }

    results[i] = HUFFDEC_EXCEPTIONAL | HUFFDEC_LITERAL;
    i += 1;


    let base_and_bits_tables = [
        (003, 0), (004, 0), (005, 0), (006, 0),
        (007, 0), (008, 0), (009, 0), (010, 0),
        (011, 1), (013, 1), (015, 1), (017, 1),
        (019, 2), (023, 2), (027, 2), (031, 2),
        (035, 3), (043, 3), (051, 3), (059, 3),
        (067, 4), (083, 4), (099, 4), (115, 4),
        (131, 5), (163, 5), (195, 5), (227, 5),
        (258, 0), (258, 0), (258, 0),
    ];
    let mut j = 0;

    while i < 288
    {
        let (length_base, extra_bits) = base_and_bits_tables[j];
        results[i] = (length_base << 16) | extra_bits;

        i += 1;
        j += 1;
    }

    results
}

const fn entry(base: u32, extra: u32) -> u32
{
    base << 16 | extra
}

#[rustfmt::skip]
pub static OFFSET_DECODE_RESULTS: [u32; 32] = [
    entry(1, 0), entry(2, 0), entry(3, 0), entry(4, 0),
    entry(5, 1), entry(7, 1), entry(9, 2), entry(13, 2),
    entry(17, 3), entry(25, 3), entry(33, 4), entry(49, 4),
    entry(65, 5), entry(97, 5), entry(129, 6), entry(193, 6),
    entry(257, 7), entry(385, 7), entry(513, 8), entry(769, 8),
    entry(1025, 9), entry(1537, 9), entry(2049, 10), entry(3073, 10),
    entry(4097, 11), entry(6145, 11), entry(8193, 12), entry(12289, 12),
    entry(16385, 13), entry(24577, 13), entry(24577, 13), entry(24577, 13),
];

pub static LITLEN_DECODE_RESULTS: [u32; 288] = construct_litlen_decode_table();

pub const DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN: u64 = 2;

pub const DEFLATE_BLOCKTYPE_UNCOMPRESSED: u64 = 0;

pub const DEFLATE_BLOCKTYPE_STATIC: u64 = 1;
