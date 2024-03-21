/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

#![no_main]

use libfuzzer_sys::fuzz_target;
fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    use zune_bmp::zune_core::bytestream::ZCursor;
    let data = ZCursor::new(data);
    let mut decoder = zune_bmp::BmpDecoder::new(data);
    let _ = decoder.decode();
});
