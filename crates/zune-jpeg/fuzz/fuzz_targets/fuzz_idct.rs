#![no_main]
use libfuzzer_sys::fuzz_target;
use zune_jpeg::idct::choose_idct_func;
use zune_jpeg::idct::scalar::idct_int;
use zune_jpeg::zune_core::options::DecoderOptions;

fuzz_target!(|data: [i32; 64]| {
    let mut data = data;

    // keep in some relatively sane range
    // to prevent scalar overflows
    for d in &mut data {
        let bound = 255;
        *d = (*d).min(bound).max(-bound);
    }
    let mut data_vec = data;
    // this is way too big but it shouldn't matter
    // scalar and vector should mutate the minimum needed

    let mut output_scalar = [0i16; 64];
    let mut output_vector = [0i16; 64];

    let dispatched = choose_idct_func(&DecoderOptions::new_fast());
    dispatched(&mut data_vec, &mut output_vector, 8);
    idct_int(&mut data, &mut output_scalar, 8);
    assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
});
