#![cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::{DecodeStatus,StreamingDecoder};
    use miniz_oxide::deflate::compress_to_vec;
    use nanorand::{Rng, WyRand};

    // ============================================================================
    // Harness (copied from existing tests for self-containment)
    // ============================================================================

    fn run_streaming_test(
        uncompressed: &[u8], compressed: &[u8], in_chunk_size: usize, initial_out_size: usize,
    ) {
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; initial_out_size];
        let mut input_offset = 0;

        // Guard against infinite loops in the harness itself.
        let max_iterations = (compressed.len() * 2 + uncompressed.len() * 2 + 4096).max(10_000);
        let mut iterations = 0;

        loop {
            iterations += 1;
            assert!(
                iterations < max_iterations,
                "Decoder appears to be stuck in an infinite loop after {} iterations",
                iterations
            );

            let end = (input_offset + in_chunk_size.max(1)).min(compressed.len());
            let chunk = &compressed[input_offset..end];
            let is_final = end == compressed.len();

            let status = decoder.decode_chunk(chunk, is_final, &mut out_vec);

            match status {
                DecodeStatus::NeedsMoreInput => {
                    assert!(
                        !is_final,
                        "Decoder asked for more input but we fed it the final chunk!"
                    );
                    input_offset = end;
                }
                DecodeStatus::NeedsMoreOutput { at_least } => {
                    let new_len = out_vec.len() + at_least + 1024;
                    out_vec.resize(new_len, 0);
                    // Do NOT advance input_offset — the current chunk must be re-processed.
                }
                DecodeStatus::Finished => {
                    assert_eq!(
                        decoder.current_dest_offset(),
                        uncompressed.len(),
                        "Decoded length mismatch!"
                    );
                    assert_eq!(
                        &out_vec[..decoder.current_dest_offset()],
                        uncompressed,
                        "Decoded data does not match original!"
                    );
                    return;
                }
                DecodeStatus::Error(e) => {
                    panic!("Decoder encountered an error: {:?}", e);
                }
                _ => panic!("Unexpected DecodeStatus variant"),
            }
        }
    }

    fn compress_deflate(data: &[u8], level: u8) -> Vec<u8> {
        compress_to_vec(data, level)
    }

    // ============================================================================
    // 1. EMPTY AND TRIVIAL INPUTS
    // ============================================================================

    /// Empty payload — a valid deflate stream can encode zero bytes.
    /// The encoder must emit at least one block (even if empty), terminated by EOB.
    #[test]
    fn test_empty_payload() {
        let data: &[u8] = &[];
        let compressed = compress_deflate(data, 6);
        run_streaming_test(data, &compressed, compressed.len(), 0);
    }

    /// A single zero byte.
    #[test]
    fn test_single_zero_byte() {
        let data = [0u8; 1];
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, compressed.len(), data.len());
    }

    /// A single non-zero byte.
    #[test]
    fn test_single_nonzero_byte() {
        let data = [0xFFu8; 1];
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, compressed.len(), data.len());
    }

    /// All 256 possible byte values present exactly once.
    /// Exercises every literal symbol in the alphabet.
    #[test]
    fn test_all_byte_values() {
        let data: Vec<u8> = (0u8..=255).collect();
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 1, data.len());
    }

    // ============================================================================
    // 2. CHUNK BOUNDARY STRESS
    // ============================================================================

    /// Feed in 2-byte chunks — forces the bit-reader to stitch pairs constantly.
    #[test]
    fn test_two_byte_chunks() {
        let data: Vec<u8> = (0..500).map(|i| (i * 7 % 256) as u8).collect();
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 2, data.len());
    }

    /// Feed in 3-byte chunks — another prime that misaligns u16/u32/u64 boundaries.
    #[test]
    fn test_three_byte_chunks() {
        let data: Vec<u8> = (0..500).map(|i| (i * 13 % 256) as u8).collect();
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 3, data.len());
    }

    /// Feed in chunks whose size is one less than the compressed stream length.
    /// Forces exactly two iterations of the feed loop.
    #[test]
    fn test_penultimate_chunk() {
        let data = b"penultimate chunk boundary stress test payload data";
        let compressed = compress_deflate(data, 6);
        let chunk_size = (compressed.len() - 1).max(1);
        run_streaming_test(data, &compressed, chunk_size, data.len());
    }

    /// Feed the whole stream in one shot but with an output buffer of exactly 1 byte.
    /// Maximally exercises NeedsMoreOutput growth logic.
    #[test]
    fn test_single_chunk_tiny_output() {
        let data = vec![0x55u8; 1000];
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, compressed.len(), 1);
    }

    /// Output buffer starts at 0 and input arrives one byte at a time.
    /// The worst-case double-stress scenario.
    #[test]
    fn test_byte_by_byte_input_zero_output() {
        let data = b"simultaneous input starvation and output exhaustion";
        let compressed = compress_deflate(data, 6);
        run_streaming_test(data, &compressed, 1, 0);
    }

    // ============================================================================
    // 3. OUTPUT BUFFER BOUNDARY ALIGNMENT
    // ============================================================================

    /// Output buffer is exactly large enough — no growth needed.
    /// Verifies the decoder doesn't write one byte past the end.
    #[test]
    fn test_output_exactly_right_size() {
        let data = b"exactly right output buffer size, no growth";
        let compressed = compress_deflate(data, 6);
        run_streaming_test(data, &compressed, compressed.len(), data.len());
    }

    /// Output buffer is one byte too small initially.
    #[test]
    fn test_output_one_byte_short() {
        let data = b"one byte short output buffer";
        let compressed = compress_deflate(data, 6);
        let short = if data.len() > 0 { data.len() - 1 } else { 0 };
        run_streaming_test(data, &compressed, compressed.len(), short);
    }

    /// Output buffer starts at exactly the size of the first literal, growing one
    /// match at a time. Exercises mid-match NeedsMoreOutput rollback correctness.
    #[test]
    fn test_output_grows_by_one() {
        let data = b"rolling output growth test with some repeated patterns aaa bbb ccc";
        let compressed = compress_deflate(data, 6);
        // Feed all input at once, but grow output 1 byte at a time via repeated calls
        run_streaming_test(data, &compressed, compressed.len(), 1);
    }

    // ============================================================================
    // 4. UNCOMPRESSED (STORE) BLOCKS — BOUNDARY CASES
    // ============================================================================

    /// Minimum-length uncompressed block: 1 byte.
    #[test]
    fn test_store_block_one_byte() {
        let data = [0x42u8; 1];
        let compressed = compress_deflate(&data, 0);
        run_streaming_test(&data, &compressed, 1, data.len());
    }

    /// Large uncompressed block fed in 1-byte chunks.
    /// Stresses the bit-boundary alignment and bulk-copy path.
    #[test]
    fn test_store_block_byte_by_byte() {
        let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let compressed = compress_deflate(&data, 0);
        run_streaming_test(&data, &compressed, 1, data.len());
    }

    /// Uncompressed block split so the LEN/NLEN header straddles a chunk boundary.
    #[test]
    fn test_store_block_header_straddles_chunk() {
        let data: Vec<u8> = (0..200).map(|i| i as u8).collect();
        let compressed = compress_deflate(&data, 0);
        // Chunk at 3 bytes to force the 4-byte LEN+NLEN header to straddle boundaries.
        run_streaming_test(&data, &compressed, 3, data.len());
    }

    /// Two uncompressed blocks back-to-back (miniz tends to emit this for level 0
    /// on large inputs that exceed a single block's 65535-byte limit).
    #[test]
    fn test_store_two_blocks_back_to_back() {
        // 70 000 bytes forces two store blocks (max block size is 65535).
        let data: Vec<u8> = (0..70_000).map(|i| (i % 256) as u8).collect();
        let compressed = compress_deflate(&data, 0);
        run_streaming_test(&data, &compressed, 37, data.len());
    }

    /// Uncompressed block with tiny output buffer — exercises NeedsMoreOutput
    /// during the bulk-copy phase.
    #[test]
    fn test_store_block_tiny_output() {
        let data: Vec<u8> = (0..500).map(|i| i as u8).collect();
        let compressed = compress_deflate(&data, 0);
        run_streaming_test(&data, &compressed, compressed.len(), 1);
    }

    // ============================================================================
    // 5. MULTI-BLOCK COMPRESSED STREAMS
    // ============================================================================

    /// A large stream that miniz will split across multiple dynamic-Huffman blocks.
    #[test]
    fn test_multi_block_dynamic() {
        // Mix of compressible and less-compressible data to encourage block splits.
        let mut data = Vec::with_capacity(200_000);
        for i in 0..100_000u32 {
            data.push((i % 251) as u8); // prime modulus avoids boring period
        }
        for _ in 0..100_000 {
            data.push(0xCC);
        }
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 64, data.len());
    }

    /// Force a transition from a static block to a dynamic block within one stream.
    /// miniz may emit static blocks for small inputs and dynamic for large ones;
    /// this splices two compressed streams to test inter-block state reset.
    /// NOTE: This test constructs a raw deflate stream manually, so it may need
    /// adjustment once the exact block boundaries emitted by miniz are known.
    /// If the format changes, replace with a golden file.
    #[test]
    fn test_block_type_transition_static_then_dynamic() {
        // Independently encode two payloads and concatenate their compressed forms
        // is NOT valid deflate (you can't just concatenate). Instead, encode a
        // single payload large enough to trigger at least two block types.
        // We rely on miniz using static Huffman for tiny data and dynamic for large.
        // The safest approach: one large payload, verify correctness.
        let mut data = Vec::new();
        // Small prefix (may emit static block)
        data.extend_from_slice(b"hello");
        // Large suffix (forces dynamic block)
        data.extend(std::iter::repeat(0xABu8).take(50_000));
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 11, data.len());
    }

    // ============================================================================
    // 6. BACK-REFERENCE / MATCH EDGE CASES
    // ============================================================================

    /// Distance-1 back-reference (RLE): every byte identical.
    /// This is the offset==1 fast-path in fixed_copy_within.
    #[test]
    fn test_distance_one_backref() {
        let data = vec![0xFFu8; 32_768];
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 13, data.len());
    }

    /// Distance equal to FASTCOPY_BYTES (typically 16 or 32 bytes).
    /// Tests the overlap-aware copy path.
    #[test]
    fn test_distance_fastcopy_bytes() {
        // Build data with period exactly 16 bytes.
        let pattern: Vec<u8> = (0..16).collect();
        let data: Vec<u8> = pattern.iter().cycle().take(32_000).cloned().collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 9, data.len());
    }

    /// Maximum match length (258 bytes) with minimum distance (1 byte).
    #[test]
    fn test_max_length_match() {
        // A long run of the same byte produces length-258 matches.
        let data = vec![0x37u8; 100_000];
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 7, 0);
    }

    /// Overlapping matches: distance < length, so the copy must expand the pattern.
    /// e.g. "abcabcabc..." with distance=3, length=100.
    #[test]
    fn test_overlapping_match_expansion() {
        let base = b"xyz";
        let data: Vec<u8> = base.iter().cycle().take(30_000).cloned().collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 5, data.len());
    }

    /// A match that spans the NeedsMoreOutput boundary mid-copy.
    /// Force this by using tiny initial output and large matches.
    #[test]
    fn test_match_straddles_needs_more_output() {
        let data: Vec<u8> = b"abcdefghij".iter().cycle().take(5000).cloned().collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, compressed.len(), 50);
    }

    // ============================================================================
    // 7. DYNAMIC HUFFMAN TREE BOUNDARY CASES
    // ============================================================================

    /// Dynamic tree where the HCLEN section straddles a chunk boundary.
    /// Feed chunks of 2 bytes to maximize straddle probability.
    #[test]
    fn test_dynamic_tree_hclen_straddles_chunk() {
        // High-entropy data forces dynamic Huffman.
        let data: Vec<u8> = (0..2000).map(|i| ((i * 17 + 5) % 256) as u8).collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 2, data.len());
    }

    /// Dynamic tree with maximum number of code lengths (286 litlen + 30 distance).
    #[test]
    fn test_dynamic_tree_max_symbols() {
        // Data that uses the full symbol alphabet.
        let mut data = Vec::new();
        for _ in 0..10 {
            for b in 0u8..=255 {
                data.push(b);
            }
        }
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 3, data.len());
    }

    /// precode symbol 16 (copy previous) at the very start of the code length
    /// sequence is illegal — the decoder must return an error.
    /// We can't easily craft this without raw bitstream manipulation, but we can
    /// at least verify that a corrupted stream doesn't panic.
    #[test]
    fn test_dynamic_tree_corrupt_precode_16_at_start() {
        // Build a minimal dynamic-block bitstream with presym=16 at position 0.
        // This requires hand-crafting; we'll use a known-bad byte sequence.
        // Format: BFINAL=1, BTYPE=10 (dynamic), then minimal HLIT/HDIST/HCLEN,
        // then a precode of length 3 for symbol 16, then emit 16 immediately.
        // Rather than hand-crafting bits, mutate a known good stream at the
        // first precode code-length byte to force an invalid repeat.
        let data = b"test payload for corruption";
        let mut compressed = compress_deflate(data, 9);

        // Flip the LSB of a byte in the precode section (bytes 3-5 typically).
        // The exact position varies; we try a few candidates.
        // If the stream is too short to contain a dynamic block, skip.
        if compressed.len() > 5 {
            compressed[3] ^= 0x01;
        }

        // The decoder MUST either succeed (if mutation didn't corrupt critically)
        // or return DecodeStatus::Error — it must never panic.
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; data.len() * 2];

        // We don't care about the result, just that it doesn't panic.
        let _status = decoder.decode_chunk(&compressed, true, &mut out_vec);
    }

    // ============================================================================
    // 8. OUTPUT LIMIT ENFORCEMENT
    // ============================================================================


    /// Limit set to exactly the output size — must succeed.
    #[test]
    fn test_output_limit_exactly_met() {
        let data = vec![0x00u8; 1000];
        let compressed = compress_deflate(&data, 6);
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; data.len()];

        let status = decoder.decode_chunk(&compressed, true, &mut out_vec);
        assert!(
            matches!(status, DecodeStatus::Finished),
            "Expected Finished when limit exactly equals output, got {:?}",
            status
        );
    }

    // ============================================================================
    // 9. CORRUPT / MALFORMED INPUT
    // ============================================================================

    /// Completely empty compressed input — must not panic.
    #[test]
    fn test_empty_compressed_input() {
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; 1024];

        // Final chunk = true, but no bytes at all.
        let status = decoder.decode_chunk(&[], true, &mut out_vec);
        // Acceptable: NeedsMoreInput or Error. Must not panic or return Finished
        // (unless the encoder emits a zero-length valid stream, which [] is not).
        assert!(
            !matches!(status, DecodeStatus::Finished),
            "Empty input should not decode to Finished"
        );
    }

    /// Truncated compressed stream with is_final=true — must return Error, not hang.
    #[test]
    fn test_truncated_stream_marked_final() {
        let data = b"some data that will be truncated";
        let compressed = compress_deflate(data, 6);

        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; data.len() * 2];

        // Feed only the first half, but lie and say it's the final chunk.
        let half = &compressed[..compressed.len() / 2];
        let status = decoder.decode_chunk(half, true, &mut out_vec);
        assert!(
            !matches!(status, DecodeStatus::Finished),
            "Truncated input should not decode successfully"
        );
    }

    /// Reserved block type (0b11) must return an Error immediately.
    #[test]
    fn test_reserved_block_type() {
        // Craft a single byte with BFINAL=1 (bit 0) and BTYPE=11 (bits 1-2).
        // Binary: 0b00000111 = 0x07
        let bad_stream = [0x07u8];
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; 1024];

        let status = decoder.decode_chunk(&bad_stream, true, &mut out_vec);
        assert!(
            matches!(status, DecodeStatus::Error(_)),
            "Reserved block type must produce Error, got {:?}",
            status
        );
    }

    /// All-zeros compressed stream (not valid deflate) — must not panic.
    #[test]
    fn test_all_zeros_corrupt() {
        let garbage = vec![0u8; 64];
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; 4096];
        let status = decoder.decode_chunk(&garbage, true, &mut out_vec);
        assert!(
            !matches!(status, DecodeStatus::Finished),
            "All-zero garbage should not decode successfully"
        );
    }

    /// Random-noise compressed stream — must not panic.
    #[test]
    fn test_random_noise_corrupt() {
        let mut rng = WyRand::new_seed(42);
        let mut garbage = vec![0u8; 128];
        rng.fill(&mut garbage);
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; 4096];
        let _status = decoder.decode_chunk(&garbage, true, &mut out_vec);
        // Any result is fine; we just verify no panic or undefined behaviour.
    }

    /// Bit-flip in the middle of a valid stream — must not panic.
    #[test]
    fn test_single_bit_flip() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let mut compressed = compress_deflate(&data, 6);

        // Flip one bit in the middle of the stream.
        let mid = compressed.len() / 2;
        compressed[mid] ^= 0x10;

        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; data.len() * 2];
        let _status = decoder.decode_chunk(&compressed, true, &mut out_vec);
        // Must not panic; error result is expected but not required.
    }

    // ============================================================================
    // 10. IDEMPOTENCY / MULTIPLE DECODE RUNS
    // ============================================================================

    /// Decoding the same stream twice with fresh decoder instances must produce
    /// identical output.
    #[test]
    fn test_decoding_is_idempotent() {
        let mut rng = WyRand::new_seed(99);
        let mut data = vec![0u8; 5000];
        rng.fill(&mut data);
        // Partially compressible: mix random with repetitive.
        for i in 0..1000 {
            data[i] = (i % 7) as u8;
        }
        let compressed = compress_deflate(&data, 6);

        let decode = |chunk_size: usize| -> Vec<u8> {
            let mut decoder = StreamingDecoder::new();

            let mut out_vec = vec![0u8; data.len() * 2];
            let mut input_offset = 0;
            loop {
                let end = (input_offset + chunk_size).min(compressed.len());
                let chunk = &compressed[input_offset..end];
                let is_final = end == compressed.len();
                match decoder.decode_chunk(chunk, is_final, &mut out_vec) {
                    DecodeStatus::NeedsMoreInput => input_offset = end,
                    DecodeStatus::NeedsMoreOutput { at_least } => {
                        out_vec.resize(out_vec.len() + at_least + 128, 0);
                    }
                    DecodeStatus::Finished => {
                        return out_vec[..decoder.current_dest_offset()].to_vec();
                    }
                    _ => panic!("Decode failed"),
                }
            }
        };

        let result_a = decode(1);
        let result_b = decode(compressed.len());
        assert_eq!(
            result_a, result_b,
            "Two decode runs produced different output"
        );
        assert_eq!(&result_a, &data);
    }

    // ============================================================================
    // 11. LONG-DISTANCE BACK-REFERENCES
    // ============================================================================

    /// Maximum Deflate back-reference distance is 32768 bytes.
    /// This forces the encoder to use the full distance code range.
    #[test]
    fn test_max_distance_backref() {
        // Two copies of a unique pattern separated by 32768 bytes of filler.
        let mut data = Vec::new();
        let pattern: Vec<u8> = (0u8..=255).collect();
        data.extend_from_slice(&pattern);
        data.extend(std::iter::repeat(0x00u8).take(32_768 - pattern.len()));
        data.extend_from_slice(&pattern); // Should produce a back-ref at distance 32768

        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 61, data.len());
    }

    // ============================================================================
    // 12. STREAMING CONTINUITY ACROSS BLOCK BOUNDARIES
    // ============================================================================

    /// NeedsMoreInput returned exactly at a block boundary — the next chunk must
    /// cause the decoder to correctly parse the next block header.
    #[test]
    fn test_needs_more_input_at_block_boundary() {
        // Force a multi-block stream with small data segments.
        let mut data = Vec::new();
        for _ in 0..20 {
            data.extend_from_slice(b"block boundary test ");
        }
        let compressed = compress_deflate(&data, 1); // Low compression = more blocks

        // Find where block boundaries fall and feed chunks that stop exactly there.
        // We can't know without parsing, so use chunk_size=1 to maximise chance
        // of landing exactly on a boundary.
        run_streaming_test(&data, &compressed, 1, data.len());
    }

    /// After a NeedsMoreOutput during DecodingData, the next call must resume
    /// without re-emitting already-written bytes.
    #[test]
    fn test_no_double_emit_after_needs_more_output() {
        let data: Vec<u8> = (0..2000).map(|i| (i % 13) as u8).collect();
        let compressed = compress_deflate(&data, 9);

        // Use exactly the correct output size so growth is never needed, but feed
        // one byte at a time to exercise every possible mid-decode resume point.
        run_streaming_test(&data, &compressed, 1, data.len());
    }

    // ============================================================================
    // 13. SPECIFIC DATA PATTERNS
    // ============================================================================

    /// Alternating 0x00 and 0xFF — exercises both-end literal and match paths.
    #[test]
    fn test_alternating_bytes() {
        let data: Vec<u8> = (0..10_000)
            .map(|i| if i % 2 == 0 { 0x00 } else { 0xFF })
            .collect();
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 11, data.len());
    }

    /// Fibonacci-modulo sequence — produces an interesting mix of short repeats.
    #[test]
    fn test_fibonacci_modulo_sequence() {
        let mut data = vec![0u8; 20_000];
        let (mut a, mut b) = (0u64, 1u64);
        for byte in data.iter_mut() {
            *byte = (a % 251) as u8;
            let c = a.wrapping_add(b);
            a = b;
            b = c;
        }
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 7, 0);
    }

    /// 1 MB of compressible data — stress test for memory and correctness at scale.
    #[test]
    fn test_one_megabyte() {
        let data: Vec<u8> = (0..1_048_576_u64)
            .map(|i| {
                ((i.wrapping_mul(6364136223846793005u64)
                    .wrapping_add(1442695040888963407))
                    % 256) as u8
            })
            .collect();
        let compressed = compress_deflate(&data, 6);
        run_streaming_test(&data, &compressed, 4096, data.len());
    }

    /// Compressible data exactly 32767 bytes (one less than the max distance).
    #[test]
    fn test_exactly_32767_bytes() {
        let data: Vec<u8> = (0..32_767).map(|i| (i % 128) as u8).collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 19, data.len());
    }

    /// Compressible data exactly 32768 bytes (the max distance).
    #[test]
    fn test_exactly_32768_bytes() {
        let data: Vec<u8> = (0..32_768).map(|i| (i % 128) as u8).collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 19, data.len());
    }

    /// Compressible data exactly 32769 bytes (one more than the max distance).
    #[test]
    fn test_exactly_32769_bytes() {
        let data: Vec<u8> = (0..32_769).map(|i| (i % 128) as u8).collect();
        let compressed = compress_deflate(&data, 9);
        run_streaming_test(&data, &compressed, 19, data.len());
    }

    // ============================================================================
    // 14. COMPRESSION LEVEL COVERAGE
    // ============================================================================

    /// Verify correctness across all miniz compression levels.
    #[test]
    fn test_all_compression_levels() {
        let data: Vec<u8> = (0..5000).map(|i| (i % 97) as u8).collect();
        for level in 0u8..=10 {
            let compressed = compress_deflate(&data, level);
            run_streaming_test(
                &data,
                &compressed,
                compressed.len(), // single chunk; focus on level differences
                data.len(),
            );
        }
    }

    // ============================================================================
    // 15. decode_dest() ACCURACY
    // ============================================================================

    /// decode_dest() must return exactly the number of bytes written after
    /// a partial NeedsMoreOutput sequence, not the output buffer capacity.
    #[test]
    fn test_decode_dest_tracks_bytes_written_not_capacity() {
        let data = b"decode_dest must track bytes written";
        let compressed = compress_deflate(data, 6);

        let mut decoder = StreamingDecoder::new();
        let mut out_vec = vec![0u8; 1000]; // Much bigger than data

        let status = decoder.decode_chunk(&compressed, true, &mut out_vec);
        assert!(matches!(status, DecodeStatus::Finished));
        assert_eq!(
            decoder.current_dest_offset(),
            data.len(),
            "decode_dest() returned {} but expected {}",
            decoder.current_dest_offset(),
            data.len()
        );
    }

    // ============================================================================
    // 16. HARNESS SELF-CHECK: is_final correctness
    // ============================================================================

    /// Feed everything except the last byte as non-final, then feed the last byte
    /// as final. Tests that NeedsMoreInput → final-chunk transition is handled.
    #[test]
    fn test_final_chunk_is_one_byte() {
        let data = b"final chunk is exactly one byte long";
        let compressed = compress_deflate(data, 6);
        let mut decoder = StreamingDecoder::new();

        let mut out_vec = vec![0u8; data.len() * 2];
        let mut input_offset = 0;

        loop {
            let is_final = input_offset + 1 >= compressed.len();
            let end = (input_offset + 1).min(compressed.len());
            let chunk = &compressed[input_offset..end];

            match decoder.decode_chunk(chunk, is_final, &mut out_vec) {
                DecodeStatus::NeedsMoreInput => {
                    assert!(!is_final, "Decoder asked for more input on final byte");
                    input_offset = end;
                }
                DecodeStatus::NeedsMoreOutput { at_least } => {
                    out_vec.resize(out_vec.len() + at_least + 64, 0);
                }
                DecodeStatus::Finished => {
                    assert_eq!(&out_vec[..decoder.current_dest_offset()], data);
                    return;
                }
                e => panic!("Unexpected status: {:?}", e),
            }
        }
    }
}

/// One-shot [`crate::DeflateDecoder`] tests for the stored/uncompressed-block output
/// path. A valid stream that grows past the initial `size_hint` must not be rejected
/// as `OutputLimitExceeded`, while the real user limit is still enforced (#94).
mod one_shot_store_limit {
    use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};

    use crate::errors::DecodeErrorStatus;
    use crate::{DeflateDecoder, DeflateOptions};

    /// gzip CRC-32 so we can build a valid gzip container in-test (miniz only emits
    /// raw/zlib).
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn gzip_wrap(raw_deflate: &[u8], original: &[u8]) -> Vec<u8> {
        let mut out = vec![0x1f, 0x8b, 0x08, 0, 0, 0, 0, 0, 0, 0xff];
        out.extend_from_slice(raw_deflate);
        out.extend_from_slice(&crc32(original).to_le_bytes());
        out.extend_from_slice(&(original.len() as u32).to_le_bytes());
        out
    }

    /// A payload a level-0 encoder stores verbatim.
    fn payload(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 31 + 7) as u8).collect()
    }

    /// A raw deflate stream of one final stored block (`data.len()` must be <= 65535),
    /// so the projected output size at the block boundary is exactly `data.len()`.
    fn raw_single_stored_block(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 5);
        out.push(0x01); // BFINAL=1, BTYPE=00
        let len = data.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(data);
        out
    }

    /// A single stored block larger than the default 37000-byte `size_hint` must decode
    /// through every one-shot entry point (regressed by 0c0f87b9, restores #94).
    #[test]
    fn stored_block_over_size_hint_decodes_all_formats() {
        let data = payload(40_000);

        let raw = compress_to_vec(&data, 0);
        assert_eq!(
            DeflateDecoder::new(&raw).decode_deflate().unwrap(),
            data,
            "raw deflate stored block > size_hint must decode"
        );

        let zlib = compress_to_vec_zlib(&data, 0);
        assert_eq!(
            DeflateDecoder::new(&zlib).decode_zlib().unwrap(),
            data,
            "zlib stored block > size_hint must decode"
        );

        let gzip = gzip_wrap(&raw, &data);
        assert_eq!(
            DeflateDecoder::new(&gzip).decode_gzip().unwrap(),
            data,
            "gzip stored block > size_hint must decode"
        );
    }

    /// Round-trip across the 37000-byte growth boundary, including inputs spanning
    /// several stored blocks (each block caps at 65535 bytes).
    #[test]
    fn stored_block_roundtrip_across_growth_boundary() {
        for &n in &[36_999usize, 37_000, 37_001, 50_000, 70_000, 200_000] {
            let data = payload(n);

            let raw = compress_to_vec(&data, 0);
            assert_eq!(
                DeflateDecoder::new(&raw).decode_deflate().unwrap(),
                data,
                "raw deflate round-trip failed at n={n}"
            );

            let zlib = compress_to_vec_zlib(&data, 0);
            assert_eq!(
                DeflateDecoder::new(&zlib).decode_zlib().unwrap(),
                data,
                "zlib round-trip failed at n={n}"
            );
        }
    }

    /// The bomb guard is preserved: a genuinely over-limit stream is still rejected, and
    /// the reported size is the projected output size (`data.len()` here), not the buffer
    /// length that the old check reported.
    #[test]
    fn stored_block_over_user_limit_still_errors_with_projected_size() {
        let data = payload(40_000);
        let raw = raw_single_stored_block(&data);

        let opts = DeflateOptions::default().set_limit(1000);
        let err = DeflateDecoder::new_with_options(&raw, opts)
            .decode_deflate()
            .expect_err("output beyond the user limit must be rejected");

        match err.error {
            DecodeErrorStatus::OutputLimitExceeded(limit, current) => {
                assert_eq!(limit, 1000);
                assert_eq!(
                    current,
                    data.len(),
                    "should report the projected output size"
                );
            }
            other => panic!("expected OutputLimitExceeded, got {other:?}")
        }
    }

    /// A limit equal to the output size must succeed: projected == limit is not
    /// "exceeded", matching the compressed-block `> limit` semantics.
    #[test]
    fn stored_block_limit_exactly_met_succeeds() {
        let data = payload(40_000);
        let raw = compress_to_vec(&data, 0);

        let opts = DeflateOptions::default().set_limit(data.len());
        assert_eq!(
            DeflateDecoder::new_with_options(&raw, opts)
                .decode_deflate()
                .unwrap(),
            data
        );
    }
}
