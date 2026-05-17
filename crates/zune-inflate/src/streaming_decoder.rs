use crate::constants::{
    DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN, DEFLATE_BLOCKTYPE_STATIC, DEFLATE_BLOCKTYPE_UNCOMPRESSED,
    DEFLATE_MAX_CODEWORD_LENGTH, DEFLATE_MAX_LITLEN_CODEWORD_LENGTH,
    DEFLATE_MAX_OFFSET_CODEWORD_LENGTH, DEFLATE_MAX_PRE_CODEWORD_LEN, DEFLATE_NUM_LITLEN_SYMS,
    DEFLATE_NUM_OFFSET_SYMS, DEFLATE_NUM_PRECODE_SYMS, DEFLATE_PRECODE_LENS_PERMUTATION,
    DELFATE_MAX_LENS_OVERRUN, FASTCOPY_BYTES, FASTLOOP_MAX_BYTES_WRITTEN, HUFFDEC_END_OF_BLOCK,
    HUFFDEC_EXCEPTIONAL, HUFFDEC_LITERAL, HUFFDEC_SUITABLE_POINTER, LITLEN_DECODE_BITS,
    LITLEN_DECODE_RESULTS, LITLEN_ENOUGH, LITLEN_TABLE_BITS, OFFSET_DECODE_RESULTS, OFFSET_ENOUGH,
    OFFSET_TABLEBITS, PRECODE_DECODE_RESULTS, PRECODE_ENOUGH, PRECODE_TABLE_BITS,
};
use crate::decoder::{build_decode_table_inner, DeflateHeaderTables};
use crate::errors::{DecodeErrorStatus, InflateDecodeErrors};
use crate::streaming_decoder::streaming_bitstream::StreamingBitStreamReader;
use crate::utils::{copy_rep_matches_slow, fixed_copy_within};

mod streaming_bitstream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum DynamicTreePhase {
    /// Reading HLIT, HDIST, and HCLEN
    #[default]
    ReadSizes,
    /// Reading the 3-bit lengths for the pre-code tree
    ReadPrecodeLens {
        num_explicit: usize,
        index: usize,
        num_litlen: usize,
        num_offset: usize,
    },
    /// Reading the actual code lengths using the pre-code tree
    ReadCodeLens {
        num_litlen: usize,
        num_offset: usize,
        index: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DeflateState {
    /// Waiting to read the 3-bit block header (BFINAL and BTYPE)
    #[default]
    ReadingBlockHeader,
    /// Waiting to read LEN and NLEN for an uncompressed block
    ReadingUncompressedHeader,
    /// Currently copying raw bytes for an uncompressed block
    CopyingUncompressedData { bytes_left: usize },
    /// Waiting to read the dynamic Huffman table headers and code lengths
    ReadingDynamicTree(DynamicTreePhase),
    /// In the main Huffman decoding loop
    DecodingData,
    /// Block is completely done
    Done,
}
/// A streaming deflate decoder instance.
///
/// The decoder allows one to incrementally increase both input and output buffers
/// as more data comes in, useful for streaming scenarios like PNG IDAT decoding
#[derive(Default)]
pub struct StreamingDecoder {
    is_last_block: bool,
    static_codes_loaded: bool,
    deflate_header_tables: DeflateHeaderTables,
    state: DeflateState,
    stream: StreamingBitStreamReader,
    dest_offset: usize,
    // We need to store this across calls if we get interrupted reading tables
    is_final_chunk: bool,
    // max bytes to be read
    max_bytes: usize,
    // Keep track of bytes shifted out of the buffer to track absolute total
    window_slid_bytes: usize,
}
impl StreamingDecoder {
    #[must_use]
    pub fn new() -> Self {
        StreamingDecoder {
            max_bytes: usize::MAX,
            ..Default::default()
        }
    }
    /// Set maximum bytes the decoder should decode
    pub fn set_limit(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes;
    }
}

#[derive(Debug)]
pub enum DecodeStatus {
    /// Input back reference was too small , increase it
    InputBackReferenceTooSmall { expected: usize, actual: usize },
    /// The decoder ran out of output bytes and the caller should provide a new chunk
    NeedsMoreOutput { at_least: usize },
    /// The decoder ran out of input bytes and needs the next chunk
    NeedsMoreInput,
    /// The decoder successfully finished the entire Deflate stream
    Finished,
    /// An error occurred during decoding
    Error(InflateDecodeErrors),
}
impl StreamingDecoder {
    fn build_decode_table(
        &mut self, block_type: u64, chunk: &[u8],
    ) -> Result<(), DecodeErrorStatus> {
        const COUNT: usize =
            DEFLATE_NUM_LITLEN_SYMS + DEFLATE_NUM_OFFSET_SYMS + DELFATE_MAX_LENS_OVERRUN;

        let mut lens = [0_u8; COUNT];
        let mut precode_lens = [0; DEFLATE_NUM_PRECODE_SYMS];
        let mut precode_decode_table = [0_u32; PRECODE_ENOUGH];
        let mut litlen_decode_table = [0_u32; LITLEN_ENOUGH];
        let mut offset_decode_table = [0; OFFSET_ENOUGH];

        let mut num_litlen_syms = 0;
        let mut num_offset_syms = 0;

        if block_type == DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN {
            const SINGLE_PRECODE: usize = 3;

            self.static_codes_loaded = false;

            // Dynamic Huffman block
            // Read codeword lengths
            if !self.stream.has(5 + 5 + 4) {
                return Err(DecodeErrorStatus::InsufficientData);
            }

            num_litlen_syms = 257 + (self.stream.get_bits(5)) as usize;
            num_offset_syms = 1 + (self.stream.get_bits(5)) as usize;

            let num_explicit_precode_lens = 4 + (self.stream.get_bits(4)) as usize;

            self.stream.refill(chunk);

            if !self.stream.has(3) {
                return Err(DecodeErrorStatus::InsufficientData);
            }

            let first_precode = self.stream.get_bits(3) as u8;
            let expected = (SINGLE_PRECODE * num_explicit_precode_lens.saturating_sub(1)) as u8;

            precode_lens[usize::from(DEFLATE_PRECODE_LENS_PERMUTATION[0])] = first_precode;

            self.stream.refill(chunk);

            if !self.stream.has(expected) {
                return Err(DecodeErrorStatus::InsufficientData);
            }

            for i in DEFLATE_PRECODE_LENS_PERMUTATION[1..]
                .iter()
                .take(num_explicit_precode_lens - 1)
            {
                let bits = self.stream.get_bits(3) as u8;

                precode_lens[usize::from(*i)] = bits;
            }

            build_decode_table_inner(
                &precode_lens,
                &PRECODE_DECODE_RESULTS,
                &mut precode_decode_table,
                PRECODE_TABLE_BITS,
                DEFLATE_NUM_PRECODE_SYMS,
                DEFLATE_MAX_CODEWORD_LENGTH,
            )?;

            /* Decode the litlen and offset codeword lengths. */

            let mut i = 0;

            loop {
                if i >= num_litlen_syms + num_offset_syms {
                    // confirm here since with a continue loop stuff
                    // breaks
                    break;
                }

                let rep_val: u8;
                let rep_count: u64;

                if !self.stream.has(DEFLATE_MAX_PRE_CODEWORD_LEN + 7) {
                    self.stream.refill(chunk);
                }
                // decode next pre-code symbol
                let entry_pos = self
                    .stream
                    .peek_bits::<{ DEFLATE_MAX_PRE_CODEWORD_LEN as usize }>();

                let entry = precode_decode_table[entry_pos];
                let presym = entry >> 16;

                if !self.stream.has(entry as u8) {
                    return Err(DecodeErrorStatus::InsufficientData);
                }

                self.stream.drop_bits(entry as u8);

                if presym < 16 {
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
                if presym == 16 {
                    if i == 0 {
                        return Err(DecodeErrorStatus::CorruptData);
                    }

                    if !self.stream.has(2) {
                        return Err(DecodeErrorStatus::InsufficientData);
                    }

                    // repeat previous length three to 6 times
                    rep_val = lens[i - 1];
                    rep_count = 3 + self.stream.get_bits(2);
                    lens[i..i + 6].fill(rep_val);
                    i += rep_count as usize;
                } else if presym == 17 {
                    if !self.stream.has(3) {
                        return Err(DecodeErrorStatus::InsufficientData);
                    }
                    /* Repeat zero 3 - 10 times. */
                    rep_count = 3 + self.stream.get_bits(3);
                    lens[i..i + 10].fill(0);
                    i += rep_count as usize;
                } else {
                    if !self.stream.has(7) {
                        return Err(DecodeErrorStatus::InsufficientData);
                    }
                    // repeat zero 11-138 times.
                    rep_count = 11 + self.stream.get_bits(7);
                    lens[i..i + rep_count as usize].fill(0);
                    i += rep_count as usize;
                }

                if i >= num_litlen_syms + num_offset_syms {
                    break;
                }
            }
        } else if block_type == DEFLATE_BLOCKTYPE_STATIC {
            if self.static_codes_loaded {
                return Ok(());
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
        build_decode_table_inner(
            &lens[num_litlen_syms..],
            &OFFSET_DECODE_RESULTS,
            &mut offset_decode_table,
            OFFSET_TABLEBITS,
            num_offset_syms,
            DEFLATE_MAX_OFFSET_CODEWORD_LENGTH,
        )?;

        build_decode_table_inner(
            &lens,
            &LITLEN_DECODE_RESULTS,
            &mut litlen_decode_table,
            LITLEN_TABLE_BITS,
            num_litlen_syms,
            DEFLATE_MAX_LITLEN_CODEWORD_LENGTH,
        )?;

        self.deflate_header_tables.offset_decode_table = offset_decode_table;
        self.deflate_header_tables.litlen_decode_table = litlen_decode_table;

        Ok(())
    }
}
impl StreamingDecoder {
    /// Current offset in the deflate buffer the dest is pointing to
    pub fn current_dest_offset(&self) -> usize {
        self.dest_offset
    }
    /// Return the total decoded bytes in this context
    pub fn decoded_bytes(&self) -> usize {
        self.window_slid_bytes + self.dest_offset
    }
    /// A streaming Deflate decoder that processes compressed data in arbitrary-sized chunks.
    ///
    /// # Overview
    ///
    /// Unlike a one-shot decoder, `DeflateDecoder` does not require the entire compressed
    /// stream to be available upfront. Instead, the caller feeds compressed data piece by
    /// piece via [`decode_chunk`], growing the output buffer on demand and supplying more
    /// input as requested. The decoder suspends and resumes transparently across chunk
    /// boundaries — including mid-codeword, mid-match, and mid-block-header boundaries.
    ///
    /// # State machine
    ///
    /// Internally the decoder is a state machine that advances through these phases:
    ///
    /// ```text
    /// ReadingBlockHeader
    ///     │
    ///     ├─ store  ──► ReadingUncompressedHeader ──► CopyingUncompressedData ──┐
    ///     ├─ static ──► DecodingData                                            │
    ///     └─ dynamic ─► ReadingDynamicTree ──────────► DecodingData             │
    ///                       │                               │                   │
    ///                       │        (more blocks)          ▼                   │
    ///                       └───────────────────── ReadingBlockHeader ◄─────────┘
    ///                                                       │
    ///                                                     Done
    /// ```
    ///
    /// The state is preserved across calls, so the caller never needs to track it.
    ///
    /// # Usage
    ///
    /// ```rust
    /// let options = DeflateOptions::default().set_limit(decompressed_size_limit);
    /// let mut decoder = DeflateDecoder::new_with_options(&[], options);
    /// let mut output = vec![0u8; initial_capacity];
    /// let mut input_pos = 0;
    ///
    /// loop {
    ///     let end = (input_pos + chunk_size).min(compressed.len());
    ///     let chunk = &compressed[input_pos..end];
    ///     let is_final = end == compressed.len();
    ///
    ///     match decoder.decode_chunk(chunk, is_final, &mut output) {
    ///         DecodeStatus::NeedsMoreInput => {
    ///             // Current chunk fully consumed. Advance and feed the next one.
    ///             // Only returned when `is_final` is false.
    ///             input_pos = end;
    ///         }
    ///         DecodeStatus::NeedsMoreOutput { at_least } => {
    ///             // Output buffer is full. Grow it, then call decode_chunk again
    ///             // with the SAME chunk and the SAME input position — the decoder
    ///             // has already rolled back its bitstream state and will re-process
    ///             // the current chunk from where it left off.
    ///             output.resize(output.len() + at_least, 0);
    ///             // do NOT advance input_pos here
    ///         }
    ///         DecodeStatus::Finished => {
    ///             // Stream fully decoded. `decoder.decode_dest()` bytes are valid
    ///             // in `output[..decoder.decode_dest()]`.
    ///             break;
    ///         }
    ///         DecodeStatus::Error(e) => {
    ///             // Unrecoverable error. The decoder must be discarded.
    ///             return Err(e);
    ///         }
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// ```
    ///
    /// # Chunk sizing
    ///
    /// Any chunk size from 1 byte upward is valid. Smaller chunks increase the number
    /// of [`NeedsMoreInput`] round-trips but do not affect correctness. There is no
    /// requirement for chunks to align to block or byte boundaries in the compressed
    /// stream.
    ///
    /// # Output buffer
    ///
    /// `out_block` is a flat `&mut [u8]` that the decoder writes into sequentially
    /// starting at offset [`decode_dest`]. The caller is responsible for allocation;
    /// the decoder never reallocates. On [`NeedsMoreOutput`], the caller must grow the
    /// buffer (e.g. with `Vec::resize`) before retrying — the decoder guarantees it will
    /// not advance past the end of the slice.
    ///
    /// Bytes in `out_block[..decode_dest()]` are always valid decompressed data, even
    /// if decoding is not yet complete.
    ///
    /// # Resumption contract
    ///
    /// | Return value        | Advance input? | Grow output? | Notes                          |
    /// |---------------------|---------------|--------------|--------------------------------|
    /// | `NeedsMoreInput`    | Yes           | No           | Current chunk fully consumed   |
    /// | `NeedsMoreOutput`   | No            | Yes          | Retry with same chunk          |
    /// | `Finished`          | —             | —            | Decoder is done                |
    /// | `Error`             | —             | —            | Decoder must be discarded      |
    ///
    /// # `is_final_chunk`
    ///
    /// Set `is_final_chunk = true` on the last chunk of the compressed stream. The
    /// decoder uses this flag to distinguish "not enough input yet" (a normal condition
    /// mid-stream) from a truncated or corrupt stream. Passing `is_final_chunk = true`
    /// on a non-final chunk will cause the decoder to treat missing input as corruption
    /// and return [`DecodeStatus::Error`]. Passing `is_final_chunk = false` on the
    /// genuinely final chunk will cause the decoder to return [`NeedsMoreInput`] instead
    /// of [`Finished`] after consuming all input.
    ///
    /// # Output limit
    ///
    /// The decoder enforces the limit set in [`DeflateOptions`]. If the decompressed
    /// output would exceed it, [`DecodeStatus::Error`] is returned immediately. This
    /// is a defence against zip-bomb inputs where a small compressed stream expands to
    /// an enormous output.
    ///
    /// # Errors
    ///
    /// | Condition                                      | Error variant                  |
    /// |------------------------------------------------|-------------------------------|
    /// | Reserved Deflate block type (`0b11`)           | `Generic`                      |
    /// | LEN/NLEN mismatch in uncompressed block        | `Generic`                      |
    /// | Back-reference distance exceeds output so far  | `InputBackReferenceTooSmall`   |
    /// | Output would exceed configured limit           | `OutputLimitExceeded`          |
    /// | Overread of zero-padded stream (corrupt data)  | `CorruptData`                  |
    /// | Invalid Huffman code lengths                   | propagated from table builder  |
    pub fn decode_chunk(
        &mut self, chunk: &[u8], is_final_chunk: bool, out_block: &mut [u8],
    ) -> DecodeStatus {
        // Update the bit reader with the new slice
        self.stream.update_chunk(is_final_chunk);
        self.is_final_chunk = is_final_chunk;

        loop {
            if self.state == DeflateState::Done {
                return DecodeStatus::Finished;
            }
            // Attempt to refill at the start of every state cycle
            self.stream.refill(chunk);
            // Calculate the max allowed offset for the current out_block slice
            let max_dest_offset = self.max_bytes.saturating_sub(self.window_slid_bytes);
            match self.state {
                DeflateState::ReadingBlockHeader => {
                    if !self.stream.has(3) {
                        return DecodeStatus::NeedsMoreInput;
                    }

                    self.is_last_block = self.stream.get_bits(1) == 1;
                    let block_type = self.stream.get_bits(2);

                    self.state = match block_type {
                        DEFLATE_BLOCKTYPE_UNCOMPRESSED => DeflateState::ReadingUncompressedHeader,

                        DEFLATE_BLOCKTYPE_STATIC => {
                            // build_decode_table for static is fast/synchronous
                            if let Err(e) = self.build_decode_table(block_type, chunk) {
                                return DecodeStatus::Error(InflateDecodeErrors::new(e, out_block));
                            }
                            DeflateState::DecodingData
                        }

                        DEFLATE_BLOCKTYPE_DYNAMIC_HUFFMAN => {
                            DeflateState::ReadingDynamicTree(DynamicTreePhase::ReadSizes)
                        }

                        _ => {
                            return DecodeStatus::Error(InflateDecodeErrors::new(
                                DecodeErrorStatus::Generic("Reserved block type 0b11 encountered"),
                                out_block,
                            ))
                        }
                    };
                }

                DeflateState::ReadingUncompressedHeader => {
                    // 1. Calculate how many bits we need to drop to reach a byte boundary.
                    let partial_bits = self.stream.get_bits_left() & 7;

                    // We need: partial_bits to align + 16 bits for LEN + 16 bits for NLEN.
                    if !self.stream.has(partial_bits + 32) {
                        return DecodeStatus::NeedsMoreInput;
                    }

                    self.stream.drop_bits(partial_bits);

                    let len = self.stream.get_bits(16) as u16;
                    let nlen = self.stream.get_bits(16) as u16;

                    if len != !nlen {
                        return DecodeStatus::Error(InflateDecodeErrors::new(
                            DecodeErrorStatus::Generic("Len and nlen do not match"),
                            out_block,
                        ));
                    }

                    // Transition to copying the data, passing the length along
                    self.state = DeflateState::CopyingUncompressedData {
                        bytes_left: len as usize,
                    };
                }

                DeflateState::CopyingUncompressedData { ref mut bytes_left } => {
                    if *bytes_left == 0 {
                        self.state = if self.is_last_block {
                            DeflateState::Done
                        } else {
                            DeflateState::ReadingBlockHeader
                        };
                        continue;
                    }
                    let total_written = self.dest_offset + self.window_slid_bytes;

                    let global_remaining = max_dest_offset.saturating_sub(total_written);

                    // If fulfilling this block breaches the limit, error out immediately.
                    // This prevents allocating or copying data for malformed payloads or bombs.
                    if *bytes_left > global_remaining {
                        return DecodeStatus::Error(InflateDecodeErrors::new_with_error(
                            DecodeErrorStatus::OutputLimitExceeded(
                                self.max_bytes,
                                total_written + *bytes_left, // The total size it attempted to reach
                            ),
                        ));
                    }

                    let available_out = out_block.len().saturating_sub( self.dest_offset);

                    // If the output buffer is full, we must pause and yield to the caller
                    if available_out == 0 {
                        return DecodeStatus::NeedsMoreOutput { at_least: 1 };
                    }

                    // Step 1: Drain any bytes trapped in the bit reader's `buffer`
                    assert_eq!(self.stream.get_bits_left() % 8, 0);
                    while *bytes_left > 0
                        && self.stream.get_bits_left() >= 8
                        && self.dest_offset < out_block.len()
                    {
                        let byte = self.stream.get_bits(8) as u8;
                        out_block[self.dest_offset] = byte;
                        self.dest_offset += 1;
                        *bytes_left -= 1;
                    }

                    if *bytes_left == 0 {
                        continue; // Loop around to check if we are Done or need the next BlockHeader
                    }

                    // If we stopped draining because the output buffer filled up, yield.
                    if self.dest_offset == out_block.len() {
                        return DecodeStatus::NeedsMoreOutput {
                            at_least: *bytes_left,
                        };
                    }

                    // Step 2: The bit buffer is empty. Fast bulk copy directly from the slice.
                    let remaining_in_chunk = self.stream.remaining_bytes(chunk);
                    let available_out_for_bulk = out_block.len() - self.dest_offset;

                    // Copy the minimum of what is remaining across all 3 constraints
                    let bytes_to_copy = (*bytes_left)
                        .min(remaining_in_chunk)
                        .min(available_out_for_bulk);

                    if bytes_to_copy > 0 {
                        let start = self.stream.position;
                        let end = start + bytes_to_copy;

                        out_block[self.dest_offset..self.dest_offset + bytes_to_copy]
                            .copy_from_slice(&chunk[start..end]);

                        self.stream.position += bytes_to_copy;
                        *bytes_left -= bytes_to_copy;
                        self.dest_offset += bytes_to_copy;
                    }

                    if *bytes_left == 0 {
                        self.state = if self.is_last_block {
                            DeflateState::Done
                        } else {
                            DeflateState::ReadingBlockHeader
                        };
                    } else if self.dest_offset == out_block.len() {
                        return DecodeStatus::NeedsMoreOutput {
                            at_least: *bytes_left,
                        };
                    } else {
                        return DecodeStatus::NeedsMoreInput;
                    }
                }

                DeflateState::ReadingDynamicTree(ref mut phase) => {
                    match *phase {
                        DynamicTreePhase::ReadSizes => {
                            // 5 bits (HLIT) + 5 bits (HDIST) + 4 bits (HCLEN) = 14 bits
                            if !self.stream.has(14) {
                                return DecodeStatus::NeedsMoreInput;
                            }

                            let num_litlen = 257 + self.stream.get_bits(5) as usize;
                            let num_offset = 1 + self.stream.get_bits(5) as usize;
                            let num_explicit = 4 + self.stream.get_bits(4) as usize;

                            // Reset our temporary arrays
                            self.deflate_header_tables.precode_lens.fill(0);
                            self.deflate_header_tables.lens.fill(0);

                            self.state = DeflateState::ReadingDynamicTree(
                                DynamicTreePhase::ReadPrecodeLens {
                                    num_explicit,
                                    index: 0,
                                    num_litlen,
                                    num_offset,
                                },
                            );
                        }

                        DynamicTreePhase::ReadPrecodeLens {
                            num_explicit,
                            mut index,
                            num_litlen,
                            num_offset,
                        } => {
                            while index < num_explicit {
                                self.stream.refill(chunk);

                                if !self.stream.has(3) {
                                    // Save progress and yield
                                    self.state = DeflateState::ReadingDynamicTree(
                                        DynamicTreePhase::ReadPrecodeLens {
                                            num_explicit,
                                            index,
                                            num_litlen,
                                            num_offset,
                                        },
                                    );
                                    return DecodeStatus::NeedsMoreInput;
                                }

                                let bits = self.stream.get_bits(3) as u8;
                                let perm_index =
                                    usize::from(DEFLATE_PRECODE_LENS_PERMUTATION[index]);

                                self.deflate_header_tables.precode_lens[perm_index] = bits;
                                index += 1;
                            }

                            // We have all precode lengths, build the precode table
                            if let Err(e) = build_decode_table_inner(
                                &self.deflate_header_tables.precode_lens,
                                &PRECODE_DECODE_RESULTS,
                                &mut self.deflate_header_tables.precode_decode_table,
                                PRECODE_TABLE_BITS,
                                DEFLATE_NUM_PRECODE_SYMS,
                                DEFLATE_MAX_CODEWORD_LENGTH,
                            ) {
                                return DecodeStatus::Error(InflateDecodeErrors::new(e, out_block));
                            }

                            self.state =
                                DeflateState::ReadingDynamicTree(DynamicTreePhase::ReadCodeLens {
                                    num_litlen,
                                    num_offset,
                                    index: 0,
                                });
                        }

                        DynamicTreePhase::ReadCodeLens {
                            num_litlen,
                            num_offset,
                            mut index,
                        } => {
                            let total_syms = num_litlen + num_offset;

                            while index < total_syms {
                                self.stream.refill(chunk);
                                // Max bits needed: 7 (max precode len) + 7 (max extra bits for code 18) = 14
                                if !self.stream.has(14) {
                                    self.state = DeflateState::ReadingDynamicTree(
                                        DynamicTreePhase::ReadCodeLens {
                                            num_litlen,
                                            num_offset,
                                            index,
                                        },
                                    );
                                    return DecodeStatus::NeedsMoreInput;
                                }

                                let entry_pos = self
                                    .stream
                                    .peek_bits::<{ DEFLATE_MAX_PRE_CODEWORD_LEN as usize }>();
                                let entry =
                                    self.deflate_header_tables.precode_decode_table[entry_pos];
                                let presym = entry >> 16;

                                self.stream.drop_bits((entry & 0xFF) as u8);

                                if presym < 16 {
                                    self.deflate_header_tables.lens[index] = presym as u8;
                                    index += 1;
                                } else if presym == 16 {
                                    if index == 0 {
                                        return DecodeStatus::Error(InflateDecodeErrors::new(
                                            DecodeErrorStatus::CorruptData,
                                            out_block,
                                        ));
                                    }
                                    let rep_val = self.deflate_header_tables.lens[index - 1];
                                    let rep_count = 3 + self.stream.get_bits(2) as usize;
                                    self.deflate_header_tables.lens[index..index + rep_count]
                                        .fill(rep_val);
                                    index += rep_count;
                                } else if presym == 17 {
                                    let rep_count = 3 + self.stream.get_bits(3) as usize;
                                    self.deflate_header_tables.lens[index..index + rep_count]
                                        .fill(0);
                                    index += rep_count;
                                } else {
                                    // presym == 18
                                    let rep_count = 11 + self.stream.get_bits(7) as usize;
                                    self.deflate_header_tables.lens[index..index + rep_count]
                                        .fill(0);
                                    index += rep_count;
                                }
                            }

                            // We have all the lengths! Build the final dynamic offset and litlen tables.
                            if let Err(e) = build_decode_table_inner(
                                &self.deflate_header_tables.lens[num_litlen..],
                                &OFFSET_DECODE_RESULTS,
                                &mut self.deflate_header_tables.offset_decode_table,
                                OFFSET_TABLEBITS,
                                num_offset,
                                DEFLATE_MAX_OFFSET_CODEWORD_LENGTH,
                            ) {
                                return DecodeStatus::Error(InflateDecodeErrors::new(e, out_block));
                            }

                            if let Err(e) = build_decode_table_inner(
                                &self.deflate_header_tables.lens,
                                &LITLEN_DECODE_RESULTS,
                                &mut self.deflate_header_tables.litlen_decode_table,
                                LITLEN_TABLE_BITS,
                                num_litlen,
                                DEFLATE_MAX_LITLEN_CODEWORD_LENGTH,
                            ) {
                                return DecodeStatus::Error(InflateDecodeErrors::new(e, out_block));
                            }

                            // Tables built successfully! Move to decoding actual data.
                            self.static_codes_loaded = false;
                            self.state = DeflateState::DecodingData;
                        }
                    }
                }

                DeflateState::DecodingData => {
                    if let Err(status) = self.decode_data(chunk, out_block) {
                        return status;
                    }
                    continue;
                }

                DeflateState::Done => {
                    return DecodeStatus::Finished;
                }
            }
        }
    }
}

impl StreamingDecoder {
    /// Tells the decoder that the first `amount` bytes of the output buffer
    /// were discarded, and the remaining data was shifted to index 0.
    pub fn slide_window(&mut self, amount: usize) {
        self.dest_offset -= amount;
        self.window_slid_bytes += amount;
    }
    pub(crate) fn decode_data(
        &mut self, chunk: &[u8], out_block: &mut [u8],
    ) -> Result<(), DecodeStatus> {
        let litlen_decode_table = &self.deflate_header_tables.litlen_decode_table;
        let offset_decode_table = &self.deflate_header_tables.offset_decode_table;

        let mut literal: u32;
        let mut length: usize;
        let mut offset: usize;
        let mut entry: u32;
        let mut saved_bitbuf: u64;

        'decode: loop {
            // --- THE FAST LOOP ---
            // Ensure we have enough input bytes AND enough output space to safely perform sloppy copies
            let close_src = self.stream.remaining_bytes(chunk) > 3 * FASTCOPY_BYTES;
            let close_dest = self.dest_offset + FASTLOOP_MAX_BYTES_WRITTEN <= out_block.len();

            if close_src && close_dest {
                self.stream.refill(chunk);

                let lit_mask = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                entry = litlen_decode_table[lit_mask];

                'sequence: loop {
                    // Check if we are approaching the end of the input chunk or output slice
                    if self.stream.remaining_bytes(chunk) < 8
                        || self.dest_offset + FASTLOOP_MAX_BYTES_WRITTEN > out_block.len()
                    {
                        break 'sequence;
                    }

                    self.stream.refill(chunk);
                    saved_bitbuf = self.stream.buffer;
                    self.stream.drop_bits((entry & 0xFF) as u8);
                    if let Some(out) = out_block.get_mut(self.dest_offset..self.dest_offset + 3) {
                        // --- FAST LITERAL 1 ---
                        if (entry & HUFFDEC_LITERAL) != 0 {
                            literal = entry >> 16;
                            let new_pos = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                            entry = litlen_decode_table[new_pos];
                            saved_bitbuf = self.stream.buffer;
                            self.stream.drop_bits(entry as u8);

                            out[0] = literal as u8;
                            self.dest_offset += 1;

                            // --- FAST LITERAL 2 ---
                            if (entry & HUFFDEC_LITERAL) != 0 {
                                literal = entry >> 16;
                                let new_pos = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                                entry = litlen_decode_table[new_pos];
                                saved_bitbuf = self.stream.buffer;
                                self.stream.drop_bits(entry as u8);

                                out[1] = literal as u8;
                                self.dest_offset += 1;

                                // --- FAST LITERAL 3 ---
                                if (entry & HUFFDEC_LITERAL) != 0 {
                                    literal = entry >> 16;
                                    let new_pos = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                                    entry = litlen_decode_table[new_pos];

                                    out[2] = literal as u8;
                                    self.dest_offset += 1;

                                    continue;
                                }
                            }
                        }
                    }

                    // --- EXCEPTIONAL (Subtable or EOB) ---
                    if (entry & HUFFDEC_EXCEPTIONAL) != 0 {
                        if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
                            break 'decode;
                        }

                        let entry_position = ((entry >> 8) & 0x3F) as usize;
                        let mut pos = (entry >> 16) as usize;

                        saved_bitbuf = self.stream.buffer;
                        pos += self.stream.peek_var_bits(entry_position);
                        entry = litlen_decode_table[pos.min(LITLEN_ENOUGH - 1)];
                        self.stream.drop_bits(entry as u8);

                        if (entry & HUFFDEC_LITERAL) != 0 {
                            let new_pos = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                            literal = entry >> 16;
                            entry = litlen_decode_table[new_pos];

                            if let Some(out) = out_block.get_mut(self.dest_offset) {
                                *out = (literal & 0xFF) as u8;
                            }

                            self.dest_offset += 1;
                            continue;
                        }

                        if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
                            break 'decode;
                        }
                    }

                    // --- FAST MATCH COPY ---
                    let entry_dup = entry;
                    entry = offset_decode_table[self.stream.peek_bits::<{ OFFSET_TABLEBITS }>()];
                    length = (entry_dup >> 16) as usize;
                    let mask = (1 << entry_dup as u8) - 1;
                    length += (saved_bitbuf & mask) as usize >> ((entry_dup >> 8) as u8);

                    if (entry & HUFFDEC_EXCEPTIONAL) != 0 {
                        self.stream.drop_bits(OFFSET_TABLEBITS as u8);
                        let extra = self.stream.peek_var_bits(((entry >> 8) & 0x3F) as usize);
                        entry = offset_decode_table[((entry >> 16) as usize + extra) & 511];
                    }

                    saved_bitbuf = self.stream.buffer;
                    self.stream.drop_bits((entry & 0xFF) as u8);

                    let mask = (1 << entry as u8) - 1;
                    offset = (entry >> 16) as usize;
                    offset += (saved_bitbuf & mask) as usize >> (((entry >> 8) & 0xFF) as u8);

                    if offset > self.dest_offset {
                        return Err(DecodeStatus::InputBackReferenceTooSmall {
                            expected: offset,
                            actual: self.dest_offset,
                        });
                    }

                    let src_offset = self.dest_offset - offset;

                    if self.stream.bits_left < 11 {
                        self.stream.refill(chunk);
                    }

                    fixed_copy_within::<{ FASTCOPY_BYTES }>(
                        out_block,
                        src_offset,
                        self.dest_offset,
                    );

                    entry = litlen_decode_table[self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>()];
                    let mut current_position = self.dest_offset;
                    self.dest_offset += length;

                    if offset == 1 {
                        let byte_to_repeat = out_block[src_offset];
                        out_block[current_position..self.dest_offset].fill(byte_to_repeat);
                    } else if offset <= FASTCOPY_BYTES
                        && current_position + offset < self.dest_offset
                    {
                        let mut src_position = src_offset + offset;
                        let mut dest_position = current_position + offset;
                        loop {
                            fixed_copy_within::<{ FASTCOPY_BYTES }>(
                                out_block,
                                src_position,
                                dest_position,
                            );
                            src_position += offset;
                            dest_position += offset;
                            if dest_position > self.dest_offset {
                                break;
                            }
                        }
                    } else if length > FASTCOPY_BYTES {
                        current_position += FASTCOPY_BYTES;
                        let mut dest_src_offset = src_offset + FASTCOPY_BYTES;
                        'match_lengths: loop {
                            fixed_copy_within::<{ FASTCOPY_BYTES }>(
                                out_block,
                                dest_src_offset,
                                current_position,
                            );
                            dest_src_offset += FASTCOPY_BYTES;
                            current_position += FASTCOPY_BYTES;
                            if current_position > self.dest_offset {
                                break 'match_lengths;
                            }
                        }
                    }
                }
            }

            // --- THE SLOW LOOP (Safe boundary parsing) ---
            loop {
                if self.stream.over_read > 10 {
                    return Err(DecodeStatus::Error(InflateDecodeErrors::new_with_error(DecodeErrorStatus::Generic("Stream filled with more than accepted number of fill bytes, this is a corrupt block"))));
                }
                let total_decompressed = self.window_slid_bytes + self.dest_offset;
                if total_decompressed > self.max_bytes {
                    return Err(DecodeStatus::Error(InflateDecodeErrors::new_with_error(
                        DecodeErrorStatus::OutputLimitExceeded(self.max_bytes, total_decompressed),
                    )));
                }
                // Take a zero-cost snapshot of the bitstream BEFORE we consume any codes
                let snapshot_pos = self.stream.position;
                let snapshot_bits = self.stream.bits_left;
                let snapshot_buf = self.stream.buffer;
                let snapshot_over = self.stream.over_read;

                self.stream.refill(chunk);

                // Yield if we are out of bits and it is not the final chunk
                if self.stream.get_bits_left() < 48
                    && self.stream.remaining_bytes(chunk) == 0
                    && !self.is_final_chunk
                {
                    return Err(DecodeStatus::NeedsMoreInput);
                }

                let literal_mask = self.stream.peek_bits::<{ LITLEN_DECODE_BITS }>();
                entry = litlen_decode_table[literal_mask];

                saved_bitbuf = self.stream.buffer;
                self.stream.drop_bits((entry & 0xFF) as u8);

                if (entry & HUFFDEC_SUITABLE_POINTER) != 0 {
                    let extra = self.stream.peek_var_bits(((entry >> 8) & 0x3F) as usize);
                    entry = litlen_decode_table[(entry >> 16) as usize + extra];
                    saved_bitbuf = self.stream.buffer;
                    self.stream.drop_bits((entry & 0xFF) as u8);
                }

                length = (entry >> 16) as usize;

                if (entry & HUFFDEC_LITERAL) != 0 {
                    // Check slice bounds
                    if self.dest_offset >= out_block.len() {
                        // ROLLBACK: We don't have space, un-drop the literal bits!
                        self.stream.position = snapshot_pos;
                        self.stream.bits_left = snapshot_bits;
                        self.stream.buffer = snapshot_buf;
                        self.stream.over_read = snapshot_over;
                        return Err(DecodeStatus::NeedsMoreOutput { at_least: 1 });
                    }
                    out_block[self.dest_offset] = length as u8;
                    self.dest_offset += 1;
                    break;
                }

                if (entry & HUFFDEC_END_OF_BLOCK) != 0 {
                    break 'decode;
                }

                let mask = (1 << entry as u8) - 1;
                length += (saved_bitbuf & mask) as usize >> ((entry >> 8) as u8);

                self.stream.refill(chunk);
                entry = offset_decode_table[self.stream.peek_bits::<{ OFFSET_TABLEBITS }>()];

                if (entry & HUFFDEC_EXCEPTIONAL) != 0 {
                    self.stream.drop_bits(OFFSET_TABLEBITS as u8);
                    let extra = self.stream.peek_var_bits(((entry >> 8) & 0x3F) as usize);
                    entry = offset_decode_table[((entry >> 16) as usize + extra) & 511];
                }

                saved_bitbuf = self.stream.buffer;

                let mask = (1 << (entry & 0xFF) as u8) - 1;

                offset = (entry >> 16) as usize;
                offset += (saved_bitbuf & mask) as usize >> ((entry >> 8) as u8);

                if offset > self.dest_offset {
                    return Err(DecodeStatus::InputBackReferenceTooSmall {
                        expected: offset,
                        actual: self.dest_offset,
                    });
                }

                let src_offset = self.dest_offset - offset;
                self.stream.drop_bits(entry as u8);

                if self.dest_offset + length > out_block.len() {
                    // ROLLBACK: We don't have space, un-drop the match bits!
                    self.stream.position = snapshot_pos;
                    self.stream.bits_left = snapshot_bits;
                    self.stream.buffer = snapshot_buf;
                    self.stream.over_read = snapshot_over;

                    let diff = self
                        .dest_offset
                        .wrapping_add(length)
                        .wrapping_sub(out_block.len());

                    return Err(DecodeStatus::NeedsMoreOutput { at_least: diff });
                }

                let (dest_src, dest_ptr) = out_block.split_at_mut(self.dest_offset);

                if src_offset + length > self.dest_offset {
                    copy_rep_matches_slow(out_block, src_offset, self.dest_offset, length);
                } else {
                    dest_ptr[0..length].copy_from_slice(&dest_src[src_offset..src_offset + length]);
                }

                self.dest_offset += length;
            }
        }
        /*
         * If any of the implicit appended zero bytes were consumed (not just
         * refilled) before hitting end of stream, then the data is bad.
         */
        if self.stream.over_read > usize::from(self.stream.bits_left >> 3) {
            let err_msg = DecodeErrorStatus::CorruptData;
            let error = InflateDecodeErrors::new(err_msg, out_block);

            return Err(DecodeStatus::Error(error));
        }

        self.state = if self.is_last_block {
            DeflateState::Done
        } else {
            DeflateState::ReadingBlockHeader
        };

        Ok(())
    }
    pub fn reset_position(&mut self) {
        self.stream.reset_position();
    }
}
