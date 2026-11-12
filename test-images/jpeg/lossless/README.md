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

## Additional public lossless modes

The additional fixtures are copied from Chromium commit
`5d20b94f5b20c0e3afaa5d7a3e5c913b7c46e37a`, under the same CC0 suite license:

| Suite / file | SHA-256 |
|---|---|
| lossless_huffman/32x32x8_ycbcr_interleaved.jpg | d836df7599e7281b7770a01ff7ddbae7443dfd2f1d393da8fd9939e2225904d0 |
| lossless_arithmetic/32x32x8_grayscale.jpg | 2bd479c5a33e144cb5b8d6cb47a20feed16710c51c71c1b00cbf12a5ae94e914 |
| lossless_arithmetic/32x32x8_ycbcr_interleaved.jpg | acc07f44710ea13517d34e1b61f56aabc68703de86e9ae1a96e82e27c145e08f |
| lossless_huffman/32x32x8_ycbcr.jpg (unsupported split-scan control) | 5b1d893d2b3211d161138bd9b4626573efffa141abd7f777c2b57c0924595f5e |
| lossless_arithmetic/32x32x8_ycbcr.jpg (unsupported split-scan control) | bef1ab671a26a16e1d47dfabb0cf7aed43d605fe6b6cef657dacf2331bdbaf1c |

The three requested files have 32x32, 8-bit frames; predictor 1; Se=Ah=Pt=0;
1x1 sampling; zero quantization/AC selectors; no DRI; and no DAC.
Grayscale uses component ID 1 and DC selector 0. Interleaved color uses SOF/SOS
IDs 1/2/3 in that order. Huffman selects DC tables 0/1/2, respectively.
Arithmetic selects table 0 for every component and therefore shares its
adaptation contexts; default conditioning is L=0, U=1.

### Independent component oracle and RGB contract

The full-JPEG reference is `thorfdbg/libjpeg` commit
`702114c8130ae818ccd2354cc03e879dac69e579`, built from its distributed configure
files using `./configure` and `make -o configure -o autoconfig.h.in -j2`.
All three modes were actually decoded using `jpeg -c input.jpg output.pnm`.
Neither stock libjpeg-turbo arithmetic support nor a blank native PDF image is
assumed to be an oracle.

For exact samples, diagnostic logging immediately after `DecodeSample` records
the independent decoder's reconstructed component values before presentation.
It does not change entropy decoding or prediction. Its four fractional storage
bits are removed, then samples are reduced to the declared 8-bit output.
The captured values also match the suite's known source planes, derived from
`generate/32x32x16_rgb.ppm` and its published generation formula.

This distinction matters: the suite's formula produces 48 chroma values of
256. The reference's ordinary PNM export clamps those to 255; that export is
not the component oracle. The checked-in `.yuv` retains the established
modulo-precision output behavior, independently confirmed with libjpeg-turbo's
API configured with `out_color_space = jpeg_color_space`.

The two color encodings share `32x32x8_ycbcr_interleaved.yuv` (3072 packed
samples), SHA-256
`ff4e763443ca446d11fbe8c7f7f186a320e11ace0fa89ae9da9113e5cd1f0921`.
Arithmetic grayscale matches the existing 1024-byte grayscale oracle.

`32x32x8_ycbcr_interleaved.rgb` is a separate, explicit fixed-point conversion
contract, not a claim of reversible RGB or parity with every native converter.
For Cb/Cr centered at 128 and `base = Y * 16384 + 8191`, compute:

```text
R = clamp((base + 22970 * Cr) >> 14)
G = clamp((base - 11700 * Cr - 5638 * Cb) >> 14)
B = clamp((base + 29032 * Cb) >> 14)
```

Clamp each result to 0..255. Its SHA-256 is
`ac9848d008072491d3dc14a054d967d19e1d68ff563191d7db5c00313e5c7e4a`.
No ordinary-JPEG conversion constants or logical-edge behavior are changed.

### Synthetic controls

The `generated` directory retains known source planes (`source-*.raw`) and
32 small, odd, narrow and restarted streams, plus a deliberately invalid
arithmetic residual-overflow stream. Source samples are authored as
`(x*73 + y*151 + component*97 + x*y*17) & 255`; expected point-transformed
samples are `(source >> Pt) << Pt`, not zune-generated expectations.
These authored synthetic data are also dedicated under the enclosing CC0
license. `generated/SHA256SUMS` records every synthetic stream and source plane.

The seven predictors use Pt=0..6. Narrow 1x9 cases use Pt=7; 1x1 cases exercise
initialization; 33x20 cases use two-row restart intervals and wrap RST numbers.
Color SOF IDs are 7/19/3, SOS order is 3/7/19, and distinct table assignments
exercise mapping independently of component position. Arithmetic controls
include shared/default conditioning, distinct non-default DAC entries, and
L=U=0 or 15.

Huffman fixtures use independent fixed-length canonical tables. Arithmetic
fixtures use the CC0 suite's QM encoder. Every encoded residual is independently
verified with the full-JPEG reference above, and Pt=0 component reconstruction
matches exactly. Its nonzero-Pt presentation is not used as an oracle.
All Huffman point transforms are additionally checked with libjpeg-turbo;
that secondary check canonicalizes SOF declaration order to accommodate its
SOS-order restriction without changing entropy data. Arithmetic nonzero-Pt
output is checked against retained source planes and independent residuals,
using the same prediction/reconstruction implementation verified by Huffman.

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
