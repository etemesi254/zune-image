#![no_main]
use libfuzzer_sys::fuzz_target;
use zune_jpeg::idct::scalar::idct_int;

fuzz_target!(|data: [i32; 64]| {
    let mut data = data;

    // keep in some relatively sane range
    // to prevent scalar overflows
    for d in &mut data
    {
        let bound = 255;
        *d = (*d).min(bound).max(-bound);
    }
    let mut data_vec = data;
    // this is way too big but it shouldn't matter
    // scalar and vector should mutate the minimum needed

    let mut output_scalar = [0i16; 64];
    let mut output_vector = [0i16; 64];

    let _must_use_supported_vector_arch;
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[cfg(target_feature = "avx2")]
    {
        use zune_jpeg::idct::avx2::idct_avx2;
        idct_avx2(&mut data_vec, &mut output_vector, 8);
        _must_use_supported_vector_arch = true;
    }

    #[cfg(target_arch = "aarch64")]
    {
        use zune_jpeg::idct::neon::idct_neon;
        idct_neon(&mut data_vec, &mut output_vector, 8);
        _must_use_supported_vector_arch = true;
    }

    if _must_use_supported_vector_arch
    {
        idct_int(&mut data, &mut output_scalar, 8);
        assert_eq!(output_scalar, output_vector, "IDCT and scalar do not match");
    }
    else
    {
        panic!("No vector IDCT ran!")
    }
});
