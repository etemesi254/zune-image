# Authored Huffman recovery control

`early_eoi_444.jpg` is a small, generated 32x24 baseline image with three
1x1-sampled YCbCr components, quantization 8, and a DC/EOB-only scan ending
partway through the second MCU. It contains no third-party image content.

The generator is the `early_eoi` helper in the Huffman recovery integration
tests. The same bytes exercise the unit test's IDCT call counter: raising
only this authored control's declared height must not trigger IDCT work for
missing MCUs. This is not a replacement for any external reproduction.
