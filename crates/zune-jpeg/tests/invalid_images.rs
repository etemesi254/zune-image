/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;

#[test]
fn eof() {
    let mut decoder = JpegDecoder::new(ZCursor::new([0xff, 0xd8, 0xa4]));

    decoder.decode().unwrap_err();
}

#[test]
fn bad_ff_marker_size() {
    let mut decoder = JpegDecoder::new(ZCursor::new([0xff, 0xd8, 0xff, 0x00, 0x00, 0x00]));

    let _ = decoder.decode().unwrap_err();
}

#[test]
fn bad_number_of_scans() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 218, 232, 197, 255]));

    // The input claims an SOS length of 59589 but only contains a handful of
    // bytes. With atomic marker parsing the truncation surfaces first as
    // `ExhaustedData`; without more bytes the decode cannot complete. We
    // accept either the original `SosError` message (if the parser ever
    // gets to see a complete-but-malformed body) or a recoverable EOF path.
    let err = decoder.decode().unwrap_err();
    assert!(
        err.is_recoverable_eof()
            || matches!(err, zune_jpeg::errors::DecodeErrors::SosError(_)),
        "unexpected error variant: {err:?}"
    );
}

#[test]
fn huffman_length_subtraction_overflow() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 196, 0, 0]));

    // A length field below 2 is a hard marker-format error.
    let err = decoder.decode().unwrap_err();
    assert!(
        matches!(
            err,
            zune_jpeg::errors::DecodeErrors::FormatStatic(_)
                | zune_jpeg::errors::DecodeErrors::Format(_)
        ),
        "unexpected error variant: {err:?}"
    );
}

#[test]
fn index_oob() {
    let mut decoder = JpegDecoder::new(ZCursor::new([255, 216, 255, 218, 0, 8, 1, 0, 8, 1]));

    let _ = decoder.decode().unwrap_err();
}

#[test]
fn mul_with_overflow() {
    let mut decoder = JpegDecoder::new(ZCursor::new([
        255, 216, 255, 192, 255, 1, 8, 9, 119, 48, 255, 192
    ]));

    // SOF with claimed length 65281 in a 12-byte input is simultaneously
    // truncated and malformed. With atomic marker parsing the truncation
    // surfaces first as `ExhaustedData`; without more bytes the decode
    // cannot complete. The original message "Length of start of frame
    // differs from expected ..." is no longer reachable (the body cannot
    // be read at all), but any hard failure is acceptable.
    let err = decoder.decode().unwrap_err();
    assert!(
        err.is_recoverable_eof()
            || matches!(err, zune_jpeg::errors::DecodeErrors::SofError(_)),
        "unexpected error variant: {err:?}"
    );
}

#[test]
fn baseline_scan_count_is_limited() {
    // Each baseline scan walks the whole image, so a few bytes per extra scan cost a
    // full-image pass each. Like progressive images, baseline images may have at
    // most `max_scans` scans.
    let data = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let limit = zune_core::options::DecoderOptions::default().jpeg_get_max_scans();
    let mut many = data[..data.len() - 2].to_vec();
    for _ in 0..limit {
        // SOS for component 1 with tables 0/0, then four bytes of entropy data
        many.extend([0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f, 0x00]);
        many.extend([0x12, 0x34, 0x56, 0x78]);
    }
    many.extend([0xff, 0xd9]);

    JpegDecoder::new(ZCursor::new(data.as_slice()))
        .decode()
        .unwrap();
    let err = JpegDecoder::new(ZCursor::new(many)).decode().unwrap_err();
    assert!(
        matches!(&err, zune_jpeg::errors::DecodeErrors::Format(m) if m.contains("Too many scans")),
        "unexpected error: {err:?}"
    );
}

/// Entropy-coded data writer: most significant bit first, 0xFF bytes stuffed, the last
/// byte padded with 1 bits.
struct Bits {
    out: Vec<u8>,
    acc: u8,
    n:   u8
}

impl Bits {
    fn new() -> Self {
        Bits {
            out: vec![],
            acc: 0,
            n:   0
        }
    }

    fn put(&mut self, value: u32, len: u8) {
        for i in (0..len).rev() {
            self.acc = (self.acc << 1) | ((value >> i) & 1) as u8;
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc);
                if self.acc == 0xff {
                    self.out.push(0x00);
                }
                self.acc = 0;
                self.n = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        while self.n != 0 {
            self.put(1, 1);
        }
        self.out
    }
}

/// AC symbols of the test table in code order; each code is its index in `code_len` bits.
const EOB: u32 = 0;
const LAST: u32 = 1;
const RUN_11: u32 = 2;
const ZRL: u32 = 3;

#[derive(Clone, Copy, Debug)]
enum Layout {
    /// SOF0, one component.
    Baseline,
    /// SOF0, three components decoded to Luma, with the overrun in Cb, whose blocks are
    /// skipped rather than decoded.
    BaselineSkipped,
    /// SOF2, one component: a DC scan, then an AC scan (Ss 1, Se 63).
    Progressive
}

/// Blocks per row of the test image. The bad block is the first; the ones after it keep
/// the decoder away from the end of the input when it fails, where an error is reported
/// as `ExhaustedData` so that incremental decoding can retry.
const BLOCKS: u16 = 32;

/// A (8 * BLOCKS)x8 JPEG whose first block has AC data three ZRLs (to zig-zag 49), run 11
/// with +1 (at 60), then `last_run_and_size` with +1: with 0x91 (run 9) that run ends at
/// 70, past the end of the block; with 0x21 (run 2) at 63. Every other block is DC 0 and
/// EOB. The AC codes are `code_len` bits: 12-bit codes are decoded on the slow path,
/// 3-bit codes through the fast lookup table.
fn ac_run_past_end(layout: Layout, code_len: u8, last_run_and_size: u8) -> Vec<u8> {
    let components: u8 = if matches!(layout, Layout::BaselineSkipped) { 3 } else { 1 };
    let overrun_id = if components == 3 { 2 } else { 1 };
    let mut data = vec![0xff, 0xd8, 0xff, 0xdb, 0x00, 0x43, 0x00];
    data.extend([0x10; 64]);
    let sof = if matches!(layout, Layout::Progressive) { 0xc2 } else { 0xc0 };
    let [w_hi, w_lo] = (8 * BLOCKS).to_be_bytes();
    data.extend([
        0xff,
        sof,
        0x00,
        8 + 3 * components,
        0x08,
        0x00,
        0x08,
        w_hi,
        w_lo,
        components
    ]);
    for id in 1..=components {
        data.extend([id, 0x11, 0x00]);
    }
    // DC table: one symbol (size 0), code `0`
    data.extend([0xff, 0xc4, 0x00, 0x14, 0x00, 0x01]);
    data.extend([0x00; 16]);
    // AC table: EOB, the last run/size, run 11 size 1 and ZRL, each `code_len` bits
    let mut counts = [0u8; 16];
    counts[usize::from(code_len) - 1] = 4;
    data.extend([0xff, 0xc4, 0x00, 0x17, 0x10]);
    data.extend(counts);
    data.extend([0x00, last_run_and_size, 0xb1, 0xf0]);

    let ac = |bits: &mut Bits, symbol: u32| bits.put(symbol, code_len);
    let overrun = |bits: &mut Bits| {
        for _ in 0..3 {
            ac(bits, ZRL);
        }
        ac(bits, RUN_11);
        bits.put(1, 1);
        ac(bits, LAST);
        bits.put(1, 1);
    };
    let sos = |data: &mut Vec<u8>, ids: &[u8], ss: u8, se: u8| {
        let n = ids.len() as u8;
        data.extend([0xff, 0xda, 0x00, 6 + 2 * n, n]);
        for &id in ids {
            data.extend([id, 0x00]);
        }
        data.extend([ss, se, 0x00]);
    };
    let mut bits = Bits::new();
    match layout {
        Layout::Baseline | Layout::BaselineSkipped => {
            let ids: Vec<u8> = (1..=components).collect();
            sos(&mut data, &ids, 0, 63);
            for block in 0..BLOCKS {
                for &id in &ids {
                    bits.put(0, 1); // DC 0
                    if block == 0 && id == overrun_id {
                        overrun(&mut bits);
                    } else {
                        ac(&mut bits, EOB);
                    }
                }
            }
        }
        Layout::Progressive => {
            sos(&mut data, &[1], 0, 0);
            let mut dc = Bits::new();
            for _ in 0..BLOCKS {
                dc.put(0, 1); // DC 0
            }
            data.extend(dc.finish());
            sos(&mut data, &[1], 1, 63);
            overrun(&mut bits);
            for _ in 1..BLOCKS {
                ac(&mut bits, EOB);
            }
        }
    }
    data.extend(bits.finish());
    data.extend([0xff, 0xd9]);
    data
}

fn decode_luma(data: &[u8], strict: bool) -> Result<Vec<u8>, DecodeErrors> {
    let options = DecoderOptions::default()
        .set_strict_mode(strict)
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    JpegDecoder::new_with_options(ZCursor::new(data), options).decode()
}

#[test]
fn ac_run_past_end_of_block() {
    // A run past the last coefficient is malformed. Lenient decoding puts the coefficient
    // on the last position, as libjpeg-turbo does, instead of wrapping it onto the DC, so
    // the stream decodes like the one whose run ends exactly at 63; strict decoding
    // rejects it. The same must hold on the slow and fast AC paths, when the component is
    // skipped, and in a progressive AC scan.
    let mut failures = vec![];
    for layout in [
        Layout::Baseline,
        Layout::BaselineSkipped,
        Layout::Progressive
    ] {
        for code_len in [12, 3] {
            let case = format!("{layout:?}, {code_len}-bit AC codes");
            let past = ac_run_past_end(layout, code_len, 0x91);
            let at_end = ac_run_past_end(layout, code_len, 0x21);
            match (decode_luma(&past, false), decode_luma(&at_end, false)) {
                (Ok(a), Ok(b)) if a == b => {}
                (Ok(_), Ok(_)) => failures.push(format!("{case}: lenient: pixels differ")),
                (a, b) => failures.push(format!("{case}: lenient: {:?} / {:?}", a.err(), b.err()))
            }
            if let Err(e) = decode_luma(&at_end, true) {
                failures.push(format!(
                    "{case}: strict rejected a run that ends at 63: {e:?}"
                ));
            }
            match decode_luma(&past, true) {
                Err(DecodeErrors::FormatStatic(m)) if m.contains("past the end") => {}
                Err(e) => failures.push(format!("{case}: strict: {e:?}")),
                Ok(_) => failures.push(format!("{case}: strict: accepted"))
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn test_panic_on_slice() {
    const JPEG_DATA: [u8; 1394] = [
        255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0, 67, 0, 3,
        2, 2, 3, 2, 2, 3, 3, 3, 3, 4, 3, 3, 4, 5, 8, 5, 5, 4, 4, 5, 10, 7, 7, 6, 8, 12, 10, 12, 12, 11,
        10, 11, 11, 13, 14, 18, 16, 13, 14, 17, 14, 11, 11, 16, 22, 16, 17, 19, 20, 21, 21, 21, 12, 15,
        23, 24, 22, 20, 24, 18, 20, 21, 20, 255, 219, 0, 67, 1, 3, 4, 4, 5, 4, 5, 9, 5, 5, 9, 19, 13,
        11, 13, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20,
        20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20, 20,
        20, 20, 20, 20, 255, 194, 0, 17, 8, 0, 100, 0, 100, 3, 1, 18, 0, 2, 17, 1, 3, 33, 1, 255, 196,
        0, 25, 0, 1, 0, 3, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 7, 8, 9, 4, 255, 196, 0, 40,
        16, 0, 1, 2, 4, 1, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 6, 23, 85, 164, 210, 4, 7, 21, 24,
        25, 86, 99, 102, 162, 163, 165, 211, 225, 227, 255, 196, 0, 26, 1, 1, 1, 0, 3, 1, 1, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 7, 5, 8, 9, 6, 4, 255, 196, 0, 43, 17, 0, 1, 0, 6, 6, 11, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 2, 5, 22, 83, 162, 209, 1, 20, 100, 161, 210, 226, 3, 4, 19, 21, 24, 81,
        82, 97, 101, 163, 227, 255, 218, 0, 12, 3, 1, 0, 2, 17, 3, 0, 0, 0, 0, 83, 2, 0, 100, 0, 40, 0,
        48, 0, 68, 83, 8, 1, 1, 0, 3, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 5, 8, 9, 6, 4, 255,
        196, 46, 55, 0, 8, 0, 24, 0, 85, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 5, 22, 83, 162, 209,
        1, 20, 100, 161, 210, 226, 3, 4, 19, 21, 24, 81, 82, 97, 101, 163, 227, 255, 218, 0, 12, 3, 1,
        0, 2, 17, 3, 0, 0, 0, 0, 83, 2, 0, 100, 0, 40, 0, 48, 0, 68, 83, 8, 0, 49, 46, 48, 92, 49, 46,
        48, 32, 40, 0, 0, 1, 85, 83, 253, 49, 52, 53, 51, 53, 57, 54, 55, 64, 74, 212, 120, 47, 92,
        123, 191, 57, 191, 109, 82, 132, 59, 170, 172, 253, 255, 255, 0, 0, 255, 251, 255, 188, 172,
        247, 255, 166, 189, 173, 160, 185, 170, 179, 179, 215, 255, 249, 255, 170, 172, 253, 255, 255,
        255, 215, 255, 239, 255, 170, 172, 253, 255, 155, 0, 40, 0, 17, 63, 0, 206, 128, 134, 29, 83,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 53, 164, 12, 100, 73, 42, 215, 188, 64, 198, 68, 146, 173, 123, 207,
        89, 81, 213, 250, 1, 1, 85, 83, 2, 0, 8, 82, 184, 125, 10, 56, 68, 12, 100, 73, 42, 215, 188,
        64, 199, 68, 146, 173, 123, 197, 71, 87, 233, 190, 153, 134, 165, 112, 250, 20, 112, 136, 24,
        200, 146, 85, 175, 120, 21, 25, 95, 166, 250, 102, 26, 149, 195, 232, 81, 194, 73, 243, 238,
        227, 159, 208, 207, 187, 142, 127, 68, 233, 185, 179, 71, 148, 231, 143, 19, 94, 31, 223, 241,
        25, 247, 113, 207, 232, 103, 221, 199, 63, 160, 220, 217, 163, 202, 56, 154, 240, 254, 255, 0,
        136, 207, 187, 142, 127, 67, 62, 238, 57, 253, 6, 230, 205, 30, 81, 196, 215, 135, 247, 252,
        70, 125, 220, 115, 250, 25, 247, 113, 207, 232, 55, 54, 104, 242, 142, 38, 188, 63, 191, 226,
        51, 238, 227, 159, 208, 207, 187, 142, 127, 65, 185, 179, 71, 148, 113, 53, 225, 253, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 0, 16, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 95, 214, 78, 225, 14, 220, 50, 254, 178, 119,
        13, 200, 176, 119, 122, 51, 12, 226, 209, 212, 72, 204, 67, 183, 12, 191, 172, 157, 194, 29,
        184, 101, 253, 100, 238, 27, 145, 96, 238, 244, 102, 25, 197, 163, 168, 145, 152, 135, 110, 25,
        127, 89, 59, 132, 59, 112, 203, 250, 201, 220, 55, 34, 193, 221, 232, 204, 51, 139, 71, 81, 35,
        49, 14, 220, 50, 254, 178, 119, 8, 118, 225, 151, 245, 147, 184, 110, 69, 131, 187, 209, 152,
        103, 22, 142, 162, 70, 98, 29, 184, 101, 253, 100, 238, 16, 237, 195, 47, 235, 39, 112, 220,
        139, 7, 119, 163, 48, 206, 45, 29, 68, 140, 196, 59, 112, 203, 250, 201, 220, 6, 228, 88, 59,
        189, 25, 134, 113, 104, 234, 36, 102, 93, 64, 170, 22, 192, 0, 0, 0, 0, 0, 0, 0, 0, 3, 133,
        224, 238, 25, 85, 0, 0, 0, 0, 0, 0, 0, 0, 0, 55, 22, 141, 217, 57, 217, 218, 236, 79, 144, 104,
        221, 147, 157, 157, 174, 196, 249, 13, 127, 106, 87, 15, 161, 71, 9, 150, 216, 104, 249, 13,
        27, 178, 115, 179, 181, 216, 159, 32, 209, 187, 39, 59, 59, 93, 137, 242, 6, 165, 112, 250, 20,
        112, 141, 134, 143, 144, 209, 187, 39, 43, 59, 93, 137, 242, 13, 27, 178, 115, 179, 181, 216,
        159, 32, 106, 87, 15, 161, 71, 8, 216, 72, 250, 149, 236, 103, 31, 140, 238, 196, 0, 1, 0, 110,
        120, 88, 15, 1, 247, 143, 129, 0, 110, 108, 209, 229, 50, 181, 110, 227, 88, 15, 1, 247, 143,
        128, 214, 3, 192, 125, 227, 224, 27, 155, 52, 121, 69, 91, 184, 214, 3, 192, 125, 227, 224, 53,
        128, 240, 31, 120, 248, 6, 230, 205, 30, 81, 86, 238, 53, 128, 240, 31, 120, 248, 13, 96, 60,
        7, 222, 62, 1, 185, 179, 71, 148, 85, 187, 141, 96, 60, 7, 222, 62, 3, 88, 15, 1, 247, 143,
        128, 110, 108, 209, 229, 21, 110, 227, 88, 15, 1, 247, 143, 128, 214, 3, 192, 125, 227, 224,
        27, 155, 52, 121, 69, 91, 184, 214, 3, 192, 125, 227, 224, 3, 115, 102, 143, 40, 171, 119, 49,
        100, 45, 148, 203, 42, 18, 184, 66, 215, 60, 178, 161, 43, 137, 89, 246, 136, 90, 231, 150, 84,
        37, 112, 133, 174, 121, 101, 66, 87, 0, 33, 107, 158, 89, 80, 149, 194, 22, 185, 229, 149, 9,
        92, 0, 133, 174, 121, 101, 66, 87, 8, 90, 231, 150, 84, 37, 112, 2, 22, 185, 229, 149, 9, 92,
        33, 107, 158, 89, 80, 149, 192, 8, 90, 231, 150, 84, 37, 112, 133, 174, 121, 101, 66, 87, 0,
        33, 107, 158, 89, 80, 149, 192, 3, 64, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 14, 171, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 11, 250, 23, 54, 37, 149, 10, 220, 33, 115, 98, 89, 80, 173, 192, 8, 92, 216, 150,
        84, 43, 112, 133, 205, 137, 101, 66, 183, 0, 33, 115, 98, 89, 80, 173, 194, 23, 54, 37, 149,
        10, 220, 0, 133, 205, 137, 101, 66, 183, 8, 92, 216, 150, 84, 43, 112, 2, 23, 54, 37, 149, 10,
        240, 33, 115, 98, 89, 80, 173, 192, 8, 92, 216, 150, 84, 43, 112, 133, 205, 137, 101, 66, 183,
        0, 33, 115, 98, 89, 80, 173, 192, 2, 84, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 63, 255, 217, 0,
    ];
    let  _ = JpegDecoder::new(ZCursor::new(JPEG_DATA)).decode();
}

