use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

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

    fn block(&mut self) {
        self.put(0b10, 2); // DC difference category 3.
        self.put(4, 3);
        self.put(0, 1); // EOB.
    }

    fn finish(mut self) -> Vec<u8> {
        self.align();
        self.bytes
    }
}

fn image(
    width: u16, height: u16, components: u8, transform: Option<u8>, restart: u16, scan: &[u8]
) -> Vec<u8> {
    let mut out = vec![255, 216];
    if let Some(transform) = transform {
        out.extend([255, 238, 0, 14]);
        out.extend(b"Adobe");
        out.extend([0, 100, 0, 0, 0, 0, transform]);
    }
    out.extend([255, 219, 0, 67, 0]);
    out.extend([8; 64]);
    for (selector, values) in [(0, [0, 3]), (0x11, [0, 1])] {
        out.extend([255, 196, 0, 21, selector, 1, 1]);
        out.extend([0; 14]);
        out.extend(values);
    }
    out.extend([255, 192, 0, 8 + 3 * components, 8]);
    out.extend((height * 8).to_be_bytes());
    out.extend((width * 8).to_be_bytes());
    out.push(components);
    for component in 1..=components {
        out.extend([component, 0x11, 0]);
    }
    if restart != 0 {
        out.extend([255, 221, 0, 4]);
        out.extend(restart.to_be_bytes());
    }
    out.extend([255, 218, 0, 6 + 2 * components, components]);
    for component in 1..=components {
        out.extend([component, 1]);
    }
    out.extend([0, 63, 0]);
    out.extend(scan);
    out.extend([255, 217]);
    out
}

fn invalid_dc() -> Vec<u8> {
    let mut bits = Bits::default();
    bits.block();
    bits.put((1 << 17) - 1, 17);
    bits.put(0, 1);
    bits.block();
    image(3, 1, 1, None, 0, &bits.finish())
}

fn invalid_ac() -> Vec<u8> {
    let mut bits = Bits::default();
    bits.put(0b10100, 5);
    bits.put(0b101, 3); // Preserve one nonzero AC coefficient before the error.
    bits.put((1 << 17) - 1, 17);
    bits.block();
    image(2, 1, 1, None, 0, &bits.finish())
}

fn early_eoi(components: u8, transform: Option<u8>, height: u16) -> Vec<u8> {
    let mut bits = Bits::default();
    for _ in 0..components {
        bits.block();
    }
    bits.block(); // Only Y of the second MCU is present for color input.
    image(4, height, components, transform, 0, &bits.finish())
}

fn discarded_damage(ac: bool) -> Vec<u8> {
    let mut bits = Bits::default();
    bits.block();
    if ac {
        bits.put(0b10100, 5);
        bits.put(0b101, 3);
    }
    bits.put((1 << 17) - 1, 17);
    if !ac {
        bits.put(0, 1);
    }
    bits.block();
    for _ in 0..3 {
        bits.block();
    }
    image(2, 1, 3, Some(1), 0, &bits.finish())
}

fn short_restart() -> Vec<u8> {
    let mut first = Bits::default();
    first.block();
    first.block();
    let mut scan = first.finish();
    scan.extend([255, 0xd0]);
    let mut second = Bits::default();
    for _ in 0..4 {
        second.block();
    }
    scan.extend(second.finish());
    image(2, 4, 1, None, 4, &scan)
}

fn empty_restart() -> Vec<u8> {
    let mut scan = vec![255, 0xd0];
    let mut bits = Bits::default();
    bits.block();
    bits.block();
    scan.extend(bits.finish());
    image(2, 2, 1, None, 2, &scan)
}

fn options(color: ColorSpace, strict: bool) -> DecoderOptions {
    DecoderOptions::default()
        .set_strict_mode(strict)
        .set_max_width(65535)
        .set_max_height(65535)
        .jpeg_set_out_colorspace(color)
}

#[test]
fn invalid_dc_consumes_seventeen_bits_and_preserves_the_predictor() {
    let data = invalid_dc();
    let mut decoder =
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, false));
    let output = decoder.decode().unwrap();
    for row in output.chunks_exact(24) {
        assert_eq!(&row[..8], &[132; 8]);
        assert_eq!(&row[8..16], &[132; 8]);
        assert_eq!(&row[16..], &[136; 8]);
    }
    assert!(
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, true))
            .decode()
            .is_err()
    );
}

#[test]
fn invalid_ac_preserves_decoded_coefficients_and_continues() {
    let data = invalid_ac();
    let mut decoder =
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, false));
    let output = decoder.decode().unwrap();
    for row in output.chunks_exact(16) {
        assert_eq!(&row[..8], &[133, 133, 133, 132, 132, 131, 131, 131]);
        assert_eq!(&row[8..], &[136; 8]);
    }
    assert!(
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, true))
            .decode()
            .is_err()
    );
}

#[test]
fn discarded_components_use_the_same_recovery_and_strict_validation() {
    for ac in [false, true] {
        let data = discarded_damage(ac);
        let mut decoder =
            JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, false));
        let output = decoder.decode().unwrap();
        for row in output.chunks_exact(16) {
            assert_eq!(&row[..8], &[132; 8]);
            assert_eq!(&row[8..], &[136; 8]);
        }
        assert!(JpegDecoder::new_with_options(
            ZCursor::new(&data),
            options(ColorSpace::Luma, true)
        )
        .decode()
        .is_err());
    }
    let data = early_eoi(3, Some(1), 3);
    assert!(
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, true))
            .decode()
            .is_err()
    );
}

#[test]
fn prefetched_eoi_does_not_discard_real_bits_or_consume_the_next_image() {
    let mut bits = Bits::default();
    for _ in 0..4 {
        bits.block();
    }
    let mut data = image(4, 1, 1, None, 0, &bits.finish());
    let end = data.len();
    data.extend([0xab; 32]);
    for strict in [false, true] {
        let mut decoder =
            JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, strict));
        let output = decoder.decode().unwrap();
        for row in output.chunks_exact(32) {
            for (block, &expected) in row.chunks_exact(8).zip(&[132, 136, 140, 144]) {
                assert!(block.iter().all(|&sample| sample == expected));
            }
        }
        assert_eq!(decoder.inner_reader().position().unwrap(), end as u64);
    }
}

#[cfg(feature = "std")]
#[test]
fn damaged_scans_and_final_eoi_retry_byte_by_byte_without_publishing_wrong_rows() {
    use std::cell::Cell;
    use std::io::{self, BufRead, Read, Seek, SeekFrom};
    use std::rc::Rc;

    struct Growing<'a> {
        data:     &'a [u8],
        limit:    Rc<Cell<usize>>,
        position: usize
    }
    impl Read for Growing<'_> {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            let source = self.fill_buf()?;
            let count = source.len().min(out.len());
            out[..count].copy_from_slice(&source[..count]);
            self.consume(count);
            Ok(count)
        }
    }
    impl BufRead for Growing<'_> {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            let end = self.limit.get().min(self.data.len());
            Ok(&self.data[self.position.min(end)..end])
        }
        fn consume(&mut self, count: usize) {
            self.position += count;
        }
    }
    impl Seek for Growing<'_> {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            let position = match from {
                SeekFrom::Start(position) => i64::try_from(position),
                SeekFrom::Current(offset) => i64::try_from(self.position).map(|p| p + offset),
                SeekFrom::End(offset) => i64::try_from(self.limit.get()).map(|p| p + offset)
            }
            .map_err(|_| io::ErrorKind::InvalidInput)?;
            self.position = usize::try_from(position).map_err(|_| io::ErrorKind::InvalidInput)?;
            Ok(self.position as u64)
        }
    }
    for data in [invalid_dc(), invalid_ac(), early_eoi(3, Some(1), 3)] {
        let expected =
            JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::RGB, false))
                .decode()
                .unwrap();
        let limit = Rc::new(Cell::new(0));
        let input = Growing {
            data:     &data,
            limit:    Rc::clone(&limit),
            position: 0
        };
        let mut decoder = JpegDecoder::new_with_options(input, options(ColorSpace::RGB, false));
        decoder.set_incremental_mode(true);
        let mut output = vec![0xa5; expected.len()];
        for end in 0..=data.len() {
            limit.set(end);
            match decoder.decode_into(&mut output) {
                Ok(()) => {
                    assert_eq!(end, data.len());
                    assert_eq!(output, expected);
                }
                Err(error) => {
                    assert!(error.is_recoverable_eof(), "visible {end}: {error:?}");
                    let stable = decoder.decoded_output_bytes().unwrap_or(0);
                    assert_eq!(&output[..stable], &expected[..stable], "visible {end}");
                    if let Some((width, _)) = decoder.dimensions() {
                        assert_eq!(stable % (width * 3), 0);
                    }
                }
            }
        }
    }
}

#[test]
fn an_empty_restart_interval_does_not_terminate_later_data() {
    let data = empty_restart();
    let output =
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, false))
            .decode()
            .unwrap();
    assert!(output[..16 * 8].iter().all(|&sample| sample == 128));
    for row in output[16 * 8..].chunks_exact(16) {
        assert_eq!(&row[..8], &[132; 8]);
        assert_eq!(&row[8..], &[136; 8]);
    }
}

#[test]
fn short_restart_intervals_fill_only_their_missing_mcus_and_resume() {
    let data = short_restart();
    let output =
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::Luma, false))
            .decode()
            .unwrap();
    for (mcu, &expected) in [132, 136, 136, 128, 132, 136, 140, 144].iter().enumerate() {
        let y = mcu / 2 * 8;
        let x = mcu % 2 * 8;
        for row in 0..8 {
            assert!(output[(y + row) * 16 + x..][..8]
                .iter()
                .all(|&sample| sample == expected));
        }
    }
}

#[test]
fn eoi_preserves_the_partial_mcu_then_neutralizes_the_same_row_and_tail() {
    let data = early_eoi(3, Some(1), 3);
    for color in [
        ColorSpace::RGB,
        ColorSpace::RGBA,
        ColorSpace::BGR,
        ColorSpace::BGRA
    ] {
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&data), options(color, false));
        let output = decoder.decode().unwrap();
        let channels = color.num_components();
        assert_eq!(output.len(), 32 * 24 * channels);
        for (index, pixel) in output.chunks_exact(channels).enumerate() {
            let (x, y) = (index % 32, index / 32);
            let (r, g, b) = if y < 8 && x < 8 {
                (138, 128, 139)
            } else if y < 8 && x < 16 {
                (142, 132, 143)
            } else {
                (128, 128, 128)
            };
            let expected = if matches!(color, ColorSpace::BGR | ColorSpace::BGRA) {
                [b, g, r, 255]
            } else {
                [r, g, b, 255]
            };
            assert_eq!(pixel, &expected[..channels], "{x}/{y}, {color:?}");
        }
        assert!(
            JpegDecoder::new_with_options(ZCursor::new(&data), options(color, true))
                .decode()
                .is_err()
        );
    }
}

#[test]
fn missing_component_samples_are_neutral_in_raw_output() {
    let data = early_eoi(3, Some(1), 3);
    let mut decoder = JpegDecoder::new(ZCursor::new(&data));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let layout = raw.layout().unwrap();
    let mut planes: Vec<Vec<u8>> = layout[..3]
        .iter()
        .map(|p| vec![0xa5; p.byte_size])
        .collect();
    let mut slices: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut slices).unwrap();
    for component in 0..3 {
        for y in 0..24 {
            for x in 0..32 {
                let expected = if y < 8 && x < 8 {
                    132
                } else if y < 8 && x < 16 {
                    if component == 0 {
                        136
                    } else {
                        132
                    }
                } else {
                    128
                };
                assert_eq!(
                    planes[component][y * layout[component].stride + x],
                    expected
                );
            }
        }
    }
}

#[test]
fn neutral_recovery_uses_existing_cmyk_and_ycck_output_semantics() {
    for transform in [None, Some(2)] {
        let data = early_eoi(4, transform, 3);
        for color in [ColorSpace::RGB, ColorSpace::RGBA, ColorSpace::CMYK] {
            let mut decoder =
                JpegDecoder::new_with_options(ZCursor::new(&data), options(color, false));
            let output = decoder.decode().unwrap();
            let channels = color.num_components();
            for pixel in output[32 * 8 * channels..].chunks_exact(channels) {
                let expected = match (transform, color) {
                    (_, ColorSpace::RGB) => [64, 64, 64, 255],
                    (_, ColorSpace::RGBA) => [64, 64, 64, 255],
                    (Some(2), ColorSpace::CMYK) => [127, 127, 127, 128],
                    _ => [128; 4]
                };
                assert_eq!(pixel, &expected[..channels]);
            }
        }
    }
}

#[test]
fn cancelling_and_retrying_a_tall_neutral_tail_preserves_complete_rows() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let data = early_eoi(3, Some(1), 7500);
    let polls = Arc::new(AtomicUsize::new(0));
    let mut decoder =
        JpegDecoder::new_with_options(ZCursor::new(&data), options(ColorSpace::RGBA, false));
    decoder.decode_headers().unwrap();
    decoder.set_cancel_interval(1);
    decoder.set_cancel({
        let polls = Arc::clone(&polls);
        move || polls.fetch_add(1, Ordering::Relaxed) >= 12
    });
    let mut output = vec![0xa5; 32 * 60000 * 4];
    let error = decoder.decode_into(&mut output).unwrap_err();
    assert!(matches!(error, zune_jpeg::errors::DecodeErrors::Cancelled));
    let stable = decoder.decoded_output_bytes().unwrap();
    assert!(stable > 32 * 8 * 4 && stable < output.len());
    assert_eq!(stable % (32 * 4), 0);
    assert!(output[32 * 8 * 4..stable]
        .chunks_exact(4)
        .all(|p| p == [128, 128, 128, 255]));
    decoder.set_cancel(|| false);
    decoder.decode_into(&mut output).unwrap();
    assert_eq!(output.len(), 32 * 60000 * 4);
    assert!(output[32 * 8 * 4..]
        .chunks_exact(4)
        .all(|p| p == [128, 128, 128, 255]));
}
