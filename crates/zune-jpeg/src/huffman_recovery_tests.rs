use core::sync::atomic::{AtomicUsize, Ordering};

use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::JpegDecoder;

static IDCT_CALLS: AtomicUsize = AtomicUsize::new(0);

fn count_full(block: &mut [i32; 64], output: &mut [i16], stride: usize) {
    IDCT_CALLS.fetch_add(1, Ordering::Relaxed);
    crate::idct::scalar::idct_int(block, output, stride);
}

fn count_sparse(block: &mut [i32; 64], output: &mut [i16], stride: usize) {
    IDCT_CALLS.fetch_add(1, Ordering::Relaxed);
    crate::idct::scalar::idct4x4(block, output, stride);
}

fn count_dc(block: &mut [i32; 64], output: &mut [i16], stride: usize) {
    IDCT_CALLS.fetch_add(1, Ordering::Relaxed);
    crate::idct::scalar::idct_int_1x1(block, output, stride);
}

#[test]
fn missing_mcus_do_not_execute_idct() {
    let mut data = include_bytes!("../../../test-images/jpeg/recovery/early_eoi_444.jpg").to_vec();
    let sof = data
        .windows(2)
        .position(|bytes| bytes == [255, 192])
        .unwrap();
    data[sof + 5..sof + 7].copy_from_slice(&60_000u16.to_be_bytes());
    let options = DecoderOptions::default()
        .set_max_height(65_535)
        .set_strict_mode(false)
        .jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(data), options);
    decoder.decode_headers().unwrap();
    // Only this decoder uses these counters, so other tests cannot affect them.
    IDCT_CALLS.store(0, Ordering::Relaxed);
    decoder.idct_func = count_full;
    decoder.idct_4x4_func = count_sparse;
    decoder.idct_1x1_func = count_dc;
    let output = decoder.decode().unwrap();
    assert_eq!(output.len(), 32 * 60_000 * 4);
    assert_eq!(IDCT_CALLS.load(Ordering::Relaxed), 6);
    assert!(output[32 * 8 * 4..]
        .chunks_exact(4)
        .all(|pixel| pixel == [128, 128, 128, 255]));
}
