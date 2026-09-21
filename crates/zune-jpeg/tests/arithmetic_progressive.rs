/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![cfg(feature = "arith")]

use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

const PIXEL_COUNT: usize = 32 * 32 * 3;

fn decode(data: &[u8], strict: bool) -> Vec<u8> {
    let options = DecoderOptions::default().set_strict_mode(strict);
    JpegDecoder::new_with_options(ZCursor::new(data), options)
        .decode()
        .unwrap()
}

fn assert_libjpeg_pixel_parity(name: &str, data: &[u8], reference: &[u8]) {
    assert_eq!(reference.len(), PIXEL_COUNT);
    for strict in [false, true] {
        let actual = decode(data, strict);
        assert_eq!(actual.len(), PIXEL_COUNT, "{name}: strict={strict}");

        let mut max_delta = 0;
        let mut total_delta = 0_u64;
        for (&actual, &expected) in actual.iter().zip(reference) {
            let delta = actual.abs_diff(expected);
            max_delta = max_delta.max(delta);
            total_delta += u64::from(delta);
        }
        let average_delta = total_delta as f64 / actual.len() as f64;
        assert!(
            max_delta <= 2,
            "{name}: strict={strict}, max channel delta {max_delta} exceeds 2"
        );
        assert!(
            average_delta < 0.015,
            "{name}: strict={strict}, average channel delta {average_delta} exceeds 0.015"
        );
    }
}

#[test]
fn one_coefficient_ac_scans_match_libjpeg_pixels() {
    // JPEGs are CC0-1.0 files from robert-ancell/jpegsuite revision
    // 8382e7831896cd1adf1cc61a1c9e565c4030aa43, mirrored by Chromium under
    // third_party/blink/web_tests/images/jpeg-suite. RGB references were
    // generated with libjpeg-turbo 3.1.0 built with WITH_ARITH_DEC=1 using
    // `djpeg -rgb -outfile reference.ppm input.jpg`; the PPM payload is stored.
    let reference =
        include_bytes!("../../../test-images/jpeg/arith/progressive_parity/libjpeg_grayscale.rgb");
    for (name, data) in [
        (
            "spectral_all",
            include_bytes!(
                "../../../test-images/jpeg/arith/progressive_parity/arith_spectral_all.jpg"
            )
            .as_slice()
        ),
        (
            "spectral_all_reverse",
            include_bytes!(
                "../../../test-images/jpeg/arith/progressive_parity/arith_spectral_all_reverse.jpg"
            )
            .as_slice()
        )
    ] {
        assert_libjpeg_pixel_parity(name, data, reference);
    }
}

#[test]
fn existing_progressive_arithmetic_controls_decode_strictly() {
    for (name, data) in [
        (
            "spectral",
            include_bytes!("../../../test-images/jpeg/arith/progressive_parity/arith_spectral.jpg")
                .as_slice()
        ),
        (
            "successive",
            include_bytes!(
                "../../../test-images/jpeg/arith/progressive_parity/arith_successive.jpg"
            )
            .as_slice()
        ),
        (
            "successive_ac",
            include_bytes!(
                "../../../test-images/jpeg/arith/progressive_parity/arith_successive_ac.jpg"
            )
            .as_slice()
        ),
        (
            "successive_dc",
            include_bytes!(
                "../../../test-images/jpeg/arith/progressive_parity/arith_successive_dc.jpg"
            )
            .as_slice()
        )
    ] {
        assert_eq!(decode(data, true).len(), PIXEL_COUNT, "{name}");
    }
}

#[test]
fn corresponding_progressive_huffman_orders_are_unchanged() {
    let ascending = decode(
        include_bytes!(
            "../../../test-images/jpeg/arith/progressive_parity/huffman_spectral_all.jpg"
        ),
        true
    );
    let descending = decode(
        include_bytes!(
            "../../../test-images/jpeg/arith/progressive_parity/huffman_spectral_all_reverse.jpg"
        ),
        true
    );

    assert_eq!(ascending.len(), PIXEL_COUNT);
    assert_eq!(ascending, descending);

    let arithmetic_ascending = decode(
        include_bytes!("../../../test-images/jpeg/arith/progressive_parity/arith_spectral_all.jpg"),
        true
    );
    let arithmetic_descending = decode(
        include_bytes!(
            "../../../test-images/jpeg/arith/progressive_parity/arith_spectral_all_reverse.jpg"
        ),
        true
    );
    assert_eq!(arithmetic_ascending, arithmetic_descending);
}

#[test]
fn arithmetic_conditioning_table_fifteen_decodes() {
    let original = include_bytes!("../../../test-images/jpeg/arith/seq.jpg");
    let expected = decode(original, true);
    let mut table_fifteen = original.to_vec();

    let dac = table_fifteen
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xCC])
        .expect("fixture must contain a DAC marker");
    let dac_len = usize::from(u16::from_be_bytes([
        table_fifteen[dac + 2],
        table_fifteen[dac + 3]
    ]));
    let mut moved_entries = 0;
    for entry in table_fifteen[dac + 4..dac + 2 + dac_len].chunks_exact_mut(2) {
        if entry[0] & 0x0F == 0 {
            entry[0] |= 0x0F;
            moved_entries += 1;
        }
    }
    assert_eq!(moved_entries, 2, "fixture must define DC and AC table 0");

    let sos = table_fifteen
        .windows(2)
        .position(|bytes| bytes == [0xFF, 0xDA])
        .expect("fixture must contain an SOS marker");
    assert_eq!(table_fifteen[sos + 4], 3, "fixture must have three scan components");
    assert_eq!(table_fifteen[sos + 5], 1, "first scan component must have ID 1");
    assert_eq!(table_fifteen[sos + 6], 0, "component 1 must initially use table 0");
    table_fifteen[sos + 6] = 0xFF;

    assert_eq!(decode(&table_fifteen, true), expected);

    let progressive = include_bytes!(
        "../../../test-images/jpeg/arith/progressive_parity/arith_spectral_all.jpg"
    );
    let expected = decode(progressive, true);
    let mut table_fifteen = progressive.to_vec();
    let sos_offsets: Vec<_> = table_fifteen
        .windows(2)
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == [0xFF, 0xDA]).then_some(offset))
        .collect();
    assert_eq!(sos_offsets.len(), 64, "fixture must contain 64 progressive scans");
    for sos in sos_offsets {
        assert_eq!(table_fifteen[sos + 4], 1, "fixture scans must have one component");
        assert_eq!(table_fifteen[sos + 6], 0, "fixture scans must initially use table 0");
        table_fifteen[sos + 6] = 0xFF;
    }

    assert_eq!(decode(&table_fifteen, true), expected);
}
