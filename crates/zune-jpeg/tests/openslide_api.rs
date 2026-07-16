use std::io::Cursor;

use zune_core::colorspace::ColorSpace;
use zune_core::options::{DecoderOptions, InputColorspaceOverride, JpegScale};
use zune_jpeg::{
    assemble_split_jpeg, DecodeRegion, JpegDecoder, JpegDimensions, RegionDecodeMode
};

/// Copy a tightly packed rectangular region out of a full decoded image.
fn crop_from_full(
    full: &[u8], image_width: usize, components: usize, region: DecodeRegion,
) -> Vec<u8> {
    let full_stride = image_width * components;
    let region_stride = region.width * components;
    let mut out = vec![0; region_stride * region.height];

    for row in 0..region.height {
        let src_start = (region.y + row) * full_stride + region.x * components;
        let dst_start = row * region_stride;
        out[dst_start..dst_start + region_stride]
            .copy_from_slice(&full[src_start..src_start + region_stride]);
    }

    out
}

/// Build an expected scaled image by conservatively sampling a full decode.
fn scaled_from_full(
    full: &[u8], width: usize, height: usize, components: usize, scale: JpegScale,
) -> Vec<u8> {
    let denominator = scale.denominator();
    let scaled_width = width.div_ceil(denominator);
    let scaled_height = height.div_ceil(denominator);
    let full_stride = width * components;
    let scaled_stride = scaled_width * components;
    let mut out = vec![0; scaled_stride * scaled_height];

    for y in 0..scaled_height {
        let src_y = (y * denominator).min(height - 1);
        for x in 0..scaled_width {
            let src_x = (x * denominator).min(width - 1);
            let src = src_y * full_stride + src_x * components;
            let dst = y * scaled_stride + x * components;
            out[dst..dst + components].copy_from_slice(&full[src..src + components]);
        }
    }

    out
}

/// Find the offset of the first JPEG marker whose code is in `markers`.
fn find_marker(data: &[u8], markers: &[u8]) -> Option<usize> {
    data.windows(2).position(|window| window[0] == 0xff && markers.contains(&window[1]))
}

/// Return the byte offset where entropy-coded data starts after the first SOS marker.
fn sos_entropy_start(data: &[u8]) -> Option<usize> {
    let sos = find_marker(data, &[0xda])?;
    let length = u16::from_be_bytes([*data.get(sos + 2)?, *data.get(sos + 3)?]) as usize;
    sos.checked_add(2)?.checked_add(length).filter(|offset| *offset <= data.len())
}

/// Construct a minimal baseline grayscale JPEG with a zero DC coefficient block.
fn baseline_luma_8x8_dc_zero_jpeg() -> Vec<u8> {
    let mut jpeg = vec![0xff, 0xd8];

    jpeg.extend_from_slice(&[0xff, 0xdb, 0x00, 0x43, 0x00]);
    jpeg.extend_from_slice(&[1; 64]);

    jpeg.extend_from_slice(&[
        0xff, 0xc0, 0x00, 0x0b, 0x08, 0x00, 0x08, 0x00, 0x08, 0x01, 0x01, 0x11, 0x00,
    ]);

    jpeg.extend_from_slice(&[
        0xff, 0xc4, 0x00, 0x14, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    jpeg.extend_from_slice(&[
        0xff, 0xc4, 0x00, 0x14, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);

    jpeg.extend_from_slice(&[
        0xff, 0xda, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3f, 0x00, 0x03, 0xff, 0xd9,
    ]);
    jpeg
}

#[test]
/// Check that full-scale region decode matches cropping a full decode.
fn decode_region_matches_full_decode_crop() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let region = DecodeRegion {
        x: 7,
        y: 11,
        width: 23,
        height: 19,
    };

    let mut full_decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    let full = full_decoder.decode().unwrap();
    let info = full_decoder.info().unwrap();
    let expected = crop_from_full(&full, info.width as usize, 3, region);

    let mut region_decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    region_decoder.decode_headers().unwrap();
    assert_eq!(
        region_decoder.region_output_buffer_size(region),
        Some(region.width * region.height * 3)
    );

    let decoded = region_decoder
        .decode_region(region, RegionDecodeMode::Conservative)
        .unwrap();
    assert_eq!(decoded, expected);
}

#[test]
/// Check region decode across representative sampling modes.
fn decode_region_matches_full_decode_crop_for_sampling_modes() {
    let cases: &[(&[u8], DecodeRegion)] = &[
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg"),
            DecodeRegion {
                x:      3,
                y:      4,
                width:  17,
                height: 13
            }
        ),
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_422_64x64.jpg"),
            DecodeRegion {
                x:      5,
                y:      7,
                width:  19,
                height: 11
            }
        ),
        (
            include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg"),
            DecodeRegion {
                x:      9,
                y:      10,
                width:  21,
                height: 15
            }
        ),
        (
            include_bytes!("../../../test-images/jpeg/down_sampled_grayscale_prog.jpg"),
            DecodeRegion {
                x:      2,
                y:      6,
                width:  12,
                height: 10
            }
        )
    ];

    for (image, region) in cases {
        let mut full_decoder = JpegDecoder::new(Cursor::new(*image));
        let full = full_decoder.decode().unwrap();
        let info = full_decoder.info().unwrap();
        let components = full_decoder.output_colorspace().unwrap().num_components();
        let expected = crop_from_full(&full, info.width as usize, components, *region);

        let mut region_decoder = JpegDecoder::new(Cursor::new(*image));
        let decoded = region_decoder
            .decode_region(*region, RegionDecodeMode::BestEffort)
            .unwrap();
        assert_eq!(decoded, expected);
    }
}

#[test]
/// Check interleaved baseline region decode reports region-sized output.
fn baseline_interleaved_region_decode_uses_region_sized_output_accounting() {
    let image = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let region = DecodeRegion {
        x:      4,
        y:      5,
        width:  16,
        height: 12
    };

    let mut decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    let decoded = decoder
        .decode_region(region, RegionDecodeMode::BestEffort)
        .unwrap();

    assert_eq!(decoded.len(), region.width * region.height * 3);
    assert_eq!(
        decoder.decoded_output_bytes(),
        Some(region.width * region.height * 3)
    );
}

#[test]
/// Check multi-SOS baseline region decode reports region-sized output.
fn baseline_multi_sos_region_decode_uses_region_sized_output_accounting() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let region = DecodeRegion {
        x:      6,
        y:      9,
        width:  18,
        height: 14
    };

    let mut decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    let decoded = decoder
        .decode_region(region, RegionDecodeMode::BestEffort)
        .unwrap();

    assert_eq!(decoded.len(), region.width * region.height * 3);
    assert_eq!(
        decoder.decoded_output_bytes(),
        Some(region.width * region.height * 3)
    );
}

#[test]
/// Check BGR and BGRA region decode against full-decode crops.
fn decode_region_matches_full_decode_crop_for_bgr_and_bgra() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let region = DecodeRegion {
        x:      8,
        y:      5,
        width:  16,
        height: 12
    };

    for colorspace in [ColorSpace::BGR, ColorSpace::BGRA] {
        let options = DecoderOptions::default().jpeg_set_out_colorspace(colorspace);
        let mut full_decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let full = full_decoder.decode().unwrap();
        let info = full_decoder.info().unwrap();
        let components = colorspace.num_components();
        let expected = crop_from_full(&full, info.width as usize, components, region);

        let mut region_decoder =
            JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let decoded = region_decoder
            .decode_region(region, RegionDecodeMode::Conservative)
            .unwrap();
        assert_eq!(decoded, expected);
    }
}

#[test]
/// Check reduced grayscale baseline decode uses the scaled IDCT path.
fn scaled_luma_baseline_decode_uses_reduced_idct_path() {
    let image = baseline_luma_8x8_dc_zero_jpeg();

    for scale in [JpegScale::Half, JpegScale::Quarter, JpegScale::Eighth] {
        let options = DecoderOptions::default()
            .jpeg_set_out_colorspace(ColorSpace::Luma)
            .jpeg_set_scale(scale);
        let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        decoder.decode_headers().unwrap();
        let expected_size = 8_usize.div_ceil(scale.denominator()).pow(2);
        assert_eq!(decoder.output_buffer_size(), Some(expected_size));

        let scaled = decoder.decode().unwrap();

        assert_eq!(scaled, vec![128; expected_size]);
        assert_eq!(decoder.decoded_output_bytes(), Some(expected_size));
        assert_eq!(
            decoder.decoded_scanlines(),
            Some(8_usize.div_ceil(scale.denominator()))
        );
    }
}

#[test]
/// Check scaled decode matches conservative sampling from a full decode.
fn scaled_decode_matches_conservative_full_decode_sampling() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");

    let mut full_decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    let full = full_decoder.decode().unwrap();
    let info = full_decoder.info().unwrap();

    for scale in [JpegScale::Half, JpegScale::Quarter, JpegScale::Eighth] {
        let options = DecoderOptions::default().jpeg_set_scale(scale);
        let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        decoder.decode_headers().unwrap();
        let scaled_width = (info.width as usize).div_ceil(scale.denominator());
        let scaled_height = (info.height as usize).div_ceil(scale.denominator());
        assert_eq!(
            decoder.output_buffer_size(),
            Some(scaled_width * scaled_height * 3)
        );

        let scaled = decoder.decode().unwrap();
        let expected = scaled_from_full(
            &full,
            info.width as usize,
            info.height as usize,
            3,
            scale
        );
        assert_eq!(scaled, expected);
    }
}

#[test]
/// Check scaled color decode and region decode produce consistent RGB output.
fn scaled_interleaved_baseline_color_decode_and_region_are_consistent() {
    let image = include_bytes!("../../../test-images/jpeg/fox410.jpg");

    for scale in [JpegScale::Half, JpegScale::Quarter, JpegScale::Eighth] {
        let options = DecoderOptions::default()
            .jpeg_set_input_colorspace_override(InputColorspaceOverride::Force(ColorSpace::YCbCr))
            .jpeg_set_scale(scale);
        let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let scaled = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        let scaled_width = (info.width as usize).div_ceil(scale.denominator());
        let scaled_height = (info.height as usize).div_ceil(scale.denominator());
        assert_eq!(scaled.len(), scaled_width * scaled_height * 3);
        assert_eq!(decoder.decoded_output_bytes(), Some(scaled.len()));

        let x = 3.min(scaled_width - 1);
        let y = 4.min(scaled_height - 1);
        let region = DecodeRegion {
            x,
            y,
            width:  11.min(scaled_width - x),
            height: 9.min(scaled_height - y)
        };
        let expected = crop_from_full(&scaled, scaled_width, 3, region);
        let mut region_decoder =
            JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let region_pixels = region_decoder
            .decode_region(region, RegionDecodeMode::BestEffort)
            .unwrap();

        assert_eq!(region_pixels, expected);
    }
}

#[test]
/// Check scaled BGR/BGRA region decode is consistent with scaled full decode.
fn scaled_interleaved_baseline_bgr_and_bgra_decode_regions_are_consistent() {
    let image = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let scale = JpegScale::Quarter;

    for colorspace in [ColorSpace::BGR, ColorSpace::BGRA] {
        let options = DecoderOptions::default()
            .jpeg_set_input_colorspace_override(InputColorspaceOverride::Force(ColorSpace::YCbCr))
            .jpeg_set_out_colorspace(colorspace)
            .jpeg_set_scale(scale);
        let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let scaled = decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        let scaled_width = (info.width as usize).div_ceil(scale.denominator());
        let scaled_height = (info.height as usize).div_ceil(scale.denominator());
        let components = colorspace.num_components();
        assert_eq!(scaled.len(), scaled_width * scaled_height * components);

        let region = DecodeRegion {
            x:      5,
            y:      6,
            width:  13,
            height: 10
        };
        let expected = crop_from_full(&scaled, scaled_width, components, region);
        let mut region_decoder =
            JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
        let region_pixels = region_decoder
            .decode_region(region, RegionDecodeMode::BestEffort)
            .unwrap();

        assert_eq!(region_pixels, expected);
    }
}

#[test]
/// Check scaled region decode matches cropping the scaled full image.
fn scaled_region_decode_matches_scaled_full_decode_crop() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_420_64x64.jpg");
    let options = DecoderOptions::default().jpeg_set_scale(JpegScale::Quarter);
    let region = DecodeRegion {
        x:      2,
        y:      3,
        width:  9,
        height: 7
    };

    let mut full_decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
    let scaled = full_decoder.decode().unwrap();
    let expected = crop_from_full(&scaled, 16, 3, region);

    let mut region_decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
    let region_pixels = region_decoder
        .decode_region(region, RegionDecodeMode::BestEffort)
        .unwrap();
    assert_eq!(region_pixels, expected);
    assert_eq!(
        region_decoder.decoded_output_bytes(),
        Some(region.width * region.height * 3)
    );
}

#[test]
/// Check scaled decode reports stable scanlines in scaled output coordinates.
fn scaled_decode_reports_scaled_stable_scanlines() {
    let image = include_bytes!("../../../test-images/jpeg/non_interleaved_422_65x65.jpg");
    let scale = JpegScale::Eighth;
    let options = DecoderOptions::default().jpeg_set_scale(scale);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
    decoder.decode_headers().unwrap();
    let info = decoder.info().unwrap();
    let scaled_width = (info.width as usize).div_ceil(scale.denominator());
    let scaled_height = (info.height as usize).div_ceil(scale.denominator());
    let expected_size = scaled_width * scaled_height * 3;
    let mut out = vec![0; expected_size];

    decoder.decode_into(&mut out).unwrap();

    assert_eq!(decoder.decoded_output_bytes(), Some(expected_size));
    assert_eq!(decoder.decoded_scanlines(), Some(scaled_height));
}

#[test]
/// Check region decode rejects output buffers smaller than the region size.
fn decode_region_into_rejects_small_output() {
    let image = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let region = DecodeRegion {
        x: 2,
        y: 3,
        width: 6,
        height: 5,
    };

    let mut decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    decoder.decode_headers().unwrap();
    let required = decoder.region_output_buffer_size(region).unwrap();
    let mut too_small = vec![0; required - 1];

    assert!(decoder
        .decode_region_into(region, RegionDecodeMode::BestEffort, &mut too_small)
        .is_err());
}

#[test]
/// Check empty and out-of-bounds regions are rejected.
fn decode_region_rejects_empty_and_out_of_bounds_regions() {
    let image = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let mut decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    decoder.decode_headers().unwrap();

    assert_eq!(
        decoder.region_output_buffer_size(DecodeRegion {
            x: 0,
            y: 0,
            width: 0,
            height: 1
        }),
        None
    );
    assert_eq!(
        decoder.region_output_buffer_size(DecodeRegion {
            x: 15,
            y: 15,
            width: 2,
            height: 1
        }),
        None
    );
}

#[test]
/// Check input colorspace override is applied after marker parsing.
fn input_colorspace_override_is_applied_after_header_markers() {
    let image = include_bytes!("../../../test-images/jpeg/four_components.jpg");
    let options = DecoderOptions::default()
        .jpeg_set_input_colorspace_override(InputColorspaceOverride::Force(ColorSpace::YCbCr));

    let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);
    decoder.decode_headers().unwrap();

    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));
    assert_eq!(
        decoder.options().jpeg_get_input_colorspace_override(),
        InputColorspaceOverride::Force(ColorSpace::YCbCr)
    );
}

#[test]
/// Check input colorspace override survives repeated decode calls.
fn input_colorspace_override_survives_decode_into_replay() {
    let image = include_bytes!("../../../test-images/jpeg/fox410.jpg");
    let options = DecoderOptions::default()
        .jpeg_set_input_colorspace_override(InputColorspaceOverride::Force(ColorSpace::YCbCr));
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(image.as_slice()), options);

    decoder.decode_headers().unwrap();
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));

    let mut first = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut first).unwrap();
    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));

    let mut replay = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.decode_into(&mut replay).unwrap();

    assert_eq!(decoder.input_colorspace(), Some(ColorSpace::YCbCr));
    assert_eq!(replay, first);
}

#[test]
/// Check split JPEG assembly patches SOF dimensions and appends EOI.
fn assemble_split_jpeg_patches_sof_dimensions_and_appends_eoi() {
    let header = [
        0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03
    ];
    let data = [0x11, 0x22, 0x33];
    let mut out = vec![0xaa, 0xbb];

    assemble_split_jpeg(
        &header,
        &data,
        2,
        JpegDimensions {
            width:  513,
            height: 258
        },
        &mut out
    )
    .unwrap();

    assert_eq!(&out[..2], &[0xff, 0xd8]);
    assert_eq!(&out[7..9], &258u16.to_be_bytes());
    assert_eq!(&out[9..11], &513u16.to_be_bytes());
    assert_eq!(&out[header.len()..header.len() + data.len()], &data);
    assert!(out.ends_with(&[0xff, 0xd9]));
}

#[test]
/// Check split JPEG assembly does not append a duplicate EOI marker.
fn assemble_split_jpeg_does_not_duplicate_existing_eoi() {
    let header = [
        0xff, 0xd8, 0xff, 0xc2, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03
    ];
    let data = [0x11, 0x22, 0xff, 0xd9];
    let mut out = Vec::new();

    assemble_split_jpeg(
        &header,
        &data,
        2,
        JpegDimensions {
            width:  1,
            height: 1
        },
        &mut out
    )
    .unwrap();

    assert_eq!(&out[out.len() - 4..], &[0x11, 0x22, 0xff, 0xd9]);
}

#[test]
/// Check split JPEG assembly rejects unsupported SOF offsets.
fn assemble_split_jpeg_rejects_invalid_sof_offset() {
    let header = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x02, 0x00, 0x00, 0x00];
    let mut out = Vec::new();

    assert!(assemble_split_jpeg(
        &header,
        &[],
        2,
        JpegDimensions {
            width:  1,
            height: 1
        },
        &mut out
    )
    .is_err());
    assert!(out.is_empty());
}

#[test]
/// Check split JPEG assembly rejects zero dimensions without mutating output.
fn assemble_split_jpeg_rejects_zero_dimensions_without_mutating_output() {
    let header = [
        0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01, 0x03
    ];
    let original = vec![0xaa, 0xbb, 0xcc];
    let mut out = original.clone();

    assert!(assemble_split_jpeg(
        &header,
        &[],
        2,
        JpegDimensions {
            width:  0,
            height: 1
        },
        &mut out
    )
    .is_err());
    assert_eq!(out, original);

    assert!(assemble_split_jpeg(
        &header,
        &[],
        2,
        JpegDimensions {
            width:  1,
            height: 0
        },
        &mut out
    )
    .is_err());
    assert_eq!(out, original);
}

#[test]
/// Check an assembled split JPEG decodes identically to the original fixture.
fn assemble_split_jpeg_reassembled_stream_decodes_like_original() {
    let image = include_bytes!("../../../test-images/jpeg/tiny_non_interleaved_444.jpg");
    let sof_offset = find_marker(image, &[0xc0, 0xc1, 0xc2]).unwrap();
    let entropy_start = sos_entropy_start(image).unwrap();
    let original_without_eoi = image
        .strip_suffix(&[0xff, 0xd9])
        .expect("fixture should end with EOI");
    let data = &original_without_eoi[entropy_start..];
    let mut assembled = Vec::new();

    assemble_split_jpeg(
        &image[..entropy_start],
        data,
        sof_offset,
        JpegDimensions {
            width:  16,
            height: 16
        },
        &mut assembled
    )
    .unwrap();

    let mut original_decoder = JpegDecoder::new(Cursor::new(image.as_slice()));
    let original_pixels = original_decoder.decode().unwrap();
    let mut assembled_decoder = JpegDecoder::new(Cursor::new(assembled.as_slice()));
    let assembled_pixels = assembled_decoder.decode().unwrap();

    assert_eq!(assembled_decoder.info().unwrap().width, 16);
    assert_eq!(assembled_decoder.info().unwrap().height, 16);
    assert_eq!(assembled_pixels, original_pixels);
}
