/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! A simple jpeg xl encoder
use alloc::vec;
use alloc::vec::Vec;
use core::cmp::{max, min};
use core::marker::PhantomData;

use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::{ZByteWriterTrait, ZWriter};
use zune_core::log::{log_enabled, trace, Level};
use zune_core::options::EncoderOptions;

use crate::bit_depth::{JxlBitEncoder, MoreThan14Bits, UpTo8Bits};
use crate::bit_writer::{
    encode_hybrid_uint_lz77, encode_hybrid_unit_000, BitWriter, BorrowingBitWriter
};
use crate::color_convert::{
    fill_row_g16, fill_row_g8, fill_row_ga16, fill_row_ga8, fill_row_rgb16, fill_row_rgb8,
    fill_row_rgba16, fill_row_rgba8
};
use crate::errors::SUPPORTED_COLORSPACES;
use crate::JxlEncodeErrors;

const K_NUM_RAW_SYMBOLS: usize = 19;
const K_NUM_LZ77: usize = 33;
const K_LZ77_MIN_LENGTH: usize = 7;
const K_LOG_CHUNK_SIZE: usize = 3;
const K_LZ77CACHE_SIZE: usize = 32;
const K_CHUNK_SIZE: usize = 1 << K_LOG_CHUNK_SIZE;
const K_MAX_NUM_SYMBOLS: usize = if K_NUM_RAW_SYMBOLS + 1 < K_NUM_LZ77 {
    K_NUM_LZ77
} else {
    K_NUM_RAW_SYMBOLS + 1
};

pub(crate) struct FrameState {
    option:              EncoderOptions,
    header:              BitWriter,
    group_data:          Vec<[BitWriter; 4]>,
    current_bit_writer:  usize,
    bit_writer_byte_pos: usize
}

/// A simple jxl encoder
///
///  # Encoding 16 bit data
/// - To encode a 16-bit image, each element needs to be re-interpreted as 2 `u8`'s in native endian
/// the library will do the appropriate clamping
///
/// # Multithreading support
/// Via the `thread` feature, the library can use multiple threads to speed up compression, one can
/// configure how many threads to open or whether the library should use threads at compile time by
/// enabling or disabling the `thread` feature and at runtime via `EncodeOptions`
///
/// ## Example option two threads
/// ```
/// use zune_core::bit_depth::BitDepth;
/// use zune_core::colorspace::ColorSpace;
/// use zune_core::options::EncoderOptions;
/// // set threads to be 2 for encoding
/// let options = EncoderOptions::new(100,100,ColorSpace::RGB,BitDepth::Eight).set_num_threads(2);
/// ```
///
/// Setting `set_num_threads` to `1` forces single threaded execution
///
///  # Example
/// - Encode grayscale image
///
/// ```
/// use zune_core::colorspace::ColorSpace;
/// use zune_core::options::EncoderOptions;
/// use zune_jpegxl::JxlSimpleEncoder;
/// use zune_jpegxl::JxlEncodeErrors;
///
/// fn main()->Result<(),JxlEncodeErrors>{
///     // set up options for encoder
///     let options = EncoderOptions::default()
///         .set_height(10)
///         .set_width(10)
///         .set_colorspace(ColorSpace::Luma);
///     let image:[u8;100] = std::array::from_fn(|x| x as u8);
///
///     let encoder = JxlSimpleEncoder::new(&image,options);
///     let mut output =vec![];
///     // encode the image
///     encoder.encode(&mut output)?;
///     
///     Ok(())
/// }
/// ```
pub struct JxlSimpleEncoder<'a> {
    data:    &'a [u8],
    options: EncoderOptions
}

pub(crate) struct ChunkSampleCollector<'a, T: JxlBitEncoder> {
    raw_counts:  &'a mut [u64; K_NUM_RAW_SYMBOLS],
    lz77_counts: &'a mut [u64; K_NUM_LZ77],
    enc:         PhantomData<T>
}

impl<'a, T: JxlBitEncoder> ChunkSampleCollector<'a, T> {
    pub fn new(
        raw_counts: &'a mut [u64; K_NUM_RAW_SYMBOLS], lz77_counts: &'a mut [u64; K_NUM_LZ77]
    ) -> ChunkSampleCollector<'a, T> {
        ChunkSampleCollector {
            raw_counts,
            lz77_counts,
            enc: PhantomData::<T>
        }
    }
    pub fn encode_rle(&mut self, mut count: usize) {
        if count == 0 {
            return;
        }
        self.raw_counts[0] += 1;
        count -= K_LZ77_MIN_LENGTH + 1;
        let (mut token, mut nbits, mut bits) = (0, 0, 0);

        encode_hybrid_uint_lz77(count as u32, &mut token, &mut nbits, &mut bits);
        self.lz77_counts[token as usize] += 1;
    }

    #[allow(clippy::needless_range_loop)]
    pub fn chunk(&mut self, run: usize, residuals: &[T::Upixel], skip: usize, n: usize) {
        // Run is broken. Encode the run and encode the individual vector.

        self.encode_rle(run);
        let (mut token, mut nbits, mut bits) = (0, 0, 0);

        for ix in skip..n {
            let c = residuals[ix];

            encode_hybrid_unit_000(c.into(), &mut token, &mut nbits, &mut bits);
            // todo: Use a mask here when done
            self.raw_counts[token as usize] += 1;
        }
    }
}

fn packed_signed(value: i32) -> u32 {
    ((value as u32) << 1) ^ ((((!value) as u32) >> 31).wrapping_sub(1))
}

pub(crate) struct PrefixCode {
    pub raw_nbits:        [u8; K_NUM_RAW_SYMBOLS],
    pub raw_bits:         [u8; K_NUM_RAW_SYMBOLS],
    pub lz77_nbits:       [u8; K_NUM_LZ77],
    pub lz77_bits:        [u16; K_NUM_LZ77],
    pub lz77_cache_bits:  [u64; K_LZ77CACHE_SIZE],
    pub lz77_cache_nbits: [u8; K_LZ77CACHE_SIZE]
}

impl PrefixCode {
    #[allow(clippy::needless_range_loop)]
    pub fn new<T: JxlBitEncoder>(
        raw_counts: &[u64; K_NUM_RAW_SYMBOLS], lz77_counts: &[u64; K_NUM_LZ77]
    ) -> PrefixCode {
        let mut raw_nbits = [0; K_NUM_RAW_SYMBOLS];
        let mut raw_bits = [0; K_NUM_RAW_SYMBOLS];
        let mut lz77_nbits = [0; K_NUM_LZ77];
        let mut lz77_bits = [0_u16; K_NUM_LZ77];
        let mut lz77_cache_bits = [0_u64; K_LZ77CACHE_SIZE];
        let mut lz77_cache_nbits = [0; K_LZ77CACHE_SIZE];

        let mut level1_counts = [0; { K_NUM_RAW_SYMBOLS + 1 }];

        level1_counts[0..K_NUM_RAW_SYMBOLS].copy_from_slice(raw_counts);

        let mut num_raw = K_NUM_RAW_SYMBOLS;

        while num_raw > 0 && level1_counts[num_raw - 1] == 0 {
            num_raw -= 1;
        }

        level1_counts[num_raw] = 0;

        for i in 0..K_NUM_LZ77 {
            level1_counts[num_raw] += lz77_counts[i];
        }

        let mut level1_nbits = [0; K_NUM_RAW_SYMBOLS + 1];

        compute_code_lengths(
            &level1_counts,
            num_raw + 1,
            &T::KMIN_RAW_LENGTH,
            &T::KMAX_RAW_LENGTH,
            &mut level1_nbits
        );
        let mut level2_nbits = [0; K_NUM_LZ77];
        let min_lengths = [0; K_NUM_LZ77];
        let l = 15 - level1_nbits[num_raw];
        let max_lengths = [l; K_NUM_LZ77];

        let mut num_lz77 = K_NUM_LZ77;

        while num_lz77 > 0 && lz77_counts[num_lz77 - 1] == 0 {
            num_lz77 -= 1;
        }
        compute_code_lengths(
            lz77_counts,
            num_lz77,
            &min_lengths,
            &max_lengths,
            &mut level2_nbits
        );

        raw_nbits[..num_raw].copy_from_slice(&level1_nbits[..num_raw]);

        for i in 0..num_lz77 {
            lz77_nbits[i] =
                if level2_nbits[i] != 0 { level1_nbits[num_raw] + level2_nbits[i] } else { 0 };
        }

        compute_canonical_code(
            &raw_nbits[..num_raw],
            &mut raw_bits[..num_raw],
            &lz77_nbits,
            &mut lz77_bits
        );

        // prepare lz77 cache
        let (mut token, mut nbits, mut bits) = (0, 0, 0);

        for i in 0..K_LZ77CACHE_SIZE {
            encode_hybrid_uint_lz77(i as u32, &mut token, &mut nbits, &mut bits);
            let token_x = token as usize;
            lz77_cache_nbits[i] = lz77_nbits[token_x] + (nbits as u8) + raw_nbits[0];

            let bits = bits as u64;
            lz77_cache_bits[i] = (((bits << lz77_nbits[token_x]) | u64::from(lz77_bits[token_x]))
                << raw_nbits[0])
                | u64::from(raw_bits[0]);
        }

        PrefixCode {
            raw_nbits,
            raw_bits,
            lz77_nbits,
            lz77_bits,
            lz77_cache_bits,
            lz77_cache_nbits
        }
    }
    fn write_to(&self, writer: &mut BitWriter) {
        let mut code_length_counts: [u64; 18] = [0; 18];
        code_length_counts[17] = 3 + 2 * (K_NUM_LZ77 - 1) as u64;

        for i in 0..K_NUM_RAW_SYMBOLS {
            code_length_counts[usize::from(self.raw_nbits[i])] += 1;
        }

        for i in 0..K_NUM_LZ77 {
            code_length_counts[usize::from(self.lz77_nbits[i])] += 1;
        }

        let mut code_length_nbits: [u8; 18] = [0; 18];
        let code_length_nbits_min: [u8; 18] = [0; 18];
        let code_length_nbits_max: [u8; 18] =
            [5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5];

        compute_code_lengths(
            &code_length_counts,
            18,
            &code_length_nbits_min,
            &code_length_nbits_max,
            &mut code_length_nbits
        );

        writer.put_bits(2, 0); // HSKIP = 0, i.e. don't skip code lengths.

        // As per Brotli RFC.
        let code_length_order: [u8; 18] =
            [1, 2, 3, 4, 0, 5, 17, 6, 16, 7, 8, 9, 10, 11, 12, 13, 14, 15];

        let code_length_length_nbits = [2, 4, 3, 2, 2, 4];
        let code_length_length_bits = [0, 7, 3, 2, 1, 15];

        let mut num_code_lengths: usize = 18;

        while code_length_nbits[usize::from(code_length_order[num_code_lengths - 1])] == 0 {
            num_code_lengths -= 1;
        }

        for i in 0..num_code_lengths {
            let symbol = usize::from(code_length_nbits[usize::from(code_length_order[i])]);

            let nbits = code_length_length_nbits[symbol];
            let bits = code_length_length_bits[symbol];

            writer.put_bits(nbits, bits);
        }
        // Compute the canonical codes for the codes that represent the lengths of
        // the actual codes for data.
        let mut code_length_bits: [u16; 18] = [0; 18];

        compute_canonical_code(&[], &mut [], &code_length_nbits, &mut code_length_bits);

        // Encode raw bit code lengths.
        for i in 0..K_NUM_RAW_SYMBOLS {
            let nbits = code_length_nbits[usize::from(self.raw_nbits[i])];
            let bits = u64::from(code_length_bits[usize::from(self.raw_nbits[i])]);

            writer.put_bits(nbits, bits);
        }

        let mut num_lz77 = K_NUM_LZ77;

        while self.lz77_nbits[num_lz77 - 1] == 0 {
            num_lz77 -= 1;
        }
        writer.put_bits(code_length_nbits[17], u64::from(code_length_bits[17]));
        writer.put_bits(3, 0b010); // 5
        writer.put_bits(code_length_nbits[17], u64::from(code_length_bits[17]));
        writer.put_bits(3, 0b000); // (5-2)*8 + 3 = 27
        writer.put_bits(code_length_nbits[17], u64::from(code_length_bits[17]));
        writer.put_bits(3, 0b010); // (27-2)*8 + 5 = 205

        // Encode LZ77 symbols, with values 224+i.
        for i in 0..num_lz77 {
            let nbits = code_length_nbits[usize::from(self.lz77_nbits[i])];
            let bits = u64::from(code_length_bits[usize::from(self.lz77_nbits[i])]);

            writer.put_bits(nbits, bits);
        }
    }
}

#[rustfmt::skip]
#[allow(clippy::zero_prefixed_literal, clippy::identity_op)]
fn bit_reverse(nbits: usize, bits: u16) -> u16
{
    const K_NIBBLE_LOOKUP: [u16; 16] = [
        0b0000, 0b1000, 0b0100, 0b1100,
        0b0010, 0b1010, 0b0110, 0b1110,
        0b0001, 0b1001, 0b0101, 0b1101,
        0b0011, 0b1011, 0b0111, 0b1111
    ];
    let rev16 =
        (K_NIBBLE_LOOKUP[usize::from((bits >> 00) & 0xF)] << 12)
            | (K_NIBBLE_LOOKUP[usize::from((bits >> 04) & 0xF)] << 08)
            | (K_NIBBLE_LOOKUP[usize::from((bits >> 08) & 0xF)] << 04)
            | (K_NIBBLE_LOOKUP[usize::from((bits >> 12) & 0xF)] << 00);

    rev16 >> 1 >> (16 - nbits - 1)
}

/// Create prefix codes given code lengths.
/// Supports code lengths being split into two halves
fn compute_canonical_code(
    first_chunk_nbits: &[u8], first_chunk_bits: &mut [u8], second_chunk_nbits: &[u8],
    second_chunk_bits: &mut [u16]
) {
    const K_MAX_CODE_LENGTH: usize = 15;
    let mut code_length_counts = [0; K_MAX_CODE_LENGTH + 1];

    for i in first_chunk_nbits {
        code_length_counts[usize::from(*i)] += 1;

        assert!(*i < K_MAX_CODE_LENGTH as u8);
        assert!(*i <= 8);
        assert!(*i > 0);
    }

    for i in second_chunk_nbits {
        code_length_counts[usize::from(*i)] += 1;
        assert!(*i <= K_MAX_CODE_LENGTH as u8);
    }

    let mut next_code = [0; K_MAX_CODE_LENGTH + 1];

    let mut code = 0;

    for i in 1..=K_MAX_CODE_LENGTH {
        code = (code + code_length_counts[i - 1]) << 1;
        next_code[i] = code;
    }

    for i in 0..first_chunk_bits.len() {
        first_chunk_bits[i] = bit_reverse(
            usize::from(first_chunk_nbits[i]),
            next_code[usize::from(first_chunk_nbits[i])]
        ) as u8;
        next_code[usize::from(first_chunk_nbits[i])] =
            next_code[usize::from(first_chunk_nbits[i])].wrapping_add(1);
    }

    for i in 0..second_chunk_bits.len() {
        second_chunk_bits[i] = bit_reverse(
            usize::from(second_chunk_nbits[i]),
            next_code[usize::from(second_chunk_nbits[i])]
        );
        next_code[usize::from(second_chunk_nbits[i])] =
            next_code[usize::from(second_chunk_nbits[i])].wrapping_add(1);
    }
}

/// Computes nbits[i] for i <= n, subject to min_limit[i] <= nbits[i] <=
/// max_limit[i] and sum 2**-nbits[i] == 1, so to minimize sum(nbits[i] *
/// freqs[i]).
fn compute_code_lengths_non_zero(
    freqs: &[u64], n: usize, min_limit: &mut [u8], max_limit: &[u8], nbits: &mut [u8]
) {
    let mut precision: u64 = 0;
    let mut shortest_length = 255;
    let mut freq_sum = 0;

    for i in 0..n {
        assert_ne!(freqs[i], 0);

        freq_sum += freqs[i];

        if min_limit[i] < 1 {
            min_limit[i] = 1;
        }
        assert!(min_limit[i] <= max_limit[i]);

        precision = max(u64::from(max_limit[i]), precision);
        shortest_length = min(min_limit[i], shortest_length);
    }

    // If all the minimum limits are greater than 1, shift precision so that we
    // behave as if the shortest was 1.
    precision -= u64::from(shortest_length) - 1;
    let infty = freq_sum * precision;

    let precision = precision as usize;

    compute_code_lengths_non_zero_impl(freqs, n, precision, infty, min_limit, max_limit, nbits)
}

fn compute_code_lengths_non_zero_impl(
    freqs: &[u64], n: usize, precision: usize, infty: u64, min_limit: &[u8], max_limit: &[u8],
    nbits: &mut [u8]
) {
    let len = ((1 << precision) + 1) * (n + 1);

    let mut dynp = vec![infty; len];

    let off_calc = |sym: usize, off: usize| -> usize {
        (sym * ((1_usize << precision) + 1)).wrapping_add(off)
    };

    dynp[off_calc(0, 0)] = 0;

    for sym in 0..n {
        for bits in min_limit[sym]..=max_limit[sym] {
            let off_delta = 1 << (precision - usize::from(bits));

            for off in 0..=((1 << precision) - off_delta) {
                let out_offset = off_calc(sym + 1, off + off_delta);

                // ran out of names
                let x = dynp[off_calc(sym, off)] + freqs[sym] * u64::from(bits);
                let y = dynp[out_offset];
                dynp[out_offset] = x.min(y);
            }
        }
    }
    let mut sym = n;
    let mut off = 1 << precision;

    let pos = off_calc(sym, off);
    assert_ne!(dynp[pos], infty);

    while sym > 0 {
        sym -= 1;

        assert!(off > 0);

        'inner: for bits in min_limit[sym]..=max_limit[sym] {
            let off_delta = 1 << (precision - usize::from(bits));
            // again, ran out of variable names
            let a = dynp[off_calc(sym + 1, off)];
            let c = freqs[sym] * u64::from(bits);

            if off_delta <= off && a == dynp[off_calc(sym, off - off_delta)] + c {
                off -= off_delta;
                nbits[sym] = bits;
                break 'inner;
            }
        }
    }
}

fn compute_code_lengths(
    freqs: &[u64], n: usize, min_limit_in: &[u8], max_limit_in: &[u8], nbits: &mut [u8]
) {
    assert!(n <= K_MAX_NUM_SYMBOLS);

    let mut compact_freqs = [0; K_MAX_NUM_SYMBOLS];
    let mut min_limit = [0; K_MAX_NUM_SYMBOLS];
    let mut max_limit = [0; K_MAX_NUM_SYMBOLS];

    let mut ni = 0;

    for i in 0..n {
        if freqs[i] != 0 {
            compact_freqs[ni] = freqs[i];
            min_limit[ni] = min_limit_in[i];
            max_limit[ni] = max_limit_in[i];
            ni += 1;
        }
    }
    let mut num_bits: [u8; K_MAX_NUM_SYMBOLS] = [0; K_MAX_NUM_SYMBOLS];

    compute_code_lengths_non_zero(
        &compact_freqs,
        ni,
        &mut min_limit,
        &max_limit,
        &mut num_bits
    );

    ni = 0;
    for i in 0..n {
        nbits[i] = 0;

        if freqs[i] != 0 {
            nbits[i] = num_bits[ni];
            ni += 1;
        }
    }
}

pub(crate) trait Enc<T: JxlBitEncoder> {
    fn chunk(&mut self, run: usize, residuals: &[T::Upixel], skip: usize, n: usize);
    fn finalize(&mut self, run: usize);
}

pub(crate) struct ChunkEncoder<'a, T: JxlBitEncoder> {
    output:      &'a mut BitWriter,
    prefix_code: &'a PrefixCode,
    encoder:     &'a T
}

impl<'a, T> ChunkEncoder<'a, T>
where
    T: JxlBitEncoder
{
    pub fn new(
        prefix_code: &'a PrefixCode, encoder: &'a T, output: &'a mut BitWriter
    ) -> ChunkEncoder<'a, T> {
        ChunkEncoder {
            prefix_code,
            encoder,
            output
        }
    }
    #[inline]
    fn encode_rle(&mut self, mut count: usize) {
        let (mut token, mut nbits, mut bits) = (0, 0, 0);
        let code = &self.prefix_code;

        if count == 0 {
            return;
        }
        count -= K_LZ77_MIN_LENGTH + 1;
        if count < K_LZ77CACHE_SIZE {
            self.output
                .put_bits(code.lz77_cache_nbits[count], code.lz77_cache_bits[count]);
        } else {
            encode_hybrid_uint_lz77(count as u32, &mut token, &mut nbits, &mut bits);
            let mut wbits = bits;
            let token_s = token as usize;

            wbits = (wbits << code.lz77_nbits[token_s]) | u32::from(code.lz77_bits[token_s]);
            wbits = (wbits << code.raw_nbits[0]) | u32::from(code.raw_bits[0]);

            let nbit_w = code.lz77_nbits[token_s] + (nbits & 0xFF) as u8 + code.raw_nbits[0];

            self.output.put_bits(nbit_w, u64::from(wbits))
        }
    }
    fn chunk(&mut self, run: usize, residuals: &[T::Upixel], skip: usize, n: usize) {
        self.encode_rle(run);

        let output = &mut self.output;
        let code = &self.prefix_code;

        self.encoder.encode_chunk(residuals, n, skip, code, output);
    }
    fn finalize(&mut self, run: usize) {
        self.encode_rle(run);
    }
}

impl<'a, T: JxlBitEncoder> Enc<T> for ChunkEncoder<'a, T> {
    fn chunk(&mut self, run: usize, residuals: &[T::Upixel], skip: usize, n: usize) {
        self.chunk(run, residuals, skip, n);
    }

    fn finalize(&mut self, run: usize) {
        self.finalize(run);
    }
}

impl<'a, T: JxlBitEncoder> Enc<T> for ChunkSampleCollector<'a, T> {
    fn chunk(&mut self, run: usize, residuals: &[T::Upixel], skip: usize, n: usize) {
        self.chunk(run, residuals, skip, n);
    }

    fn finalize(&mut self, _run: usize) {
        // no op
    }
}

pub(crate) struct ChannelRowProcessor<T: Enc<U>, U: JxlBitEncoder> {
    run: usize,
    x:   T,
    u:   PhantomData<U>
}

impl<T, U> ChannelRowProcessor<T, U>
where
    T: Enc<U>,
    U: JxlBitEncoder
{
    pub fn new(processor: T) -> ChannelRowProcessor<T, U> {
        ChannelRowProcessor {
            run: 0,
            x:   processor,
            u:   PhantomData::<U>
        }
    }

    #[allow(clippy::explicit_counter_loop, clippy::too_many_arguments)]
    fn process_chunk(
        &mut self, row: &[U::Pixel], row_left: &[U::Pixel], row_top: &[U::Pixel],
        row_topleft: &[U::Pixel], n: usize
    ) {
        let mut residuals: [U::Upixel; K_CHUNK_SIZE] = [U::upixel_default(); K_CHUNK_SIZE];

        let mut prefix_size: usize = 0;
        let mut required_prefix_size: usize = 0;

        for ix in 0..K_CHUNK_SIZE {
            let px = row[ix];
            let left = row_left[ix];
            let top = row_top[ix];
            let topleft = row_topleft[ix];

            let ac = left - topleft;
            let ab = left - top;
            let bc = top - topleft;

            let grad = ac + top;

            let d = ab ^ bc;
            let s = ac ^ bc;

            let clamp = if d < U::pixel_zero() { top } else { left };

            let pred = if s < U::pixel_zero() { grad } else { clamp };

            let v = px - pred;

            residuals[ix] = U::from_u32(packed_signed(v.into()));

            prefix_size = if prefix_size == required_prefix_size {
                prefix_size + usize::from(residuals[ix] == U::upixel_zero())
            } else {
                prefix_size
            };
            required_prefix_size += 1;
        }
        prefix_size = min(n, prefix_size);

        if prefix_size == n && (self.run > 0 || prefix_size > K_LZ77_MIN_LENGTH) {
            // Run continues, nothing to do.
            self.run += prefix_size;
        } else if prefix_size + self.run > K_LZ77_MIN_LENGTH {
            let run = self.run + prefix_size;
            // Run is broken. Encode the run and encode the individual vector.
            self.x.chunk(run, &residuals, prefix_size, n);
            self.run = 0;
        } else {
            // there was no run to begin with
            self.x.chunk(0, &residuals, 0, n);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn process_row(
        &mut self, row: &[U::Pixel], row_left: &[U::Pixel], row_top: &[U::Pixel],
        row_topleft: &[U::Pixel], xs: usize
    ) {
        for x in (0..xs).step_by(K_CHUNK_SIZE) {
            self.process_chunk(
                &row[x..],
                &row_left[x..],
                &row_top[x..],
                &row_topleft[x..],
                min(K_CHUNK_SIZE, xs - x)
            );
        }
    }
    pub fn finalize(&mut self) {
        self.x.finalize(self.run);
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_samples<B>(
    pixels: &[u8], x0: usize, y0: usize, xs: usize, row_stride: usize, row_count: usize,
    raw_counts: &mut [[u64; K_NUM_RAW_SYMBOLS]; 4], lz77_counts: &mut [[u64; K_NUM_LZ77]; 4],
    channels: usize
) where
    B: JxlBitEncoder
{
    let mut channel_row_processors: [Option<ChannelRowProcessor<ChunkSampleCollector<B>, B>>; 4] =
        [None, None, None, None];

    for (c, (raw_c, lz7_c)) in raw_counts
        .iter_mut()
        .zip(lz77_counts.iter_mut())
        .take(channels)
        .enumerate()
    {
        let chunk_collector = ChunkSampleCollector::<B>::new(raw_c, lz7_c);

        let channel = ChannelRowProcessor::new(chunk_collector);

        channel_row_processors[c] = Some(channel);
    }

    process_image_area(
        pixels,
        x0,
        y0,
        xs,
        1,
        1 + row_count,
        row_stride,
        channels,
        channel_row_processors
    )
}

#[allow(clippy::too_many_arguments)]
fn process_image_area<A, BitDepth>(
    pixels: &[u8], x0: usize, y0: usize, xs: usize, yskip: usize, ys: usize, row_stride: usize,
    channels: usize, mut processors: [Option<ChannelRowProcessor<A, BitDepth>>; 4]
) where
    BitDepth: JxlBitEncoder,
    A: Enc<BitDepth>
{
    const K_PADDING: usize = 32;
    const K_ALIGN: usize = 64;
    let k_align_pixels: usize = K_ALIGN / core::mem::size_of::<BitDepth::Pixel>();
    let k_num_px: usize = (256 + K_PADDING * 2 + k_align_pixels + k_align_pixels - 1)
        / k_align_pixels
        * k_align_pixels;

    let g1 = vec![BitDepth::pixel_zero(); k_num_px];
    let g = [g1.clone(), g1];

    let mut group_data = vec![g; channels];

    for y in 0..ys {
        let row_start = row_stride * (y0 + y) + x0 * channels * BitDepth::K_INPUT_BYTES;

        let rgba_row = &pixels[row_start..];

        let mut crow: [&mut [BitDepth::Pixel]; 4] = [&mut [], &mut [], &mut [], &mut []];
        let mut prow: [&mut [BitDepth::Pixel]; 4] = [&mut [], &mut [], &mut [], &mut []];

        for (i, cd) in group_data.iter_mut().take(channels).enumerate() {
            let xe = cd.split_at_mut(1);

            match y & 1 {
                0 => {
                    crow[i] = &mut xe.0[0][..];
                    prow[i] = &mut xe.1[0][..];
                }
                1 => {
                    crow[i] = &mut xe.1[0][..];
                    prow[i] = &mut xe.0[0][..];
                }
                _ => unreachable!()
            }
        }
        const K_OFFSET: usize = K_PADDING - 1;

        if channels == 1 {
            if BitDepth::K_INPUT_BYTES == 1 {
                fill_row_g8::<BitDepth::Pixel>(rgba_row, xs, &mut crow[0][K_PADDING..]);
            } else {
                fill_row_g16::<BitDepth::Pixel>(rgba_row, xs, &mut crow[0][K_PADDING..]);
            }
        } else if channels == 2 {
            let (l, a) = crow.split_at_mut(1);

            if BitDepth::K_INPUT_BYTES == 1 {
                fill_row_ga8::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut l[0][K_PADDING..],
                    &mut a[0][K_PADDING..]
                );
            } else {
                fill_row_ga16::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut l[0][K_PADDING..],
                    &mut a[0][K_PADDING..]
                );
            }
        } else if channels == 3 {
            let (yo, rest) = crow.split_at_mut(1);
            let (co, rest) = rest.split_at_mut(1);
            let (cg, _) = rest.split_at_mut(1);

            if BitDepth::K_INPUT_BYTES == 1 {
                fill_row_rgb8::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut yo[0][K_PADDING..],
                    &mut co[0][K_PADDING..],
                    &mut cg[0][K_PADDING..]
                );
            } else {
                fill_row_rgb16::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut yo[0][K_PADDING..],
                    &mut co[0][K_PADDING..],
                    &mut cg[0][K_PADDING..]
                );
            }
        } else if channels == 4 {
            let (yo, rest) = crow.split_at_mut(1);
            let (co, rest) = rest.split_at_mut(1);
            let (cg, rest) = rest.split_at_mut(1);
            let (ca, _) = rest.split_at_mut(1);

            if BitDepth::K_INPUT_BYTES == 1 {
                fill_row_rgba8::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut yo[0][K_PADDING..],
                    &mut co[0][K_PADDING..],
                    &mut cg[0][K_PADDING..],
                    &mut ca[0][K_PADDING..]
                );
            } else {
                fill_row_rgba16::<BitDepth::Pixel>(
                    rgba_row,
                    xs,
                    &mut yo[0][K_PADDING..],
                    &mut co[0][K_PADDING..],
                    &mut cg[0][K_PADDING..],
                    &mut ca[0][K_PADDING..]
                );
            }
        }

        for i in 0..channels {
            crow[i][K_OFFSET] = if y > 0 { prow[i][K_PADDING] } else { BitDepth::pixel_zero() };

            prow[i][K_OFFSET] = if y > 0 { prow[i][K_PADDING] } else { BitDepth::pixel_zero() };
        }

        if y < yskip {
            continue;
        }
        for i in 0..channels {
            let row = &crow[i][K_PADDING..];
            let row_left = &crow[i][K_OFFSET..];
            let row_top = if y == 0 { row_left } else { &prow[i][K_PADDING..] };
            let row_top_left = if y == 0 { row_left } else { &prow[i][K_OFFSET..] };

            let processor = &mut processors[i];

            if let Some(proc) = processor {
                proc.process_row(row, row_left, row_top, row_top_left, xs);
            } else {
                panic!("Channel without processor");
            }
        }
    }

    for processor in processors.iter_mut().flatten().take(channels) {
        processor.finalize();
    }
}

fn prepare_dc_global_common(
    is_single_group: bool, width: usize, height: usize, codes: &[PrefixCode],
    output: &mut BitWriter
) {
    let length = if is_single_group { width * height * 16 } else { 0 };

    output.allocate(100000 + length);

    output.put_bits(1, 1); // default DC dequantization factors (?)
    output.put_bits(1, 1); // use global tree / histograms
    output.put_bits(1, 0); // no lz77 for the tree

    output.put_bits(1, 1); // simple code for the tree's context map
    output.put_bits(2, 0); // all contexts clustered together
    output.put_bits(1, 1); // use prefix code for tree
    output.put_bits(4, 0); // 000 hybrid uint
    output.put_bits(6, 0b100011); // Alphabet size is 4 (var16)
    output.put_bits(2, 1); // simple prefix code
    output.put_bits(2, 3); // with 4 symbols
    output.put_bits(2, 0);
    output.put_bits(2, 1);
    output.put_bits(2, 2);
    output.put_bits(2, 3);
    output.put_bits(1, 0); // First tree encoding option

    let indices = [
        1, 2, 1, 4, 1, 0, 0, 5, 0, 0, 0, 0, 5, 0, 0, 0, 0, 5, 0, 0, 0, 0, 5, 0, 0, 0
    ];
    // Huffman table + extra bits for the tree.
    let symbol_bits: [u8; 6] = [0b00, 0b10, 0b001, 0b101, 0b0011, 0b0111];
    let symbol_nbits: [u8; 6] = [2, 2, 3, 3, 4, 4];

    for index in indices {
        output.put_bits(symbol_nbits[index], u64::from(symbol_bits[index]));
    }
    // Enable lz77 for main
    output.put_bits(1, 1); // Enable lz77 for the main bitstream
    output.put_bits(2, 0b00); // lz77 offset 224
    output.put_bits(4, 0b1010); // lz77 min length 7
                                // 400 hybrid uint config for lz77
    output.put_bits(4, 4);
    output.put_bits(3, 0);
    output.put_bits(3, 0);

    output.put_bits(1, 1); // simple code for the context map
    output.put_bits(2, 3); // 3 bits per entry
    output.put_bits(3, 4); // channel 3
    output.put_bits(3, 3); // channel 2
    output.put_bits(3, 2); // channel 1
    output.put_bits(3, 1); // channel 0
    output.put_bits(3, 0); // distance histogram first

    output.put_bits(1, 1); // use prefix codes
    output.put_bits(4, 0); // 000 hybrid uint config for distances (only need 0)
    for _ in 0..4 {
        output.put_bits(4, 0); // 000 hybrid uint config for symbols (only <= 10)
    }
    output.put_bits(5, 1); // 2: just need 1 for RLE (i.e. distance 1)
                           // symbol+lz77 alphabet size;
    for _ in 0..4 {
        output.put_bits(1, 1); // >1
        output.put_bits(4, 8); // <= 512
        output.put_bits(8, 256); // == 512
    }
    // distance histogram
    output.put_bits(2, 1); // simple prefix code
    output.put_bits(2, 0); // with one symbol
    output.put_bits(1, 1); // 1
                           // symbol+lz77 histogram
    for code in codes {
        code.write_to(output);
    }
    // group header for global modular image
    output.put_bits(1, 1);
    output.put_bits(1, 1);
}

#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn write_a_c_section<B: JxlBitEncoder>(
    pixels: &[u8], x0: usize, y0: usize, xs: usize, ys: usize, row_stride: usize,
    is_single_group: bool, depth: &B, channels: usize, prefix_code: &[PrefixCode],
    output: &mut [BitWriter; 4]
) {
    for i in 0..channels {
        if is_single_group && i == 0 {
            continue;
        }
        output[i].allocate(xs * ys * depth.max_encoded_bits_per_sample() + 4);
    }

    if !is_single_group {
        output[0].put_bits(1, 1);
        output[0].put_bits(1, 1);
        output[0].put_bits(2, 0);
    }

    let mut channel_row_processors: [Option<ChannelRowProcessor<ChunkEncoder<B>, B>>; 4] =
        [None, None, None, None];

    for (c, (pref_c, lz7_c)) in prefix_code
        .iter()
        .zip(output.iter_mut())
        .take(channels)
        .enumerate()
    {
        let chunk_collector = ChunkEncoder::<B>::new(pref_c, depth, lz7_c);

        let channel = ChannelRowProcessor::new(chunk_collector);

        channel_row_processors[c] = Some(channel);
    }

    process_image_area(
        pixels,
        x0,
        y0,
        xs,
        0,
        ys,
        row_stride,
        channels,
        channel_row_processors
    );
}

fn prepare_dc_global(
    is_single_group: bool, width: usize, height: usize, channels: usize, code: &[PrefixCode],
    output: &mut BitWriter
) {
    prepare_dc_global_common(is_single_group, width, height, code, output);

    if channels > 2 {
        output.put_bits(2, 0b01); // 1 transform
        output.put_bits(2, 0b00); // RCT
        output.put_bits(5, 0b00000); // Starting from ch 0
        output.put_bits(2, 0b00); // YCoCg
    } else {
        output.put_bits(2, 0b00); // no transforms
    }
    if !is_single_group {
        output.zero_pad();
    }
}

impl<'a> JxlSimpleEncoder<'a> {
    /// Create a new jpeg xl encoder
    ///
    /// # Arguments
    /// - data: Raw pixel data
    /// - options: Encoder options for the raw pixels, this include the width, height colorspace, depth etc
    pub fn new(data: &'a [u8], options: EncoderOptions) -> JxlSimpleEncoder<'a> {
        JxlSimpleEncoder { data, options }
    }

    /// Encode a jxl image producing the raw encoded
    /// bytes or an error if there was any that occurred during encoding
    ///
    /// # Returns
    /// Ok(usize): The number of bytes written to the sink
    ///
    /// Err(e): The error incase one is encountered
    ///
    pub fn encode<T: ZByteWriterTrait>(&self, sink: T) -> Result<usize, JxlEncodeErrors> {
        if self.options.width() <= 1 {
            return Err(JxlEncodeErrors::ZeroDimension("width"));
        }
        if self.options.height() <= 1 {
            return Err(JxlEncodeErrors::ZeroDimension("height"));
        }
        let depth = self.options.depth();

        let mut frame_state = match depth {
            BitDepth::Eight => self.encode_inner(UpTo8Bits())?,
            BitDepth::Sixteen => self.encode_inner(MoreThan14Bits())?,
            _ => return Err(JxlEncodeErrors::UnsupportedDepth(depth))
        };
        prepare_header(&mut frame_state, true, true);
        // TODO: Make this an encode_inner function
        let size = fast_lossless_max_required_output(&frame_state);

        let mut writer = ZWriter::new(sink);
        writer.reserve(size)?;
        fast_lossless_write_output(&mut frame_state, &mut writer)?;
        Ok(writer.bytes_written())
    }

    pub(crate) fn encode_inner<B: JxlBitEncoder + Send + Sync>(
        &self, encoder: B
    ) -> Result<FrameState, JxlEncodeErrors> {
        let depth = self.options.depth();
        let width = self.options.width();
        let height = self.options.height();
        let colorspace = self.options.colorspace();
        let effort = usize::from(self.options.effort()).clamp(0, 127);
        let num_components = colorspace.num_components();
        let stride = depth.size_of() * width * num_components;

        if !SUPPORTED_COLORSPACES.contains(&colorspace) {
            return Err(JxlEncodeErrors::UnsupportedColorspace(colorspace));
        }
        if width == 0 {
            return Err(JxlEncodeErrors::ZeroDimension("width"));
        }
        if height == 0 {
            return Err(JxlEncodeErrors::ZeroDimension("height"));
        }

        if log_enabled!(Level::Trace) {
            trace!("JXL details");
            trace!("Width: {}", width);
            trace!("Height: {}", height);
            trace!("Colorspace: {:?}", colorspace);
            trace!("Depth: {:?}", depth);
            trace!("Configured threads: {:?}", self.options.num_threads());
        }

        let expected = calculate_expected_input(&self.options);
        let found = self.data.len();

        if expected != found {
            return Err(JxlEncodeErrors::LengthMismatch(expected, found));
        }

        let num_groups_x = (width + 255) / 256;
        let num_groups_y = (height + 255) / 256;
        let num_dc_groups_x = (width + 2047) / 2048;
        let num_dc_groups_y = (height + 2047) / 2048;

        let mut raw_counts: [[u64; K_NUM_RAW_SYMBOLS]; 4] = [[0; K_NUM_RAW_SYMBOLS]; 4];
        let mut lz77_counts: [[u64; K_NUM_LZ77]; 4] = [[0; K_NUM_LZ77]; 4];

        let one_group = num_groups_x == 1 && num_groups_y == 1;

        for g in 0..num_groups_y * num_groups_x {
            let xg = g % num_groups_x;
            let yg = g / num_groups_x;
            let y_offset = yg * 256;
            let y_max = min(height - yg * 256, 256);
            let y_begin = y_offset + max(0, y_max.saturating_sub(2 * effort)) / 2;
            let y_count = min(2 * effort * y_max / 256, y_offset + y_max - y_begin - 1);
            let x_max = min(width - xg * 256, 256) / K_CHUNK_SIZE * K_CHUNK_SIZE;

            collect_samples::<B>(
                self.data,
                xg * 256,
                y_begin,
                x_max,
                stride,
                y_count,
                &mut raw_counts,
                &mut lz77_counts,
                num_components
            );
        }

        let mut base_raw_counts: [u64; K_NUM_RAW_SYMBOLS] = [
            3843, 852, 1270, 1214, 1014, 727, 481, 300, 159, 51, 5, 1, 1, 1, 1, 1, 1, 1, 1
        ];

        let doing_ycocg = num_components > 2;

        base_raw_counts[encoder.num_symbols(doing_ycocg)..K_NUM_RAW_SYMBOLS].fill(0);

        for raw_c in &mut raw_counts {
            for (r_c, bc_i) in raw_c.iter_mut().zip(base_raw_counts.iter()) {
                *r_c = ((*r_c) << 8) + (*bc_i);
            }
        }

        let base_lz77_counts: [u64; K_NUM_LZ77] = [
            29, 27, 25, 23, 21, 21, 19, 18, 21, 17, 16, 15, 15, 14, 13, 13, 137, 98, 61, 34, 1, 1,
            1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0
        ];

        for lz77_count in lz77_counts.iter_mut() {
            for (lz7_c, bxs_c) in lz77_count.iter_mut().zip(&base_lz77_counts) {
                *lz7_c = (*lz7_c << 8) + (*bxs_c);
            }
        }
        let mut codes = Vec::with_capacity(4);

        for i in 0..4 {
            let code = PrefixCode::new::<B>(&raw_counts[i], &lz77_counts[i]);
            codes.push(code);
        }

        let num_groups = if one_group {
            1
        } else {
            2 + num_dc_groups_x * num_dc_groups_y + num_groups_x * num_groups_y
        };

        let bit_writers = [
            BitWriter::new(),
            BitWriter::new(),
            BitWriter::new(),
            BitWriter::new()
        ];

        // The to #cfg's share a lot of code, but that is actually needed because otherwise
        // something will break

        #[cfg(feature = "std")]
        {
            use core::sync::atomic::{AtomicUsize, Ordering};
            use std::sync::{Arc, Mutex};

            let mut group_data = vec![];
            for _ in 0..num_groups {
                group_data.push(Mutex::new(bit_writers.clone()))
            }
            let group_data = Arc::new(group_data);

            {
                let writer = &mut group_data[0].lock().unwrap()[0];

                prepare_dc_global(one_group, width, height, num_components, &codes, writer);
            }
            let pixels = self.data;

            // this runner may run in multiple threads, so make it work
            // for both single threaded and multi-threaded
            let runner = |g: usize, threads: Arc<AtomicUsize>| {
                let xg = g % num_groups_x;
                let yg = g / num_groups_x;
                let group_id =
                    if one_group { 0 } else { 2 + num_dc_groups_x * num_dc_groups_y + g };
                let xs = min(width - xg * 256, 256);
                let ys = min(height - yg * 256, 256);
                let x0 = xg * 256;
                let y0 = yg * 256;
                // run in multiple threads, so make it thread safe
                let writers = &mut group_data[group_id].lock().unwrap();

                write_a_c_section(
                    pixels,
                    x0,
                    y0,
                    xs,
                    ys,
                    stride,
                    one_group,
                    &encoder,
                    num_components,
                    &codes,
                    writers
                );
                // This thread is done, kill it and tell the caller that
                // they can spawn another thread
                threads.fetch_sub(1, Ordering::Relaxed);
            };

            // set to true if the thread ran some runners
            // there are places it's better to do single thread
            // e.g for small images, so the runners are never ran
            let mut ran_runners = false;

            #[cfg(feature = "threads")]
            {
                if !one_group && self.options.num_threads() > 0 {
                    ran_runners = true;
                    let open_threads = Arc::new(AtomicUsize::new(0));

                    // Thread scope doesn't count number of threads
                    // so we do our own small one
                    std::thread::scope(|x| {
                        for i in 0..num_groups_x * num_groups_y {
                            let run_clone = runner;
                            let num_threads = open_threads.clone();

                            // spin if we opened more threads than requested,
                            // i.e the user asked for 10 threads but frame can be divided 11 times,
                            // we don't open 11 threads.
                            while num_threads.load(Ordering::Relaxed)
                                > usize::from(self.options.num_threads())
                            // assumed maximum threads to open
                            {
                                // tell CPU to wait, don't burn stuff
                                std::hint::spin_loop();
                            }

                            num_threads.fetch_add(1, Ordering::Relaxed);

                            x.spawn(move || run_clone(i, num_threads));
                        }
                    })
                }
            }

            // single threaded part of the multithreaded interface
            if !ran_runners {
                // we really don't care if we mess up here
                let dummy = Arc::new(AtomicUsize::new(num_groups + 10));

                for i in 0..num_groups_x * num_groups_y {
                    runner(i, dummy.clone());
                }
            }
            // By this point, all threads have finished since we used
            // scoped threads

            // remove Mutex and Arc by consuming previous vector into new one
            let group_data: Vec<[BitWriter; 4]> = Arc::try_unwrap(group_data)
                .unwrap()
                .into_iter()
                .map(|x| x.into_inner().unwrap())
                .collect();

            Ok(FrameState {
                option: self.options,
                header: BitWriter::new(),
                group_data,
                current_bit_writer: 0,
                bit_writer_byte_pos: 0
            })
        }
        #[cfg(not(feature = "std"))]
        {
            let mut group_data = vec![];
            for _ in 0..num_groups {
                group_data.push(bit_writers.clone())
            }

            {
                let writer = &mut group_data[0][0];
                prepare_dc_global(one_group, width, height, num_components, &codes, writer);
            }
            let pixels = self.data;

            // this runner may run in multiple threads, so make it work
            // for both single threaded and multi-threaded
            let mut runner = |g: usize| {
                let xg = g % num_groups_x;
                let yg = g / num_groups_x;
                let group_id =
                    if one_group { 0 } else { 2 + num_dc_groups_x * num_dc_groups_y + g };
                let xs = min(width - xg * 256, 256);
                let ys = min(height - yg * 256, 256);
                let x0 = xg * 256;
                let y0 = yg * 256;
                // run in multiple threads, so make it thread safe
                let writers = &mut group_data[group_id];

                write_a_c_section(
                    pixels,
                    x0,
                    y0,
                    xs,
                    ys,
                    stride,
                    one_group,
                    &encoder,
                    num_components,
                    &codes,
                    writers
                );
                // This thread is done, kill it and tell the caller that
                // they can spawn another thread
            };

            for i in 0..num_groups_x * num_groups_y {
                runner(i);
            }

            Ok(FrameState {
                option: self.options,
                header: BitWriter::new(),
                group_data,
                current_bit_writer: 0,
                bit_writer_byte_pos: 0
            })
        }
    }
}

/// Write output from the frame to `output`
#[allow(clippy::never_loop)]
fn fast_lossless_write_output<T: ZByteWriterTrait>(
    frame: &mut FrameState, output: &mut ZWriter<T>
) -> Result<(), JxlEncodeErrors> {
    let components = frame.option.colorspace().num_components();

    let mut out_writer = BorrowingBitWriter::new(output);

    let max_iters = 1 + frame.group_data.len() * components;

    loop {
        let curr = &mut frame.current_bit_writer;
        let bw_pos = &mut frame.bit_writer_byte_pos;

        if *curr >= max_iters {
            out_writer.flush()?;
            assert_eq!(out_writer.bits_in_buffer, 0);
            return Ok(());
        }

        let nbc = frame.option.colorspace().num_components();

        let writer = if *curr == 0 {
            &mut frame.header
        } else {
            &mut frame.group_data[(*curr - 1) / nbc][(*curr - 1) % nbc]
        };

        if writer.bits_in_buffer > 0 {
            writer.flush();
        }

        let full_byte_count = writer.position - *bw_pos;

        out_writer.put_bytes(&writer.dest[*bw_pos..*bw_pos + full_byte_count])?;
        *bw_pos += full_byte_count;

        if *bw_pos == writer.position {
            // check for any spare bytes
            if writer.bits_in_buffer != 0 {
                // transfer those bits to our general writer
                out_writer.put_bits(writer.bits_in_buffer, writer.buffer)?;
            }
            *bw_pos = 0;
            *curr += 1;

            out_writer.flush()?;

            if (*curr - 1) % nbc == 0 && out_writer.bits_in_buffer != 0 {
                out_writer.put_bits(8 - out_writer.bits_in_buffer, 0)?;
            }
        }
    }
}

fn fast_lossless_output(frame_state: &FrameState) -> usize {
    let mut total_size_groups = 0;

    for group in &frame_state.group_data {
        let mut sz = 0;

        for writer in group.iter() {
            sz += (writer.position * 8) + usize::from(writer.bits_in_buffer);
        }

        sz = (sz + 7) / 8;
        total_size_groups += sz;
    }
    frame_state.header.position + total_size_groups
}

fn fast_lossless_max_required_output(frame_state: &FrameState) -> usize {
    fast_lossless_output(frame_state) + 32
}

#[allow(clippy::needless_range_loop)]
fn prepare_header(frame: &mut FrameState, add_image_header: bool, is_last: bool) {
    let colorspace = frame.option.colorspace();
    let depth = frame.option.depth();

    let output = &mut frame.header;
    output.allocate(1000 + frame.group_data.len() * 32);

    let mut group_sizes = vec![0; frame.group_data.len()];

    for i in 0..frame.group_data.len() {
        let mut sz = 0;
        for j in 0..frame.option.colorspace().num_components() {
            let writer = &frame.group_data[i][j];

            sz += (writer.position * 8) + usize::from(writer.bits_in_buffer);
        }
        sz = (sz + 7) / 8;
        group_sizes[i] = sz;
    }

    let have_alpha = colorspace.has_alpha();

    if add_image_header {
        // signature
        output.put_bits(16, 0x0AFF);
        // Size header, hand-crafted
        // not small
        output.put_bits(1, 0);

        let mut write_header = |size: usize, add_ration: bool| {
            assert!(size > 1 && size < (1 << 30));
            if size - 1 < (1 << 9) {
                output.put_bits(2, 0);
                output.put_bits(9, (size - 1) as u64);
            } else if size - 1 < (1 << 13) {
                output.put_bits(2, 1);
                output.put_bits(13, (size - 1) as u64);
            } else if size - 1 < (1 << 18) {
                output.put_bits(2, 2);
                output.put_bits(18, (size - 1) as u64);
            } else {
                output.put_bits(2, 3);
                output.put_bits(30, (size - 1) as u64);
            }
            if add_ration {
                output.put_bits(3, 0);
            }
        };
        write_header(frame.option.height(), true);

        write_header(frame.option.width(), false);
        // hand crafted image metadata
        output.put_bits(1, 0); // defaults
        output.put_bits(1, 0); // extra fields
        output.put_bits(1, 0); // bit depth floating point sample

        match depth {
            BitDepth::Eight => output.put_bits(2, 0),
            _ => {
                output.put_bits(2, 0b11);
                output.put_bits(6, (frame.option.depth().bit_size() - 1) as u64);
            }
        };
        if frame.option.depth().bit_size() <= 14 {
            // 16 bit buffer sufficient
            output.put_bits(1, 1);
        } else {
            // not sufficient
            output.put_bits(1, 0);
        }
        if have_alpha {
            output.put_bits(2, 1); // extra channel
            output.put_bits(1, 1); // all default (8 bit alpha)
        } else {
            output.put_bits(2, 0); // no extra channel
        }

        // TODO: Support XYB
        output.put_bits(1, 0); // not xyb

        if colorspace.num_components() > 2 {
            output.put_bits(1, 1); // color_encoding.all_default (sRGB)
        } else {
            output.put_bits(1, 0); // color_encoding.all_default false
            output.put_bits(1, 0); // color_encoding.want_icc false
            output.put_bits(2, 1); // grayscale
            output.put_bits(2, 1); // D65
            output.put_bits(1, 0); // no gamma transfer function
            output.put_bits(2, 0b10); // tf: 2 + u(4)
            output.put_bits(4, 11); // tf of sRGB
            output.put_bits(2, 1); // relative rendering intent
        }
        output.put_bits(2, 0b00); // No extensions.

        output.put_bits(1, 1); // all_default transform data

        // No ICC, no preview. Frame should start at byte boundery.
        output.zero_pad();
    }

    // Handcrafted frame header.
    output.put_bits(1, 0); // all_default
    output.put_bits(2, 0b00); // regular frame
    output.put_bits(1, 1); // modular
    output.put_bits(2, 0b00); // default flags
    output.put_bits(1, 0); // not YCbCr
    output.put_bits(2, 0b00); // no upsampling

    if have_alpha {
        output.put_bits(2, 0b00); // no alpha upsampling
    }
    output.put_bits(2, 0b01); // default group size
    output.put_bits(2, 0b00); // exactly one pass
    output.put_bits(1, 0); // no custom size or origin
    output.put_bits(2, 0b00); // kReplace blending mode
    if have_alpha {
        output.put_bits(2, 0b00); // kReplace blending mode for alpha channel
    }
    output.put_bits(1, u64::from(is_last)); // is_last
    output.put_bits(2, 0b00); // a frame has no name
    output.put_bits(1, 0); // loop filter is not all_default
    output.put_bits(1, 0); // no gaborish
    output.put_bits(2, 0); // 0 EPF iters
    output.put_bits(2, 0b00); // No LF extensions
    output.put_bits(2, 0b00); // No FH extensions

    output.put_bits(1, 0); // No TOC permutation
    output.zero_pad(); // TOC is byte-aligned.

    for i in 0..frame.group_data.len() {
        let size = group_sizes[i] as u64;

        if size < (1 << 10) {
            output.put_bits(2, 0b00);
            output.put_bits(10, size);
        } else if size - 1024 < (1 << 14) {
            output.put_bits(2, 0b01);
            output.put_bits(14, size - 1024);
        } else if size - 17408 < (1 << 22) {
            output.put_bits(2, 0b10);
            output.put_bits(22, size - 17408);
        } else {
            output.put_bits(2, 0b11);
            output.put_bits(30, size - 4211712);
        }
    }
    output.zero_pad();
}

fn calculate_expected_input(options: &EncoderOptions) -> usize {
    options
        .width()
        .checked_mul(options.depth().size_of())
        .unwrap()
        .checked_mul(options.height())
        .unwrap()
        .checked_mul(options.colorspace().num_components())
        .unwrap()
}

// #[test]
// fn hello()
// {
//     let file = std::fs::read("").unwrap();
//     let mut data = zune_ppm::PPMDecoder::new(&file);
//     let bytes = data.decode().unwrap().u8().unwrap();
//     let (width, height) = data.get_dimensions().unwrap();
//     let colorspace = data.get_colorspace().unwrap();
//     let depth = data.get_bit_depth().unwrap();
//
//     let opts = EncoderOptions::default()
//         .set_width(width)
//         .set_height(height)
//         .set_colorspace(colorspace)
//         .set_depth(depth);
//
//     let encoder = JxlSimpleEncoder::new(&bytes, opts);
//     encoder.encode().unwrap();
// }
