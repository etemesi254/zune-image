//! Fuzz target for the incremental / resumable decoding API.
//!
//! Drives `JpegDecoder::decode_headers` and `decode_into` on the *same*
//! decoder instance while feeding bytes through a `GrowableCursor` that
//! exposes the input one chunk at a time. This exercises the resumable
//! state machine introduced for fine-grained header resume:
//!
//!   * `DecodingState::DecodeHeaders { resume_position }` and the
//!     marker-boundary checkpoints in `decode_headers_internal`.
//!   * `DecodingState::DecodeScan { scan_start_position, .. }` and the
//!     rollback-then-reseek path in `decode_into`.
//!   * `HeaderAppendStateSnapshot` capture/rollback inside
//!     `parse_marker_inner`, including the non-strict-mode inline-marker
//!     dispatch from `mcu.rs::check_stream_marker_after_mcu_width`.
//!
//! For inputs that one-shot-decode successfully, the incremental run must
//! reach the same final pixel buffer and ICC profile, with no metadata
//! duplication caused by scan replay re-encountering inline markers.

#![no_main]

use std::cell::Cell;
use std::io::{BufRead, Read, Seek, SeekFrom};
use std::rc::Rc;

use libfuzzer_sys::fuzz_target;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

/// Cursor over a byte slice with an externally-controllable visibility
/// limit. Reads/seeks past `limit` behave as EOF; growing `limit` simulates
/// more data arriving on the wire.
struct GrowableCursor<'a> {
    data: &'a [u8],
    position: usize,
    limit: Rc<Cell<usize>>,
}

impl<'a> GrowableCursor<'a> {
    fn new(data: &'a [u8], limit: Rc<Cell<usize>>) -> Self {
        Self {
            data,
            position: 0,
            limit,
        }
    }

    fn visible(&self) -> usize {
        self.limit.get().min(self.data.len())
    }
}

impl Read for GrowableCursor<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(0);
        }
        let available = &self.data[self.position..visible];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.position += n;
        Ok(n)
    }
}

impl BufRead for GrowableCursor<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let visible = self.visible();
        if self.position >= visible {
            return Ok(&[]);
        }
        Ok(&self.data[self.position..visible])
    }

    fn consume(&mut self, amt: usize) {
        self.position += amt;
    }
}

impl Seek for GrowableCursor<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(p) => self.position as i64 + p,
            SeekFrom::End(p) => self.visible() as i64 + p,
        };
        if new_pos < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

/// Hard cap to keep individual fuzz iterations bounded.
const MAX_INPUT_LEN: usize = 1 << 20; // 1 MiB

/// Safety net against inputs that never resolve to a terminal status.
const MAX_ITERATIONS: usize = 8192;

/// Pick a chunk size derived from input length so the full fuzz input
/// remains a valid JPEG (no bytes consumed as metadata). Different corpus
/// file sizes naturally explore different feeding cadences.
fn chunk_for(seed: u8) -> usize {
    match seed {
        0 => 1,
        1..=15 => seed as usize,
        16..=63 => (seed as usize) * 4,
        _ => (seed as usize) * 16,
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_LEN {
        return;
    }

    let chunk = chunk_for((data.len() % 256) as u8);
    let payload = data;
    if payload.len().div_ceil(chunk) > MAX_ITERATIONS {
        return;
    }

    // Reference one-shot decode (own copy of the bytes in a separate
    // decoder). If it succeeds, we use it as ground truth; if it fails,
    // we only require that the incremental path doesn't panic and reaches
    // a terminal state.
    let options = DecoderOptions::default()
        .set_strict_mode(true)
        .set_max_width(4_096)
        .set_max_height(4_096)
        .jpeg_set_max_scans(256);
    let mut oneshot = JpegDecoder::new_with_options(ZCursor::new(payload), options);
    let oneshot_pixels = oneshot.decode().ok();
    let oneshot_succeeded = oneshot_pixels.is_some();
    let oneshot_icc = oneshot.icc_profile();

    // Drive the incremental path on the same decoder instance.
    let limit = Rc::new(Cell::new(0_usize));
    let cursor = GrowableCursor::new(payload, Rc::clone(&limit));
    let mut decoder = JpegDecoder::new_with_options(cursor, options);
    decoder.set_incremental_mode(true);

    let mut out: Vec<u8> = Vec::new();
    let mut headers_done = false;

    // Phase 1: Feed data incrementally until headers are complete.
    for _ in 0..MAX_ITERATIONS {
        // Grow visibility by `chunk`, capped at the full payload length.
        let new_limit = limit.get().saturating_add(chunk).min(payload.len());
        if new_limit == limit.get() && new_limit == payload.len() {
            // We've already exposed everything and the previous call
            // didn't terminate — treat as a stuck loop and bail.
            break;
        }
        limit.set(new_limit);

        match decoder.decode_headers() {
            Ok(()) => {
                headers_done = true;
                let size = match decoder.output_buffer_size() {
                    Some(s) if s <= 64 * 1024 * 1024 => s,
                    _ => return, // refuse implausibly large allocations
                };
                out = vec![0_u8; size];
                break;
            }
            Err(ref e) if e.is_recoverable_eof() => continue,
            Err(error) => {
                assert!(
                    !oneshot_succeeded,
                    "incremental headers failed after strict one-shot success: {:?}",
                    error
                );
                return;
            }
        }
    }

    if !headers_done {
        assert!(
            !oneshot_succeeded,
            "incremental headers did not complete after strict one-shot success"
        );
        return;
    }

    // Phase 2: Continue exposing the scan incrementally and retry the same
    // decoder until it succeeds or reports a non-recoverable error.
    let mut scan_done = false;
    for _ in 0..MAX_ITERATIONS {
        match decoder.decode_into(&mut out) {
            Ok(()) => {
                scan_done = true;
                break;
            }
            Err(ref error) if error.is_recoverable_eof() => {
                if limit.get() == payload.len() {
                    assert!(
                        !oneshot_succeeded,
                        "incremental scan exhausted full input after strict one-shot success"
                    );
                    return;
                }
                limit.set(limit.get().saturating_add(chunk).min(payload.len()));
            }
            Err(error) => {
                assert!(
                    !oneshot_succeeded,
                    "incremental scan failed after strict one-shot success: {:?}",
                    error
                );
                return;
            }
        }
    }
    if !scan_done {
        assert!(
            !oneshot_succeeded,
            "incremental scan did not complete after strict one-shot success"
        );
        return;
    }

    // Compare against ground truth: one-shot succeeded AND incremental
    // finished with full data visibility for the scan.
    if let Some(expected_pixels) = oneshot_pixels {
        assert_eq!(
            out.len(),
            expected_pixels.len(),
            "incremental output length diverges from one-shot decode \
             (chunk={chunk}, len={})",
            payload.len()
        );
        assert_eq!(
            out,
            expected_pixels,
            "incremental pixels diverge from one-shot decode \
             (chunk={chunk}, len={})",
            payload.len()
        );

        let got_icc = decoder.icc_profile();
        assert_eq!(
            got_icc,
            oneshot_icc,
            "incremental ICC profile diverges from one-shot \
             (chunk={chunk}, len={}); duplication or loss across \
             scan retry",
            payload.len()
        );
    }
});
