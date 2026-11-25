use zune_core::bytestream::ZCursor;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

fn assert_native_colors(
    jpeg: &[u8], width: usize, height: usize, samples: &[(usize, usize, [u8; 3])],
) {
    let mut decoder = JpegDecoder::new(ZCursor::new(jpeg));
    let pixels = decoder.decode().expect("valid progressive JPEG");
    let info = decoder.info().unwrap();
    assert_eq!((info.width as usize, info.height as usize), (width, height));
    assert_eq!(pixels.len(), width * height * 3);
    // libjpeg-turbo measurements allow integer-IDCT rounding, not recoloring.
    for &(x, y, expected) in samples {
        let offset = (y * width + x) * 3;
        for channel in 0..3 {
            let actual = pixels[offset + channel];
            assert!(
                actual.abs_diff(expected[channel]) <= 2,
                "pixel ({x}, {y}) channel {channel}: {actual}, expected {}",
                expected[channel]
            );
        }
    }
}

#[test]
fn noninterleaved_dc_scan_uses_table_one() {
    assert_native_colors(
        include_bytes!("../../../test-images/jpeg/noninterleaved-dc-table-1-17x19.jpg"),
        17,
        19,
        &[
            (5, 5, [221, 30, 20]),
            (15, 5, [19, 199, 42]),
            (5, 16, [40, 20, 231]),
        ],
    );
}

#[test]
fn noninterleaved_dc_scan_uses_table_two() {
    assert_native_colors(
        include_bytes!("../../../test-images/jpeg/noninterleaved-dc-table-2-83x59.jpg"),
        83,
        59,
        &[
            (5, 5, [221, 30, 20]),
            (18, 5, [20, 200, 39]),
            (31, 5, [42, 20, 229]),
            (44, 5, [230, 180, 21]),
            (57, 5, [21, 180, 220]),
            (70, 5, [241, 240, 246]),
            (5, 16, [43, 18, 231]),
            (57, 49, [222, 28, 16]),
        ],
    );
}

#[test]
fn noninterleaved_dc_scan_uses_table_three() {
    assert_native_colors(
        include_bytes!("../../../test-images/jpeg/noninterleaved-dc-table-3-160x120.jpg"),
        160,
        120,
        &[
            (5, 5, [221, 30, 20]),
            (18, 5, [20, 200, 39]),
            (31, 5, [42, 20, 229]),
            (44, 5, [230, 180, 21]),
            (57, 5, [21, 180, 220]),
            (70, 5, [241, 240, 246]),
            (109, 71, [39, 19, 230]),
            (148, 115, [20, 200, 39]),
        ],
    );
}

#[test]
fn strict_mode_rejects_noninterleaved_dc_table_selectors_above_three() {
    let valid: &[u8] =
        include_bytes!("../../../test-images/jpeg/noninterleaved-dc-table-1-17x19.jpg");
    let expected = JpegDecoder::new(ZCursor::new(valid)).decode().unwrap();
    // The Cb DC scan: one component, id 2, DC table 1.
    let sos = valid
        .windows(7)
        .position(|bytes| bytes == [0xFF, 0xDA, 0x00, 0x08, 0x01, 0x02, 0x10])
        .unwrap();

    // Only tables 0 to 3 exist. libjpeg-turbo refuses these scans.
    for table in [5, 9, 13] {
        let mut jpeg = valid.to_vec();
        jpeg[sos + 6] = table << 4;

        let strict = DecoderOptions::default().set_strict_mode(true);
        let error = JpegDecoder::new_with_options(ZCursor::new(&jpeg), strict)
            .decode()
            .unwrap_err();
        assert!(
            format!("{error:?}").contains(&format!("No huffman table for DC component: {table}")),
            "{error:?}"
        );

        // Default mode still decodes the scan, with the selector wrapped to table 1.
        let pixels = JpegDecoder::new(ZCursor::new(&jpeg)).decode().unwrap();
        assert_eq!(pixels, expected, "table {table}");
    }
}
