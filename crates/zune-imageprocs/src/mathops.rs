//! Mathematical operations shared amongst functions

/// Implement fast integer division from
/// Daniel's Lemire fastmod.
///  
/// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
/// Software: Practice and Experience  49 (6), 2019.
#[inline(always)]
#[must_use]
pub fn compute_mod_u32(d: u64) -> u64
{
    // operator precedence will be the end of me,,
    return ((0xFFFF_FFFF_FFFF_FFFF_u64) / d) + 1;
}

/// Implement fast integer division from
/// Daniel's Lemire fastmod.
///  
/// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
/// Software: Practice and Experience  49 (6), 2019.
#[inline(always)]
#[must_use]
pub fn mul128_u32(low_bits: u64, d: u32) -> u64
{
    return ((u128::from(low_bits) * u128::from(d)) >> 64) as u64;
}

/// Fast division of u32 numbers
///  
/// Implement fast integer division from
/// See [Faster Remainder by Direct Computation: Applications to Compilers and Software Libraries](https://arxiv.org/abs/1902.01961),
/// Software: Practice and Experience  49 (6), 2019.
#[inline(always)]
#[allow(clippy::cast_possible_truncation)]
#[must_use]
pub fn fastdiv_u32(a: u32, m: u64) -> u32
{
    mul128_u32(m, a) as u32
}
