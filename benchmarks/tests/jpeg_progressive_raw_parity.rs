use std::fs::read;

use zune_benches::sample_path;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::{JpegDecoder, PlaneInfo};

fn decode_zune(data: &[u8]) -> (Vec<Vec<u8>>, [PlaneInfo; 4], Vec<u8>) {
    let mut decoder = JpegDecoder::new(ZCursor::new(data));
    decoder.decode_headers().unwrap();
    let mut raw = decoder.raw_output();
    let count = raw.num_components().unwrap();
    let ids = raw.component_ids().unwrap();
    let layout = raw.layout().unwrap();
    let mut planes: Vec<Vec<u8>> = layout[..count]
        .iter()
        .map(|plane| vec![0; plane.byte_size])
        .collect();
    let mut refs: Vec<&mut [u8]> = planes.iter_mut().map(Vec::as_mut_slice).collect();
    raw.decode_into_planes(&mut refs).unwrap();
    (planes, layout, ids)
}

#[test]
fn progressive_raw_planes_match_libjpeg_turbo() {
    for (name, fixture) in [
        ("420", "progressive_420_webcodecs.jpg"),
        ("422", "progressive_422_65x67.jpg"),
        ("444", "progressive_444_65x67.jpg")
    ] {
        let data = read(sample_path().join("test-images/jpeg").join(fixture)).unwrap();
        let (actual, layout, actual_ids) = decode_zune(&data);
        let reference = turbojpeg::decompress_to_yuv(&data).unwrap();
        assert_eq!(actual_ids, [1, 2, 3], "{name}");

        let y_stride = reference.y_width();
        let y_height = reference.y_height();
        let uv_stride = reference.uv_width();
        let uv_height = reference.uv_height();
        let y_len = y_stride * y_height;
        let uv_len = uv_stride * uv_height;
        let reference_planes = [
            (&reference.pixels[..y_len], y_stride),
            (&reference.pixels[y_len..y_len + uv_len], uv_stride),
            (
                &reference.pixels[y_len + uv_len..y_len + 2 * uv_len],
                uv_stride
            )
        ];

        for index in 0..actual.len() {
            let mut maximum_delta = 0;
            for row in 0..layout[index].height {
                for column in 0..layout[index].width {
                    let actual_sample = actual[index][row * layout[index].stride + column];
                    let (plane, stride) = reference_planes[index];
                    let reference_sample = plane[row * stride + column];
                    maximum_delta = maximum_delta.max(actual_sample.abs_diff(reference_sample));
                }
            }
            assert!(
                maximum_delta <= 2,
                "{name}, component {index}: maximum delta {maximum_delta} exceeds 2"
            );
        }
    }
}
