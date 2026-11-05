use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::errors::DecodeErrors;
use zune_jpeg::JpegDecoder;

const CHROMIUM: &[u8] = include_bytes!("../../../test-images/jpeg/lossless/32x32x8_grayscale.jpg");
const REFERENCE: &[u8] =
    include_bytes!("../../../test-images/jpeg/lossless/32x32x8_grayscale.gray");

#[test]
fn chromium_sof3_matches_libjpeg_turbo_exactly() {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    for data in [
        CHROMIUM,
        include_bytes!("../../../test-images/jpeg/lossless/32x32x8_grayscale_predictor1.jpg"),
        include_bytes!("../../../test-images/jpeg/lossless/32x32x8_grayscale_predictor7.jpg")
    ] {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
        assert_eq!(decoder.output_buffer_size(), None);
        decoder.decode_headers().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (32, 32));
        assert_eq!((info.pixel_density, info.components), (8, 1));
        assert!(info.sof.is_lossless());
        assert_eq!(decoder.input_colorspace(), Some(ColorSpace::Luma));
        assert_eq!(decoder.output_buffer_size(), Some(1024));
        let pixels = decoder.decode().unwrap();
        assert_eq!(pixels.len(), 1024);
        assert_eq!(pixels, REFERENCE);
    }
}

const SAMPLES: [u8; 9] = [0, 255, 1, 200, 3, 240, 5, 250, 2];
// Hand-calculated T.81 differences, including negative odd values in predictors 5 and 6.
const DIFFERENCES: [[i32; 9]; 7] = [
    [-128, 255, -254, 200, -197, 237, -195, 245, -248],
    [-128, 255, -254, 200, -252, 239, -195, 247, -238],
    [-128, 255, -254, 200, 3, -15, -195, 50, -1],
    [-128, 255, -254, 200, -452, 491, -195, 442, -485],
    [-128, 255, -254, 200, -324, 364, -195, 344, -366],
    [-128, 255, -254, 200, -352, 365, -195, 345, -361],
    [-128, 255, -254, 200, -224, 238, -195, 246, -243]
];

#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    byte:  u8,
    count: u8
}

impl Bits {
    fn put(&mut self, value: u32, length: u8) {
        for bit in (0..length).rev() {
            self.byte = (self.byte << 1) | ((value >> bit) & 1) as u8;
            self.count += 1;
            if self.count == 8 {
                self.bytes.push(self.byte);
                if self.byte == 255 {
                    self.bytes.push(0);
                }
                self.byte = 0;
                self.count = 0;
            }
        }
    }

    fn align(&mut self) {
        while self.count != 0 {
            self.put(1, 1);
        }
    }
}

fn image(width: u16, height: u16, predictor: u8, pt: u8, restart: u16, diffs: &[i32]) -> Vec<u8> {
    image_with_code_length(width, height, predictor, pt, restart, diffs, 5)
}

fn image_with_code_length(
    width: u16, height: u16, predictor: u8, pt: u8, restart: u16, diffs: &[i32], code_length: u8
) -> Vec<u8> {
    let mut data = vec![255, 216, 255, 195, 0, 11, 8];
    data.extend(height.to_be_bytes());
    data.extend(width.to_be_bytes());
    data.extend([1, 1, 0x11, 0, 255, 196, 0, 36, 0]);
    let mut counts = [0; 16];
    counts[usize::from(code_length) - 1] = 17;
    data.extend(counts);
    data.extend(0..=16);
    if restart != 0 {
        data.extend([255, 221, 0, 4]);
        data.extend(restart.to_be_bytes());
    }
    data.extend([255, 218, 0, 8, 1, 1, 0, predictor, 0, pt]);
    let mut bits = Bits::default();
    let mut rst = 0;
    for (index, &difference) in diffs.iter().enumerate() {
        if restart != 0 && index != 0 && index % usize::from(restart) == 0 {
            bits.align();
            bits.bytes.extend([255, 0xd0 + rst]);
            rst = (rst + 1) % 8;
        }
        let category = if difference == 32768 {
            16
        } else {
            (32 - difference.unsigned_abs().leading_zeros()) as u8
        };
        bits.put(u32::from(category), code_length);
        if category != 0 && category != 16 {
            let value = if difference < 0 { difference + (1 << category) - 1 } else { difference };
            bits.put(value as u32, category);
        }
    }
    bits.align();
    data.extend(bits.bytes);
    data.extend([255, 217]);
    data
}

fn decode(data: &[u8], strict: bool) -> Result<Vec<u8>, DecodeErrors> {
    JpegDecoder::new_with_options(
        ZCursor::new(data),
        DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::Luma)
            .set_strict_mode(strict)
    )
    .decode()
}

#[test]
fn all_predictors_use_correct_edges_and_signed_half_differences() {
    for (index, diffs) in DIFFERENCES.iter().enumerate() {
        for length in [5, 16] {
            let data = image_with_code_length(3, 3, index as u8 + 1, 0, 0, diffs, length);
            assert_eq!(decode(&data, true).unwrap(), SAMPLES);
        }
    }
}

#[test]
fn every_point_transform_restores_sample_precision() {
    for pt in 0..8 {
        let initial = 1 << (7 - pt);
        let maximum = (1 << (8 - pt)) - 1;
        let data = image(2, 2, 1, pt, 0, &[-initial, maximum, maximum, -maximum]);
        let maximum = (maximum << pt) as u8;
        assert_eq!(decode(&data, true).unwrap(), [0, maximum, maximum, 0]);
    }
}

#[test]
fn restarts_reset_the_predictor_and_wrap_marker_numbers() {
    for (index, diffs) in DIFFERENCES.iter().enumerate() {
        let differences = diffs[..6].repeat(10);
        let data = image(3, 20, index as u8 + 1, 0, 6, &differences);
        assert_eq!(decode(&data, true).unwrap(), SAMPLES[..6].repeat(10));

        let data = image(3, 3, index as u8 + 1, 2, 3, &[-32, 1, 1].repeat(3));
        assert_eq!(decode(&data, true).unwrap(), [0, 4, 8].repeat(3));
    }
}

#[test]
fn byte_stuffing_and_restart_fill_bytes_are_consumed() {
    let mut data = image(1, 2, 1, 0, 1, &[127, 127]);
    assert!(data.windows(2).any(|bytes| bytes == [255, 0]));
    let restart = data
        .windows(2)
        .position(|bytes| bytes == [255, 0xd0])
        .unwrap();
    data.insert(restart, 255);
    assert_eq!(decode(&data, true).unwrap(), [255, 255]);
}

#[test]
fn reconstruction_wraps_before_output_and_category_sixteen_has_no_payload() {
    let data = image(5, 1, 1, 0, 0, &[32767, 1, -1, 32768, 32768]);
    assert_eq!(decode(&data, true).unwrap(), [127, 128, 127, 127, 127]);
    let data = image(2, 2, 5, 0, 0, &[32768, 0, 0, 0]);
    assert_eq!(decode(&data, true).unwrap(), [128; 4]);
}

#[test]
fn single_samples_rows_and_columns_do_not_require_dct_padding() {
    for predictor in 1..=7 {
        for (width, height) in [(1, 1), (1, 9), (9, 1)] {
            let mut diffs = vec![1; usize::from(width * height)];
            diffs[0] = -128;
            let data = image(width, height, predictor, 0, 0, &diffs);
            assert_eq!(
                decode(&data, true).unwrap(),
                (0..diffs.len() as u8).collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn decode_into_checks_size_preserves_padding_and_can_replay() {
    for parse_headers in [false, true] {
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(CHROMIUM), options);
        if parse_headers {
            decoder.decode_headers().unwrap();
        }
        let mut short = [0xa5; 1023];
        assert!(matches!(
            decoder.decode_into(&mut short),
            Err(DecodeErrors::TooSmallOutput(1024, 1023))
        ));
        assert_eq!(short, [0xa5; 1023]);
        let mut pixels = [0xa5; 1037];
        for _ in 0..2 {
            decoder.decode_into(&mut pixels).unwrap();
            assert_eq!(&pixels[..1024], REFERENCE);
            assert_eq!(&pixels[1024..], &[0xa5; 13]);
            assert_eq!(decoder.decoded_output_bytes(), Some(1024));
            assert_eq!(decoder.decoded_scanlines(), Some(32));
        }
    }
}

#[test]
fn grayscale_expands_to_existing_packed_output_formats() {
    for color in [
        ColorSpace::Luma,
        ColorSpace::LumaA,
        ColorSpace::RGB,
        ColorSpace::BGR,
        ColorSpace::RGBA,
        ColorSpace::BGRA
    ] {
        let options = DecoderOptions::default().jpeg_set_out_colorspace(color);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(CHROMIUM), options);
        let pixels = decoder.decode().unwrap();
        let channels = color.num_components();
        assert_eq!(pixels.len(), 1024 * channels);
        for (pixel, &gray) in pixels.chunks_exact(channels).zip(REFERENCE) {
            let gray_channels = channels - usize::from(color.has_alpha());
            assert!(pixel[..gray_channels].iter().all(|&sample| sample == gray));
            if color.has_alpha() {
                assert_eq!(pixel[channels - 1], 255);
            }
        }
    }
}

#[test]
fn invalid_scans_and_tables_return_clear_errors() {
    let valid = image(1, 1, 1, 0, 0, &[0]);
    let sos = valid
        .windows(2)
        .position(|bytes| bytes == [255, 218])
        .unwrap();
    let dht = valid
        .windows(2)
        .position(|bytes| bytes == [255, 196])
        .unwrap();
    for (offset, value) in [
        (sos + 7, 0),
        (sos + 7, 8),
        (sos + 8, 1),
        (sos + 9, 0x10),
        (sos + 9, 8),
        (sos + 6, 1),
        (sos + 6, 0x40),
        (dht + 21, 17)
    ] {
        let mut data = valid.clone();
        data[offset] = value;
        for strict in [false, true] {
            assert!(
                decode(&data, strict).is_err(),
                "offset {offset}, value {value}"
            );
        }
    }
    let mut data = valid.clone();
    data[sos + 6] = 0x30;
    assert!(decode(&data, true)
        .unwrap_err()
        .to_string()
        .contains("Missing"));
}

#[test]
fn unsupported_lossless_combinations_stay_explicit() {
    let valid = image(1, 1, 1, 0, 0, &[0]);
    for precision in [2, 12, 16] {
        let mut data = valid.clone();
        data[6] = precision;
        assert!(matches!(
            decode(&data, true),
            Err(DecodeErrors::Unsupported(_))
        ));
    }
    let mut arithmetic = valid.clone();
    arithmetic[3] = 0xcb;
    arithmetic[6] = 16;
    assert!(matches!(
        decode(&arithmetic, true),
        Err(DecodeErrors::Unsupported(_))
    ));
    let mut multiple = valid.clone();
    multiple[5] += 9;
    multiple[11] = 4;
    multiple.splice(15..15, [2, 0x11, 0, 3, 0x11, 0, 4, 0x11, 0]);
    assert!(matches!(
        decode(&multiple, true),
        Err(DecodeErrors::Unsupported(_))
    ));
    let mut quantized = valid.clone();
    quantized[14] = 1;
    assert!(decode(&quantized, true)
        .unwrap_err()
        .to_string()
        .contains("selectors"));
    let mut dnl = valid;
    dnl[7..9].fill(0);
    assert!(decode(&dnl, true).unwrap_err().to_string().contains("DNL"));
}

#[test]
fn malformed_entropy_and_restart_sequences_are_not_synthesized() {
    for strict in [false, true] {
        let mut data = image(2, 2, 1, 0, 2, &[0; 4]);
        let rst = data
            .windows(2)
            .position(|bytes| bytes == [255, 0xd0])
            .unwrap();
        data[rst + 1] = 0xd3;
        assert!(decode(&data, strict)
            .unwrap_err()
            .to_string()
            .contains("restart"));
        let data = image(3, 2, 1, 0, 2, &[0; 6]);
        assert!(decode(&data, strict)
            .unwrap_err()
            .to_string()
            .contains("whole rows"));
        let mut data = image(3, 3, 1, 0, 0, &[0]);
        assert!(decode(&data, strict).is_err());
        data.truncate(data.len() - 3);
        assert!(decode(&data, strict).unwrap_err().is_recoverable_eof());
    }
    let mut data = image(1, 1, 1, 0, 0, &[0]);
    data.truncate(data.len() - 2);
    assert_eq!(decode(&data, false).unwrap(), [128]);
    assert!(decode(&data, true).unwrap_err().is_recoverable_eof());
}

#[test]
fn unsupported_streaming_interfaces_never_enter_a_dct_decoder() {
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(CHROMIUM), options);
    decoder.decode_headers().unwrap();
    let mut planes = [0xa5; 1024];
    assert!(decoder
        .raw_output()
        .decode_into_planes(&mut [&mut planes])
        .is_err());
    assert_eq!(planes, [0xa5; 1024]);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(CHROMIUM), options);
    let mut row = [0xa5; 32];
    assert!(decoder
        .scanline_output()
        .read_scanlines(&mut row, 32)
        .is_err());
    assert_eq!(row, [0xa5; 32]);
}

#[test]
fn single_component_frame_sampling_does_not_scale_the_image() {
    for sampling in [0x11, 0x12, 0x21, 0x22, 0x43] {
        let mut data = image(3, 3, 4, 0, 0, &DIFFERENCES[3]);
        data[13] = sampling;
        assert_eq!(decode(&data, true).unwrap(), SAMPLES);
    }
}

#[test]
fn cancellation_reports_only_stable_rows_and_can_replay() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(CHROMIUM), options);
    decoder.decode_headers().unwrap();
    let polls = Arc::new(AtomicUsize::new(0));
    decoder.set_cancel({
        let polls = Arc::clone(&polls);
        move || polls.fetch_add(1, Ordering::Relaxed) >= 2
    });
    decoder.set_cancel_interval(1);
    let mut output = [0xa5; 1024];
    assert!(matches!(
        decoder.decode_into(&mut output),
        Err(DecodeErrors::Cancelled)
    ));
    let stable = decoder.decoded_output_bytes().unwrap();
    assert!(stable < 1024);
    assert_eq!(stable % 32, 0);
    assert_eq!(&output[..stable], &REFERENCE[..stable]);
    decoder.set_cancel(|| false);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(&output, REFERENCE);
}

#[cfg(feature = "std")]
#[test]
fn byte_by_byte_input_replays_without_publishing_incomplete_rows() {
    assert_byte_by_byte(CHROMIUM, ColorSpace::Luma, REFERENCE);
    let color = include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr_interleaved.yuv");
    assert_byte_by_byte(
        include_bytes!("../../../test-images/jpeg/lossless/32x32x8_ycbcr_interleaved.jpg"),
        ColorSpace::YCbCr,
        color
    );
    #[cfg(feature = "arith")]
    {
        assert_byte_by_byte(
            include_bytes!("../../../test-images/jpeg/lossless/arith/32x32x8_grayscale.jpg"),
            ColorSpace::Luma,
            REFERENCE
        );
        assert_byte_by_byte(
            include_bytes!(
                "../../../test-images/jpeg/lossless/arith/32x32x8_ycbcr_interleaved.jpg"
            ),
            ColorSpace::YCbCr,
            color
        );
    }
}

#[cfg(feature = "std")]
fn assert_byte_by_byte(data: &'static [u8], color: ColorSpace, expected: &[u8]) {
    use std::cell::Cell;
    use std::io::{self, BufRead, Read, Seek, SeekFrom};
    use std::rc::Rc;

    struct Growing {
        visible:  Rc<Cell<usize>>,
        position: usize,
        data:     &'static [u8]
    }

    impl Read for Growing {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let source = self.fill_buf()?;
            let count = out.len().min(source.len());
            out[..count].copy_from_slice(&source[..count]);
            self.consume(count);
            Ok(count)
        }
    }

    impl BufRead for Growing {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            let visible = self.visible.get().min(self.data.len());
            Ok(&self.data[self.position.min(visible)..visible])
        }

        fn consume(&mut self, count: usize) {
            self.position += count;
        }
    }

    impl Seek for Growing {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            let position = match from {
                SeekFrom::Start(position) => i64::try_from(position),
                SeekFrom::Current(offset) => i64::try_from(self.position).map(|p| p + offset),
                SeekFrom::End(offset) => i64::try_from(self.visible.get()).map(|p| p + offset)
            }
            .map_err(|_| io::ErrorKind::InvalidInput)?;
            self.position = usize::try_from(position).map_err(|_| io::ErrorKind::InvalidInput)?;
            Ok(self.position as u64)
        }
    }

    let visible = Rc::new(Cell::new(0));
    let input = Growing {
        visible: Rc::clone(&visible),
        position: 0,
        data
    };
    let options = DecoderOptions::default().jpeg_set_out_colorspace(color);
    let mut decoder = JpegDecoder::new_with_options(input, options);
    decoder.set_incremental_mode(true);
    let mut output = vec![0xa5; expected.len()];
    for end in 0..=data.len() {
        visible.set(end);
        match decoder.decode_into(&mut output) {
            Ok(()) => {
                assert_eq!(end, data.len());
                assert_eq!(&output, expected);
            }
            Err(error) => {
                assert!(error.is_recoverable_eof(), "visible {end}: {error:?}");
                let stable = decoder.decoded_output_bytes().unwrap_or(0);
                assert_eq!(stable % (32 * color.num_components()), 0);
                assert_eq!(&output[..stable], &expected[..stable], "visible {end}");
            }
        }
    }
}
