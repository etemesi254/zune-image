use zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

fn main() {
    // Try with an interleaved JPEG for comparison
    let interleaved_data = include_bytes!("../../../test-images/jpeg/sampling_factors.jpg");
    let mut decoder1 = JpegDecoder::new(ZCursor::new(interleaved_data));
    let pixels1 = decoder1.decode().expect("Failed to decode interleaved");
    let info1 = decoder1.info().expect("Failed to get info");
    println!("Interleaved: {}x{}, first pixel: [{}, {}, {}]",
        info1.width, info1.height, pixels1[0], pixels1[1], pixels1[2]);

    // Non-interleaved 4:4:4 (simpler case - all components same size)
    let test_data = include_bytes!("../../../test-images/jpeg/non_interleaved_444_64x64.jpg");
    let mut decoder = JpegDecoder::new(ZCursor::new(test_data));
    let pixels = decoder.decode().expect("Failed to decode");

    let info = decoder.info().expect("Failed to get info");
    println!("\nNon-interleaved: {}x{}", info.width, info.height);
    println!("Pixels length: {}", pixels.len());

    // Print first 10 pixels
    println!("First 10 pixels:");
    for i in 0..10 {
        let r = pixels[i * 3];
        let g = pixels[i * 3 + 1];
        let b = pixels[i * 3 + 2];
        println!("  [{}, {}, {}]", r, g, b);
    }

    // Print row 32 (middle)
    println!("Middle row (y=32):");
    let row_offset = 32 * 64 * 3;
    for i in 0..5 {
        let r = pixels[row_offset + i * 3];
        let g = pixels[row_offset + i * 3 + 1];
        let b = pixels[row_offset + i * 3 + 2];
        println!("  x={}: [{}, {}, {}]", i, r, g, b);
    }

    // Count non-black pixels
    let non_black = pixels.chunks(3).filter(|c| c[0] > 0 || c[1] > 0 || c[2] > 0).count();
    println!("Non-black pixels: {} / {}", non_black, pixels.len() / 3);

    // Check for any red pixels
    let red_pixels = pixels.chunks(3).filter(|c| c[0] > 10).count();
    let blue_pixels = pixels.chunks(3).filter(|c| c[2] > 10).count();
    println!("Pixels with R>10: {}, B>10: {}", red_pixels, blue_pixels);
}
