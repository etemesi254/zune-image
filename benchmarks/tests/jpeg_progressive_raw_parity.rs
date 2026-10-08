use std::fs::read;

use mozjpeg::CompInfoExt;
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

fn decode_libjpeg(data: &[u8]) -> (Vec<Vec<u8>>, Vec<usize>, Vec<u8>, Vec<(u8, u8)>) {
    let decoder = mozjpeg::Decompress::with_markers(mozjpeg::ALL_MARKERS)
        .from_mem(data)
        .unwrap();
    let mut image = decoder.raw().unwrap();
    let strides: Vec<_> = image
        .components()
        .iter()
        .map(CompInfoExt::row_stride)
        .collect();
    let ids = image
        .components()
        .iter()
        .map(|component| component.component_id as u8)
        .collect();
    let sampling = image
        .components()
        .iter()
        .map(CompInfoExt::sampling)
        .collect();
    let mut planes = vec![Vec::new(); image.components().len()];
    let mut refs: Vec<&mut Vec<u8>> = planes.iter_mut().collect();
    image.read_raw_data(&mut refs);
    image.finish().unwrap();
    (planes, strides, ids, sampling)
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
        let (reference, reference_strides, reference_ids, reference_sampling) =
            decode_libjpeg(&data);
        assert_eq!(actual_ids, reference_ids, "{name}");
        assert_eq!(actual.len(), reference.len(), "{name}");

        for index in 0..actual.len() {
            assert_eq!(
                (
                    layout[index].horizontal_sampling_factor as u8,
                    layout[index].vertical_sampling_factor as u8
                ),
                reference_sampling[index],
                "{name}, component {index}"
            );
            let mut maximum_delta = 0;
            for row in 0..layout[index].height {
                for column in 0..layout[index].width {
                    let actual_sample = actual[index][row * layout[index].stride + column];
                    let reference_sample =
                        reference[index][row * reference_strides[index] + column];
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
