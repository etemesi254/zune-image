# Lossless JPEG regression fixtures

The three JPEGs are copied unchanged from Chromium's JPEG Test Suite at
commit `9f0767762862670f6ecb0f057f6a4430af1165e8`:

`third_party/blink/web_tests/images/jpeg-suite/lossless_huffman/`

Chromium's `README.chromium` identifies the original source as
<https://github.com/robert-ancell/jpegsuite> at revision
`8382e7831896cd1adf1cc61a1c9e565c4030aa43`, licensed CC0-1.0.
The accompanying [LICENSE](./LICENSE) is copied from that suite.

The actual SOF3/SOS headers specify 8-bit, 32x32, single-component frames,
component selector 1, sampling 1x1, point transform 0, and no restart interval.
The predictor is 1 in `32x32x8_grayscale.jpg` and
`32x32x8_grayscale_predictor1.jpg`, which are byte-identical; it is 7 in
`32x32x8_grayscale_predictor7.jpg`.

The original reproduction's SHA-256 is:

```text
aec07d4b97e72cea9d4f9d6f438fafe6ee577d90ccf0951eeca00c845d0f9d73
```

`32x32x8_grayscale.gray` contains all 1024 reference samples, without a file
header or row padding. All three fixtures independently decode to those bytes.
Reference SHA-256:

```text
7afa3d4a60293b48a49f0a9e6377f573c401060b7c77363ce952b0c644ed780e
```

## Independent reference

The reference was produced with libjpeg-turbo commit
`afad69dafa6193d838ed075dc34652e646bf745e`, reporting version 3.1.91.
It is a test oracle only, not a Rust dependency or fallback.

```sh
cmake -S /path/to/libjpeg-turbo -B /tmp/lossless-reference \
  -DENABLE_SHARED=OFF -DENABLE_STATIC=ON -DWITH_SIMD=OFF \
  -DWITH_TURBOJPEG=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/lossless-reference --target djpeg-static cjpeg-static -j2
/tmp/lossless-reference/djpeg-static -grayscale \
  -outfile reference.pgm 32x32x8_grayscale.jpg
```

Remove the binary PGM header (`P5`, dimensions `32 32`, maximum `255`) to
obtain the checked-in `.gray` bytes. No zune decoding, screenshot comparison,
rounding tolerance or generated zune expectation was used for this oracle.
The system `djpeg` used during development could not decode SOF3; only the
lossless-capable build above was used as the reference.

Development validation also compared 560 independently encoded/decoded native
cases: all seven predictors, all eight point transforms, with and without
two-row restart intervals, at 1x1, 1x17, 19x1, 17x19 and 32x32 dimensions.
The encoder command was `cjpeg-static -lossless P,Pt [-restart 2] source.pgm`.
Every raw byte matched. Another 34 authored test streams (including 16-bit
Huffman codes, category-16 differences and non-unit frame sampling factors)
were independently decoded by the same native reference.
