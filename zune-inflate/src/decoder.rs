#![allow(clippy::never_loop)]

use std::cmp::min;

use crate::bitstream::BitStreamReader;
use crate::constants::{
    DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN, DEFLATE_BLOCKTYPE_STATIC, DEFLATE_BLOCKTYPE_UNCOMPRESSED,
    DEFLATE_MAX_CODEWORD_LENGTH, DEFLATE_MAX_LITLEN_CODEWORD_LENGTH, DEFLATE_MAX_NUM_SYMS,
    DEFLATE_MAX_OFFSET_CODEWORD_LENGTH, DEFLATE_MAX_PRE_CODEWORD_LEN, DEFLATE_NUM_LITLEN_SYMS,
    DEFLATE_NUM_OFFSET_SYMS, DEFLATE_NUM_PRECODE_SYMS, DEFLATE_PRECODE_LENS_PERMUTATION,
    DELFATE_MAX_LENS_OVERRUN, HUFFDEC_EXCEPTIONAL, HUFFDEC_SUITABLE_POINTER, LITLEN_DECODE_RESULTS,
    LITLEN_ENOUGH, LITLEN_TABLE_BITS, OFFSET_DECODE_RESULTS, OFFSET_ENOUGH, OFFSET_TABLEBITS,
    PRECODE_DECODE_RESULTS, PRECODE_ENOUGH, PRECODE_TABLE_BITS
};
use crate::enums::DeflateState;
use crate::errors::ZlibDecodeErrors;
use crate::errors::ZlibDecodeErrors::InsufficientData;
use crate::utils::make_decode_table_entry;

pub struct DeflateDecoder<'a>
{
    data:                &'a [u8],
    position:            usize,
    state:               DeflateState,
    stream:              BitStreamReader<'a>,
    is_last_block:       bool,
    static_codes_loaded: bool
}

impl<'a> DeflateDecoder<'a>
{
    pub fn new(data: &'a [u8]) -> DeflateDecoder<'a>
    {
        // create stream

        DeflateDecoder {
            data,
            position: 0,
            state: DeflateState::Initialized,
            stream: BitStreamReader::new(data),
            is_last_block: false,
            static_codes_loaded: false
        }
    }
    pub fn decode_zlib(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        if self.data.len()
            < 2 /* zlib header */
            + 4
        /* Deflate */
        {
            return Err(InsufficientData);
        }

        // Zlib flags
        // See https://www.ietf.org/rfc/rfc1950.txt for
        // the RFC
        let cmf = self.data[0];
        let flg = self.data[1];

        let cm = cmf & 0xF;
        let cinfo = cmf >> 4;

        // let fcheck = flg & 0xF;
        // let fdict = (flg >> 4) & 1;
        // let flevel = flg >> 5;

        // confirm we have the right deflate methods
        if cm != 8
        {
            if cm == 15
            {
                return Err(ZlibDecodeErrors::Generic(
                    "CM of 15 is preserved by the standard,currently don't know how to handle it"
                ));
            }
            return Err(ZlibDecodeErrors::GenericStr(format!(
                "Unknown zlib compression method {}",
                cm
            )));
        }
        if cinfo > 7
        {
            return Err(ZlibDecodeErrors::GenericStr(format!(
                "Unknown cinfo `{}` greater than 7, not allowed",
                cinfo
            )));
        }
        let flag_checks = (u16::from(cmf) * 256) + u16::from(flg);

        if flag_checks % 31 != 0
        {
            return Err(ZlibDecodeErrors::Generic("FCHECK integrity not preserved"));
        }

        self.position = 2;

        self.decode_deflate()?;

        Ok(())
    }
    ///Decode a deflate stream
    pub fn decode_deflate(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        match self.state
        {
            DeflateState::Initialized => self.start_deflate_block()?,

            _ => todo!()
        }
        Ok(())
    }
    fn start_deflate_block(&mut self) -> Result<(), ZlibDecodeErrors>
    {
        let mut precode_lens = [0; DEFLATE_NUM_PRECODE_SYMS];
        let mut precode_decode_table = [0_u32; PRECODE_ENOUGH];
        let mut litlen_decode_table = [0_u32; LITLEN_ENOUGH];
        let mut offset_decode_table = [0; OFFSET_ENOUGH];

        // start deflate decode
        // re-read the stream so that we can remove code read by zlib
        self.stream = BitStreamReader::new(&self.data[self.position..]);

        self.stream.refill();

        let mut out_block = Vec::<u8>::with_capacity(37000);
        // bits used
        self.is_last_block = self.stream.get_bits(1) == 1;
        let block_type = self.stream.get_bits(2);

        let overread_count = 0;
        const COUNT: usize =
            DEFLATE_NUM_LITLEN_SYMS + DEFLATE_NUM_OFFSET_SYMS + DELFATE_MAX_LENS_OVERRUN;

        let mut lens = [0_u8; COUNT];

        let mut num_litlen_syms = 0;
        let mut num_offset_syms = 0;

        'block: loop
        {
            if block_type == DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN
            {
                const SINGLE_PRECODE: usize = 3;

                // Dynamic Huffman block
                // Read codeword lengths
                num_litlen_syms = 257 + (self.stream.get_bits(5)) as usize;
                num_offset_syms = 1 + (self.stream.get_bits(5)) as usize;

                let num_explicit_precode_lens = 4 + (self.stream.get_bits(4)) as usize;

                self.static_codes_loaded = false;

                self.stream.refill();

                let expected = (SINGLE_PRECODE * num_explicit_precode_lens) as u8;

                if !self.stream.has(expected)
                {
                    return Err(ZlibDecodeErrors::InsufficientData);
                }

                for i in DEFLATE_PRECODE_LENS_PERMUTATION
                    .iter()
                    .take(num_explicit_precode_lens)
                {
                    let bits = self.stream.get_bits(3) as u8;

                    precode_lens[usize::from(*i)] = bits;
                }

                self.build_decode_table(
                    &precode_lens,
                    &PRECODE_DECODE_RESULTS,
                    &mut precode_decode_table,
                    PRECODE_TABLE_BITS,
                    DEFLATE_NUM_PRECODE_SYMS,
                    DEFLATE_MAX_CODEWORD_LENGTH
                )?;

                /* Decode the litlen and offset codeword lengths. */

                let mut i = 0;

                loop
                {
                    let rep_val: u8;
                    let rep_count: u64;

                    if !self.stream.has(DEFLATE_MAX_PRE_CODEWORD_LEN + 7)
                    {
                        self.stream.refill();
                    }
                    // decode next precode symbol
                    let entry_pos = self
                        .stream
                        .peek_bits::<{ DEFLATE_MAX_PRE_CODEWORD_LEN as usize }>();

                    let entry = precode_decode_table[entry_pos];
                    let presym = entry >> 16;

                    self.stream.drop_bits(entry as u8);

                    if presym < 16
                    {
                        // explicit codeword length
                        lens[i] = presym as u8;
                        i += 1;
                        continue;
                    }

                    /* Run-length encoded codeword lengths */

                    /*
                     * Note: we don't need verify that the repeat count
                     * doesn't overflow the number of elements, since we've
                     * sized the lens array to have enough extra space to
                     * allow for the worst-case overrun (138 zeroes when
                     * only 1 length was remaining).
                     *
                     * In the case of the small repeat counts (presyms 16
                     * and 17), it is fastest to always write the maximum
                     * number of entries.  That gets rid of branches that
                     * would otherwise be required.
                     *
                     * It is not just because of the numerical order that
                     * our checks go in the order 'presym < 16', 'presym ==
                     * 16', and 'presym == 17'.  For typical data this is
                     * ordered from most frequent to least frequent case.
                     */
                    if presym == 16
                    {
                        // repeat previous length three to 6 times
                        if i == 0
                        {
                            return Err(ZlibDecodeErrors::CorruptData);
                        }
                        rep_val = lens[i - 1];
                        rep_count = 3 + self.stream.get_bits(2);

                        lens[i..i + 6].fill(rep_val);

                        i += rep_count as usize;
                    }
                    else if presym == 17
                    {
                        /* Repeat zero 3 - 10 times. */
                        rep_count = 3 + self.stream.get_bits(3);

                        lens[i..i + 10].fill(0);

                        i += rep_count as usize;
                    }
                    else
                    {
                        // repeat zero 11-138 times.
                        rep_count = 11 + self.stream.get_bits(7);

                        lens[i..i + rep_count as usize].fill(0);

                        i += rep_count as usize;
                    }
                    if i >= num_litlen_syms + num_offset_syms
                    {
                        break;
                    }
                }
            }
            else if block_type == DEFLATE_BLOCKTYPE_UNCOMPRESSED
            {
                /*
                 * Uncompressed block: copy 'len' bytes literally from the input
                 * buffer to the output buffer.
                 */

                /*
                 * Align the bitstream to the next byte boundary.  This means
                 * the next byte boundary as if we were reading a byte at a
                 * time.  Therefore, we have to rewind 'in_next' by any bytes
                 * that have been refilled but not actually consumed yet (not
                 * counting overread bytes, which don't increment 'in_next').
                 * The RFC says that
                 * skip any remaining bits in current partially
                 *       processed byte
                 *     read LEN and NLEN (see next section)
                 *     copy LEN bytes of data to output
                 */

                if overread_count > self.stream.get_bits_left() >> 3
                {
                    return Err(ZlibDecodeErrors::Generic("Over-read stream"));
                }
                let partial_bits = usize::from((self.stream.get_bits_left() & 7) != 0);
                // advance if we have extra bits
                self.stream.advance(partial_bits);
                let len = self.stream.get_bits(16) as usize;
                let nlen = self.stream.get_bits(16) as usize;

                // copy to deflate
                if len != !nlen
                {
                    return Err(ZlibDecodeErrors::Generic("Len and nlen do not match"));
                }

                let start = self.stream.get_position();
                out_block.extend_from_slice(&self.data[start..start + len]);
            }
            else if block_type == DEFLATE_BLOCKTYPE_STATIC
            {
                if self.static_codes_loaded
                {
                    break;
                }

                self.static_codes_loaded = true;

                lens[000..144].fill(8);
                lens[144..256].fill(9);
                lens[256..280].fill(7);
                lens[280..288].fill(8);
                lens[288..].fill(5);

                num_litlen_syms = 288;
                num_offset_syms = 32;
            }

            // build offset decode table
            self.build_decode_table(
                &lens[num_litlen_syms..],
                &OFFSET_DECODE_RESULTS,
                &mut offset_decode_table,
                OFFSET_TABLEBITS,
                num_offset_syms,
                DEFLATE_MAX_OFFSET_CODEWORD_LENGTH
            )?;

            self.build_decode_table(
                &lens,
                &LITLEN_DECODE_RESULTS,
                &mut litlen_decode_table,
                LITLEN_TABLE_BITS,
                num_litlen_syms,
                DEFLATE_MAX_LITLEN_CODEWORD_LENGTH
            )?;

            /*
             * This is the "fastloop" for decoding literals and matches.  It does
             * bounds checks on in_next and out_next in the loop conditions so that
             * additional bounds checks aren't needed inside the loop body.
             *
             * To reduce latency, the bitbuffer is refilled and the next litlen
             * decode table entry is preloaded before each loop iteration.
             */

            'decode: loop
            {
                let litlen_decode_bits: usize =
                    min(DEFLATE_MAX_LITLEN_CODEWORD_LENGTH, LITLEN_TABLE_BITS);

                self.stream.refill();

                let entry_pos = self.stream.peek_bits_no_const(litlen_decode_bits);

                let entry = litlen_decode_table[entry_pos];

                'sequence: loop
                {
                    // sequence loop
                    /*
                     * Consume the bits for the litlen decode table entry.  Save the
                     * original bitbuf for later, in case the extra match length
                     * bits need to be extracted from it.
                     */
                    let saved_bitbuf = self.stream.buffer;

                    self.stream.drop_bits(entry as u8);
                    break;
                }

                break;
            }

            if self.is_last_block
            {
                break;
            }
        }

        Ok(())
    }
    /// Build the decode table for the precode
    fn build_decode_table(
        &mut self, lens: &[u8], decode_results: &[u32], decode_table: &mut [u32],
        table_bits: usize, num_syms: usize, mut max_codeword_len: usize
    ) -> Result<(), ZlibDecodeErrors>
    {
        const BITS: u32 = usize::BITS - 1;

        let mut len_counts: [u32; DEFLATE_MAX_CODEWORD_LENGTH + 1] =
            [0; DEFLATE_MAX_CODEWORD_LENGTH + 1];
        let mut offsets: [u32; DEFLATE_MAX_CODEWORD_LENGTH + 1] =
            [0; DEFLATE_MAX_CODEWORD_LENGTH + 1];
        let mut sorted_syms: [u16; DEFLATE_MAX_NUM_SYMS] = [0; DEFLATE_MAX_NUM_SYMS];

        let mut i;

        // count how many codewords have each length, including 0.
        for sym in 0..num_syms
        {
            len_counts[usize::from(lens[sym])] += 1;
        }

        /*
         * Determine the actual maximum codeword length that was used, and
         * decrease table_bits to it if allowed.
         */
        while max_codeword_len > 1 && len_counts[max_codeword_len] == 0
        {
            max_codeword_len -= 1;
        }
        /*
         * Sort the symbols primarily by increasing codeword length and
         *	A temporary array of length @num_syms.
         * secondarily by increasing symbol value; or equivalently by their
         * codewords in lexicographic order, since a canonical code is assumed.
         *
         * For efficiency, also compute 'codespace_used' in the same pass over
         * 'len_counts[]' used to build 'offsets[]' for sorting.
         */
        offsets[0] = 0;
        offsets[1] = len_counts[0];

        let mut codespace_used = 0_u32;

        for len in 1..max_codeword_len
        {
            offsets[len + 1] = offsets[len] + len_counts[len];
            codespace_used = (codespace_used << 1) + len_counts[len];
        }
        codespace_used = (codespace_used << 1) + len_counts[max_codeword_len];

        for sym in 0..num_syms
        {
            let pos = usize::from(lens[sym]);
            sorted_syms[offsets[pos] as usize] = sym as u16;
            offsets[pos] += 1;
        }
        i = (offsets[0]) as usize;

        /*
         * Check whether the lengths form a complete code (exactly fills the
         * codespace), an incomplete code (doesn't fill the codespace), or an
         * overfull code (overflows the codespace).  A codeword of length 'n'
         * uses proportion '1/(2^n)' of the codespace.  An overfull code is
         * nonsensical, so is considered invalid.  An incomplete code is
         * considered valid only in two specific cases; see below.
         */

        // Overfull code
        if codespace_used > 1 << max_codeword_len
        {
            return Err(ZlibDecodeErrors::Generic("Overflown code"));
        }
        // incomplete code
        if codespace_used < 1 << max_codeword_len
        {
            let entry = if codespace_used == 0
            {
                /*
                 * An empty code is allowed.  This can happen for the
                 * offset code in DEFLATE, since a dynamic Huffman block
                 * need not contain any matches.
                 */

                /* sym=0, len=1 (arbitrary) */
                make_decode_table_entry(decode_results, 0, 1)
            }
            else
            {
                /*
                 * Allow codes with a single used symbol, with codeword
                 * length 1.  The DEFLATE RFC is unclear regarding this
                 * case.  What zlib's decompressor does is permit this
                 * for the litlen and offset codes and assume the
                 * codeword is '0' rather than '1'.  We do the same
                 * except we allow this for precodes too, since there's
                 * no convincing reason to treat the codes differently.
                 * We also assign both codewords '0' and '1' to the
                 * symbol to avoid having to handle '1' specially.
                 */
                if codespace_used != 1 << max_codeword_len || len_counts[1] != 1
                {
                    return Err(ZlibDecodeErrors::Generic(
                        "Cannot work with empty pre-code table"
                    ));
                }
                make_decode_table_entry(decode_results, usize::from(sorted_syms[i]), 1)
            };
            /*
             * Note: the decode table still must be fully initialized, in
             * case the stream is malformed and contains bits from the part
             * of the codespace the incomplete code doesn't use.
             */
            decode_table.fill(entry);
            return Ok(());
        }

        /*
         * The lengths form a complete code.  Now, enumerate the codewords in
         * lexicographic order and fill the decode table entries for each one.
         *
         * First, process all codewords with len <= table_bits.  Each one gets
         * '2^(table_bits-len)' direct entries in the table.
         *
         * Since DEFLATE uses bit-reversed codewords, these entries aren't
         * consecutive but rather are spaced '2^len' entries apart.  This makes
         * filling them naively somewhat awkward and inefficient, since strided
         * stores are less cache-friendly and preclude the use of word or
         * vector-at-a-time stores to fill multiple entries per instruction.
         *
         * To optimize this, we incrementally double the table size.  When
         * processing codewords with length 'len', the table is treated as
         * having only '2^len' entries, so each codeword uses just one entry.
         * Then, each time 'len' is incremented, the table size is doubled and
         * the first half is copied to the second half.  This significantly
         * improves performance over naively doing strided stores.
         *
         * Note that some entries copied for each table doubling may not have
         * been initialized yet, but it doesn't matter since they're guaranteed
         * to be initialized later (because the Huffman code is complete).
         */
        let mut codeword = 0;
        let mut len = 1;
        let mut count = len_counts[1];

        while count == 0
        {
            len += 1;

            if len >= len_counts.len()
            {
                break;
            }
            count = len_counts[len];
        }

        let mut curr_table_end = 1 << len;

        while len <= table_bits
        {
            // Process all count codewords with length len

            loop
            {
                let entry = make_decode_table_entry(
                    decode_results,
                    usize::from(sorted_syms[i]),
                    len as u32
                );
                i += 1;
                // fill first entry for current codeword
                decode_table[codeword] = entry;

                if codeword == curr_table_end - 1
                {
                    // last codeword (all 1's)
                    for _ in len..table_bits
                    {
                        decode_table.copy_within(0..curr_table_end, curr_table_end);

                        curr_table_end <<= 1;
                    }
                    return Ok(());
                }
                /*
                 * To advance to the lexicographically next codeword in
                 * the canonical code, the codeword must be incremented,
                 * then 0's must be appended to the codeword as needed
                 * to match the next codeword's length.
                 *
                 * Since the codeword is bit-reversed, appending 0's is
                 * a no-op.  However, incrementing it is nontrivial.  To
                 * do so efficiently, use the 'bsr' instruction to find
                 * the last (highest order) 0 bit in the codeword, set
                 * it, and clear any later (higher order) 1 bits.  But
                 * 'bsr' actually finds the highest order 1 bit, so to
                 * use it first flip all bits in the codeword by XOR'ing
                 * it with (1U << len) - 1 == cur_table_end - 1.
                 */

                let adv = BITS - (codeword ^ (curr_table_end - 1)).leading_zeros();
                let bit = 1 << adv;

                codeword &= bit - 1;
                codeword |= bit;
                count -= 1;

                if count == 0
                {
                    break;
                }
            }
            // advance to the next codeword length
            loop
            {
                len += 1;

                if len <= table_bits
                {
                    // dest is decode_table[curr_table_end]
                    // source is decode_table(start of table);
                    // size is curr_table;

                    decode_table.copy_within(0..curr_table_end, curr_table_end);

                    //decode_table.copy_within(range, curr_table_end);
                    curr_table_end <<= 1;
                }
                count = len_counts[len];

                if count != 0
                {
                    break;
                }
            }
        }
        // process codewords with len > table_bits.
        // Require sub-tables
        curr_table_end = 1 << table_bits;

        let mut subtable_prefix = usize::MAX;
        let mut subtable_start = 0;
        let mut subtable_bits;

        loop
        {
            /*
             * Start a new sub-table if the first 'table_bits' bits of the
             * codeword don't match the prefix of the current subtable.
             */
            if codeword & ((1_usize << table_bits) - 1) != subtable_prefix
            {
                subtable_prefix = codeword & ((1 << table_bits) - 1);
                subtable_start = curr_table_end;

                /*
                 * Calculate the subtable length.  If the codeword has
                 * length 'table_bits + n', then the subtable needs
                 * '2^n' entries.  But it may need more; if fewer than
                 * '2^n' codewords of length 'table_bits + n' remain,
                 * then the length will need to be incremented to bring
                 * in longer codewords until the subtable can be
                 * completely filled.  Note that because the Huffman
                 * code is complete, it will always be possible to fill
                 * the sub-table eventually.
                 */
                subtable_bits = len - table_bits;
                codespace_used = count;

                while codespace_used < (1 << subtable_bits)
                {
                    subtable_bits += 1;

                    if subtable_bits + table_bits > 15
                    {
                        return Err(ZlibDecodeErrors::CorruptData);
                    }

                    codespace_used = (codespace_used << 1) + len_counts[table_bits + subtable_bits];
                }

                decode_table[subtable_prefix] = (subtable_start as u32) << 16
                    | HUFFDEC_EXCEPTIONAL
                    | HUFFDEC_SUITABLE_POINTER
                    | (subtable_bits as u32) << 8
                    | table_bits as u32;

                curr_table_end = subtable_start + (1 << subtable_bits);
            }

            /* Fill the sub-table entries for the current codeword. */
            let entry = make_decode_table_entry(
                decode_results,
                sorted_syms[i] as usize,
                (len - table_bits) as u32
            );

            i += 1;

            let stride = 1 << (len - table_bits);

            let mut j = subtable_start + (codeword >> table_bits);

            while j < curr_table_end
            {
                decode_table[j] = entry;
                j += stride;
            }
            //advance to the next codeword
            if codeword == (1 << len) - 1
            {
                // last codeword
                return Ok(());
            }

            let adv = BITS - (codeword ^ ((1 << len) - 1)).leading_zeros();
            let bit = 1 << adv;

            codeword &= bit - 1;
            codeword |= bit;
            count -= 1;

            while count == 0
            {
                len += 1;
                count = len_counts[len];
            }
        }
    }
}

#[test]
fn simple_test()
{
    use std::fs::read;
    let file = read("/home/caleb/Documents/zune-image/zune-inflate/tests/tt.zlib").unwrap();
    let mut decoder = DeflateDecoder::new(&file);
    decoder.decode_zlib().unwrap();
}
