/// Number of symbols in each Huffman code.  Note: for the literal/length
/// and offset codes, these are actually the maximum values; a given block
/// might use fewer symbols.
pub const DEFLATE_NUM_PRECODE_SYMS: usize = 19;

/// Order which precode lengths are stored
pub static DEFLATE_PRECODE_LENS_PERMUTATION: [u8; DEFLATE_NUM_PRECODE_SYMS] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

pub const PRECODE_ENOUGH: usize = 128;

/// Maximum codeword length across all codes.
pub const DEFLATE_MAX_CODEWORD_LENGTH: usize = 15;
