use std::arch::x86_64::{__m128i, _mm_clmulepi64_si128, _mm_xor_si128};

unsafe fn _fold_sse(src: __m128i, dst: __m128i, multipliers: __m128i) -> __m128i
{
    /*
     * The immediate constant for PCLMULQDQ specifies which 64-bit halves of
     * the 128-bit vectors to multiply:
     */

    //0x00 means low halves (higher degree polynomial terms for us)
    let first = _mm_clmulepi64_si128::<0x00>(src, multipliers);
    // 0x11 means high halves (lower degree polynomial terms for us)
    let second = _mm_clmulepi64_si128::<0x11>(src, multipliers);

    _mm_xor_si128(_mm_xor_si128(dst, first), second)
}
