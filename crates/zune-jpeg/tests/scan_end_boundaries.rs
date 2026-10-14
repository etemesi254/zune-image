/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it and/or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for scan boundaries at the end of entropy-coded data.
//!
//! ITU-T T.81 section E.2.3 ends a scan on its MCU count, not on a marker, so a
//! scan whose data holds every MCU but ends with no EOI after it must decode
//! whole; and a scan whose data does not hold every MCU must never decode an
//! MCU from the zero padding the bit reader appends past the end of the data,
//! because such an MCU carries values the data never stated.

use zune_core::bytestream::ZCursor;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;

/// A 100 x 107 single-component baseline frame whose scan data holds all of its
/// MCUs and ends with no EOI. Without the fixes, the last MCU row (output rows
/// 104 to 106) decoded to 128; with them, it decodes like the same bytes with
/// FF D9 appended.
const COMPLETE_SCAN_WITHOUT_EOI: &[u8] = &decode_hex::<348>(
    "ffd8ffe000104a46494600010100000100010000ffdb004300100b0c0e0c0a100e0d0e1211101318281a181616183123
    251d283a333d3c3933383740485c4e404457453738506d51575f626768673e4d71797064785c656763ffc0000b08006b
    006401011100ffc400190001000301010000000000000000000000000102030407ffc400191001010101010100000000
    000000000000000102111203ffda0008010100003f00f3f0000000048701000253c4f0e1c388e23480129916917994f9
    4f856e55b956c56a0131791a672d6656984f856e19eb2cf519d54168d331b6236ce5acc26e14d658ef2c3719695a8168
    d72dfe6e8c46d989b19ea30dc73ed8e94a8168d32e8f9d7462b6cd5ad67bae7fa5736eb2d29502634cd6d8adf1a6b9da
    6ed4d6986f4c3759d56a04c5a5699d34ceda4da7dabadb2d699eaa955013169569a5a693ed5ba56e95b508004a7a74e9
    d47440000000000000037fd9",
);

/// Hex to bytes, ignoring whitespace, so the codestreams above stay readable.
const fn decode_hex<const N: usize>(hex: &str) -> [u8; N] {
    let bytes = hex.as_bytes();
    let mut out = [0u8; N];
    let mut i = 0;
    let mut o = 0;
    while o < N {
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        let high = (bytes[i] as char).to_digit(16).unwrap();
        let low = (bytes[i + 1] as char).to_digit(16).unwrap();
        out[o] = (high * 16 + low) as u8;
        i += 2;
        o += 1;
    }
    out
}

fn decode(bytes: &[u8]) -> Result<Vec<u8>, DecodeErrors> {
    let mut decoder = JpegDecoder::new(ZCursor::new(bytes));
    decoder.decode()
}

#[test]
fn a_complete_scan_without_eoi_decodes_like_one_with_eoi() {
    let plain = decode(COMPLETE_SCAN_WITHOUT_EOI).expect("the scan holds every MCU");
    let mut with_eoi = COMPLETE_SCAN_WITHOUT_EOI.to_vec();
    with_eoi.extend_from_slice(&[0xff, 0xd9]);
    let with = decode(&with_eoi).expect("the scan with an EOI decodes");
    assert_eq!(plain.len(), with.len(), "the two frames have one size");
    assert!(
        plain == with,
        "a scan that ends on its MCU count decodes whole without the EOI"
    );
}

/// Every row of a truncated decode is either exactly the whole frame's row or
/// the neutral filler — never values decoded from the zero padding appended
/// past the end of the data. The cutoffs are the ones the upstream review
/// reproduced the defect at.
#[test]
fn a_truncated_row_never_decodes_from_padding() {
    let data = include_bytes!("../../../test-images/jpeg/app14/baseline_rgb_interleaved.jpg");
    let full = decode(data).expect("the whole frame decodes");
    for cutoff in [948, 952, 953, 955, 1722, 1723, 1724, 2386, 2387, 2388, 2389] {
        let truncated = match decode(&data[..cutoff]) {
            Ok(pixels) => pixels,
            Err(_) => continue,
        };
        assert_eq!(truncated.len(), full.len(), "one buffer size at cutoff {cutoff}");
        let stride = full.len() / 32; // the frame is 32 x 32, so a row is a thirty-second of the buffer
        for (row, (a, b)) in truncated.chunks(stride).zip(full.chunks(stride)).enumerate() {
            let neutral = a.iter().all(|&v| v == 128);
            assert!(
                neutral || a == b,
                "cutoff {cutoff}, row {row}: neither the whole frame's row nor neutral filler"
            );
        }
    }
}