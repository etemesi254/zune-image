#![no_main]

use libfuzzer_sys::fuzz_target;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

const MAX_INPUT_LEN: usize = 1 << 20;
const MAX_OUTPUT_LEN: usize = 64 * 1024 * 1024;

fn options(data: &[u8]) -> DecoderOptions {
    let colorspaces = [
        ColorSpace::RGB,
        ColorSpace::RGBA,
        ColorSpace::BGR,
        ColorSpace::BGRA,
        ColorSpace::Luma,
        ColorSpace::YCbCr,
    ];
    DecoderOptions::default()
        .set_strict_mode(data.len() % 2 != 0)
        .set_max_width(4_096)
        .set_max_height(4_096)
        .jpeg_set_max_scans(256)
        .jpeg_set_out_colorspace(colorspaces[data.len() % colorspaces.len()])
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 || data.len() > MAX_INPUT_LEN {
        return;
    }

    match data.len() % 5 {
        0 => {
            let decoder_options = options(data);
            let mut probe = JpegDecoder::new_with_options(ZCursor::new(data), decoder_options);
            if probe.decode_headers().is_err()
                || probe
                    .output_buffer_size()
                    .is_none_or(|size| size > MAX_OUTPUT_LEN)
            {
                return;
            }
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), decoder_options);
            let _ = decoder.decode();
        }
        1 => {
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options(data));
            if decoder.decode_headers().is_err() {
                return;
            }
            let Some(size) = decoder.output_buffer_size() else {
                return;
            };
            if size > MAX_OUTPUT_LEN {
                return;
            }
            let mut output = vec![0; size];
            let _ = decoder.decode_into(&mut output);
        }
        2 => {
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options(data));
            if decoder.decode_headers().is_err() {
                return;
            }
            let Some(size) = decoder.output_buffer_size() else {
                return;
            };
            if size == 0 || size > MAX_OUTPUT_LEN {
                return;
            }
            let mut output = vec![0; size - 1];
            let _ = decoder.decode_into(&mut output);
        }
        3 => {
            let mut decoder = JpegDecoder::new(ZCursor::new(data));
            if decoder.decode_headers().is_err() {
                return;
            }
            let mut raw = decoder.raw_output();
            let Some(layout) = raw.layout() else {
                return;
            };
            let Some(count) = raw.num_components() else {
                return;
            };
            let total = layout[..count]
                .iter()
                .try_fold(0usize, |sum, plane| sum.checked_add(plane.byte_size));
            if total.is_none_or(|size| size > MAX_OUTPUT_LEN) {
                return;
            }
            let mut storage: Vec<Vec<u8>> = layout[..count]
                .iter()
                .map(|plane| vec![0; plane.byte_size])
                .collect();
            let mut planes: Vec<&mut [u8]> = storage.iter_mut().map(Vec::as_mut_slice).collect();
            let _ = raw.decode_into_planes(&mut planes);
        }
        _ => {
            let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options(data));
            if decoder.decode_headers().is_err() {
                return;
            }
            let Some(size) = decoder.output_buffer_size() else {
                return;
            };
            if size > MAX_OUTPUT_LEN {
                return;
            }
            let mut first = vec![0; size];
            if decoder.decode_into(&mut first).is_err() {
                return;
            }
            let mut second = vec![0; size];
            decoder
                .decode_into(&mut second)
                .expect("successful decode could not be replayed");
            assert_eq!(first, second, "successful decode replay changed pixels");
        }
    }
});
