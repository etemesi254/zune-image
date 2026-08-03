use std::io::Cursor;
use zune_jpeg::JpegDecoder;

#[test]
fn test_roi_decoding() {
    let image: &[u8] = include_bytes!("images/iptc.jpeg");

    // 1. Decode full image
    let mut decoder = JpegDecoder::new(Cursor::new(image));
    let full_pixels = decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    let width = info.width as usize;
    let height = info.height as usize;
    let out_colorspace = decoder.options().jpeg_get_out_colorspace();
    let channels = out_colorspace.num_components();

    // Verify output buffer size calculations and overall dimensions
    assert_eq!(full_pixels.len(), width * height * channels);

    // 2. Define cropping region
    // Choose coordinates that are offset and not aligned to MCU boundaries
    let crop_x = 25;
    let crop_y = 17;
    let crop_w = 80;
    let crop_h = 60;

    // 3. Manually crop the full pixels
    let mut expected_pixels = Vec::with_capacity(crop_w * crop_h * channels);
    for r in 0..crop_h {
        let row_idx = crop_y + r;
        let start = (row_idx * width + crop_x) * channels;
        let end = start + crop_w * channels;
        expected_pixels.extend_from_slice(&full_pixels[start..end]);
    }

    // 4. Decode using the ROI / cropping capability
    let mut crop_decoder = JpegDecoder::new(Cursor::new(image));
    crop_decoder.set_cropping_region(crop_x, crop_y, crop_w, crop_h);
    let crop_pixels = crop_decoder.decode().unwrap();

    // 5. Assert equality
    assert_eq!(crop_pixels.len(), expected_pixels.len(), "Cropped pixel length mismatch");
    assert_eq!(crop_pixels, expected_pixels, "Cropped pixel data mismatch");
}

#[test]
fn test_roi_decoding_progressive() {
    let image: &[u8] = include_bytes!("../../../test-images/jpeg/benchmarks/speed_bench_prog.jpg");

    // 1. Decode full image
    let mut decoder = JpegDecoder::new(Cursor::new(image));
    let full_pixels = decoder.decode().unwrap();
    let info = decoder.info().unwrap();
    let width = info.width as usize;
    let out_colorspace = decoder.options().jpeg_get_out_colorspace();
    let channels = out_colorspace.num_components();

    // 2. Define cropping region
    let crop_x = 100;
    let crop_y = 80;
    let crop_w = 120;
    let crop_h = 90;

    // 3. Manually crop the full pixels
    let mut expected_pixels = Vec::with_capacity(crop_w * crop_h * channels);
    for r in 0..crop_h {
        let row_idx = crop_y + r;
        let start = (row_idx * width + crop_x) * channels;
        let end = start + crop_w * channels;
        expected_pixels.extend_from_slice(&full_pixels[start..end]);
    }

    // 4. Decode using the ROI / cropping capability
    let mut crop_decoder = JpegDecoder::new(Cursor::new(image));
    crop_decoder.set_cropping_region(crop_x, crop_y, crop_w, crop_h);
    let crop_pixels = crop_decoder.decode().unwrap();

    // 5. Assert equality
    assert_eq!(crop_pixels.len(), expected_pixels.len(), "Cropped pixel length mismatch");
    assert_eq!(crop_pixels, expected_pixels, "Cropped pixel data mismatch");
}
