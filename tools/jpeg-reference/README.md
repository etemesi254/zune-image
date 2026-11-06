# Local JPEG reference checks

This **standalone Cargo workspace** compares complete raw Y/Cb/Cr output with
libjpeg-turbo. It is not a root workspace member, library dependency, or default
test/benchmark target. Only this tool's lockfile contains its reference dependencies.

## Requirements and reference configuration

- Rust 1.89 or newer, a C compiler, CMake, and NASM on x86.
- Use the committed tool lockfile with `--locked`.
- `turbojpeg` 1.5.1 and the locked `turbojpeg-sys` 1.2.0 use vendored
  libjpeg-turbo 3.1.0. The comparison uses its full-size, accurate-DCT YUV defaults.
- Set `TURBOJPEG_SOURCE=vendor` and `TURBOJPEG_STATIC=1` when building, as in the
  manual workflow. Do not substitute a system library when reporting results.

The initial scope is complete, lossy 8-bit Huffman YCbCr JPEGs ending in EOI,
with component selectors 1/2/3 and 4:4:4, 4:2:2, or 4:2:0 subsampling.
Inputs requiring more than 256 MiB of YUV output are rejected.
RGB/CMYK, arithmetic, lossless, scaled output, and truncated-input recovery
comparisons are intentionally outside this small tool's scope.

## Run

From this directory:

```sh
export TURBOJPEG_SOURCE=vendor TURBOJPEG_STATIC=1
cargo test --locked
cargo run --release --locked -- report
cargo run --release --locked -- check
```

Both commands print per-plane differing-sample counts, maximum absolute delta,
and the first differing coordinate and values. Padding is excluded.
`report` is explicitly diagnostic: reported differences do not change its exit
status. `check` exits nonzero if **any** logical sample differs; there is no
blanket tolerance of two.

Current `dev` can report IDCT rounding differences on the textured fixtures.
That is a finding, not a tool-test failure. The explicit `check` command exposes
these differences; it must not be a default merge gate until the corresponding
decoder corrections land.

To inspect other supported complete inputs, append their paths:

```sh
cargo run --release --locked -- report /path/to/image.jpg
```

## Optional timing

```sh
cargo run --release --locked -- bench 1000 fixtures/flat_420.jpg
```

The command first requires exact output agreement. It then times fresh decoder
creation, decoding, and output allocation for each implementation. Both use one
contiguous output allocation with matching four-byte row strides and subsampling
padding. Logical output dimensions are checked before comparison.

These are end-to-end diagnostic means, not pure entropy/IDCT timings or statistical
regression gates. Record the commit, compiler, CPU, iteration count, and fixture;
use an otherwise idle host and repeat runs before making performance claims.
There is no reused-buffer mode or cross-platform speed threshold in this first tool.

## Fixtures and provenance

The four 33x35 fixtures are authored synthetic inputs, not downloaded photographs.
The three textured progressive cases exercise different subsampling and partial
MCU edges. The constant-gray baseline case is an exact-match control.

Their RGB formulas and encoding settings (quality 90, progressive for texture)
are in [the generator](examples/generate-fixtures.rs). Regenerate intentionally,
not as part of normal CI:

```sh
cargo run --release --locked --example generate-fixtures
```

## CI

The separate `JPEG reference` workflow is **manual-only**. Its default `report`
mode records findings without asserting parity. Selecting `check` explicitly
enables exact failure behavior. Normal workspace tests and benchmark workflows
do not build this tool.
