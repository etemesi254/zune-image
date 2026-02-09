/*
 * Copyright (c) 2026.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! Tests for non-interleaved JPEG decoding.
//!
//! Non-interleaved JPEGs have separate SOS (Start Of Scan) markers for each
//! color component, rather than interleaving all components in a single scan.
//! This is valid per ITU-T.81 (JPEG standard) Section B.2.3.
//!
//! These images are created with tools like libjpeg's cjpeg -scans option.

use std::borrow::Cow;
use std::panic;

use dicom::core::DicomValue;
use dicom::dictionary_std::tags;
use dicom::object::InMemDicomObject;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;
use zune_jpeg::JpegDecoder;

fn get_pixeldata<'a>(dcm: &'a InMemDicomObject, frame_number: u32) -> Cow<'a, [u8]> {
    let pixeldata = dcm
        .element(tags::PIXEL_DATA)
        .expect("Missing PixelData element");
    match pixeldata.value() {
        DicomValue::PixelSequence(seq) => {
            let number_of_frames = match dcm.get(tags::NUMBER_OF_FRAMES) {
                Some(elem) => elem.to_int::<u32>().unwrap_or_else(|e| {
                    panic!("Invalid Number of Frames: {}", e);
                    1
                }),
                None => 1
            };

            if number_of_frames as usize == seq.fragments().len() {
                // frame-to-fragment mapping is 1:1

                // get fragment containing our frame
                let fragment = seq
                    .fragments()
                    .get(frame_number as usize)
                    .expect("Frame number exceeds available fragments");

                Cow::Borrowed(&fragment[..])
            } else {
                // In this case we look up the basic offset table
                // and gather all of the frame's fragments in a single vector.
                // Note: not the most efficient way to do this,
                // consider optimizing later with byte chunk readers
                let offset_table = seq.offset_table();
                let base_offset = offset_table.get(frame_number as usize).copied();
                let base_offset = if frame_number == 0 {
                    base_offset.unwrap_or(0) as usize
                } else {
                    base_offset.expect("Missing offset entry for frame") as usize
                };
                let next_offset = offset_table.get(frame_number as usize + 1);

                let mut offset = 0;
                let mut frame_data = Vec::new();
                for fragment in seq.fragments() {
                    // include it
                    if offset >= base_offset {
                        frame_data.extend_from_slice(fragment);
                    }
                    offset += fragment.len() + 8;
                    if let Some(&next_offset) = next_offset {
                        if offset >= next_offset as usize {
                            // next fragment is for the next frame
                            break;
                        }
                    }
                }

                Cow::Owned(frame_data)
            }
        }
        DicomValue::Primitive(v) => {
            // grab the intended slice based on image properties

            let get_int_property = |tag, name| {
                dcm.get(tag)
                    .expect("Missing property")
                    .to_int::<usize>()
                    .expect("Invalid property value")
            };

            let rows = get_int_property(tags::ROWS, "Rows");
            let columns = get_int_property(tags::COLUMNS, "Columns");
            let samples_per_pixel = get_int_property(tags::SAMPLES_PER_PIXEL, "Samples Per Pixel");
            let bits_allocated = get_int_property(tags::BITS_ALLOCATED, "Bits Allocated");
            let frame_size = rows * columns * samples_per_pixel * ((bits_allocated + 7) / 8);

            let frame = frame_number as usize;
            let mut data = v.to_bytes();
            match &mut data {
                Cow::Borrowed(data) => {
                    *data = data
                        .get((frame_size * frame)..(frame_size * (frame + 1)))
                        .expect("Frame number exceeds available fragments");
                }
                Cow::Owned(data) => {
                    *data = data
                        .get((frame_size * frame)..(frame_size * (frame + 1)))
                        .expect("Frame number exceeds available fragments")
                        .to_vec();
                }
            }
            data
        }
        _ => {
            panic!("Unsupported PixelData format: expected PixelSequence or Primitive")
        }
    }
}

/// Test decoding a 12-bit lossy JPEG image stored in a DICOM file.
#[test]
fn decode_dicom_lossy_extended_12bit() {
    let test_data = include_bytes!("../../../test-images/jpeg/dicom/JPLY/MG1_JPLY");

    // Extract pixeldata from the DICOM file
    let dicom = dicom::object::from_reader(&mut ZCursor::new(test_data))
        .expect("Failed to read DICOM file");
    let pixel_data = get_pixeldata(&dicom, 0);

    let options =
        zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(pixel_data), options);
    let pixels = decoder.decode().expect(
        "Failed to decode 3064x4664 non-interleaved JPEG - \
         decoder likely doesn't handle DHT markers between scans"
    );

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 3064, "Expected width 3064");
    assert_eq!(info.height, 4664, "Expected height 4664");
    assert_eq!(pixels.len(), 2 * 4664 * 3064, "Unexpected pixel count");

    // Write out the image for visual verification
    let encoder = zune_ppm::PPMEncoder::new(
        &pixels,
        EncoderOptions::new(
            info.width as usize,
            info.height as usize,
            ColorSpace::Luma,
            BitDepth::Custom(info.pixel_density)
        )
    );

    encoder
        .encode(std::fs::File::create("output/dicom_lossy_extended_12bit.ppm").unwrap())
        .unwrap();
}

/// Test decoding a 10-bit lossy JPEG image stored in a DICOM
#[test]
fn decode_dicom_lossy_extended_10bit() {
    let test_data = include_bytes!("../../../test-images/jpeg/dicom/JPLY/RG2_JPLY");

    // Extract pixeldata from the DICOM file
    let dicom = dicom::object::from_reader(&mut ZCursor::new(test_data))
        .expect("Failed to read DICOM file");
    let pixel_data = get_pixeldata(&dicom, 0);

    let options =
        zune_core::options::DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);

    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(pixel_data), options);
    let pixels = decoder.decode().expect(
        "Failed to decode 1760x2140 non-interleaved JPEG - \
         decoder likely doesn't handle DHT markers between scans"
    );

    let info = decoder.info().expect("Failed to get image info");
    assert_eq!(info.width, 1760, "Expected width 1760");
    assert_eq!(info.height, 2140, "Expected height 2140");
    assert_eq!(pixels.len(), 2 * 2140 * 1760, "Unexpected pixel count");

    // Write out the image for visual verification
    let encoder = zune_ppm::PPMEncoder::new(
        &pixels,
        EncoderOptions::new(
            info.width as usize,
            info.height as usize,
            ColorSpace::Luma,
            BitDepth::Custom(info.pixel_density)
        )
    );

    encoder
        .encode(std::fs::File::create("output/dicom_lossy_extended_10bit.ppm").unwrap())
        .unwrap();
}
