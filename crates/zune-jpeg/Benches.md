# Benchmarks of popular jpeg libraries

Here I compare how long it takes popular JPEG decoders to decode the below 7680*4320 image
of (now defunct ?) [Cutefish OS](https://en.cutefishos.com/) default wallpaper.
![img](../../../zune-image/test-images/jpeg/benchmarks/speed_bench.jpg)

## About benchmarks

Benchmarks are weird, especially IO & multi-threaded programs. This library uses both of the above hence performance may
vary.

For best results shut down your machine, go take coffee, think about life and how it came to be and why people should
save the environment.

Then power up your machine, if it's a laptop connect it to a power supply and if there is a setting for performance
mode, tweak it.

Then run.

## Benchmarks vs real world usage

Real world usage may vary.

Notice that I'm using a large image but probably most decoding will be small to medium images.

To make the library thread safe, we do about 1.5-1.7x more allocations than libjpeg-turbo. Although, do note that the
allocations do not occur at ago, we allocate when needed and deallocate when not needed.

Do note if memory bandwidth is a limitation. This is not for you.

## Reproducibility

The benchmarks are carried out on my local machine with an AMD Ryzen 5 4500u

The benchmarks are reproducible.

To reproduce them

1. Clone this repository
2. Install rust(if you don't have it yet)
3. `cd` into the directory.
4. Run `cargo bench`

## Performance features of the three libraries

| feature                      | image-rs/jpeg-decoder | libjpeg-turbo | zune-jpeg |
|------------------------------|-----------------------|---------------|-----------|
| multithreaded                | ✅                     | ❌             | ❌         |
| platform specific intrinsics | ✅                     | ✅             | ✅         |

- Image-rs/jpeg-decoder uses [rayon] under the hood but it's under a feature
  flag.

- libjpeg-turbo uses hand-written asm for platform specific intrinsics, ported to
  the most common architectures out there but falls back to scalar
  code if it can't run in a platform.

# Finally benchmarks

[here]

## Notes

Benchmarks are ran at least once a week to catch regressions early and
are uploaded to Github pages.

Machine specs can be found on the other [landing page]

Benchmarks may not reflect real world usage(threads, other I/O machine bottlenecks)

[landing page]:https://etemesi254.github.io/posts/Zune-Benchmarks/

[here]:https://etemesi254.github.io/assets/criterion/report/index.html

[libjpeg-turbo]:https://github.com/libjpeg-turbo/libjpeg-turbo

[jpeg-decoder]:https://github.com/image-rs/jpeg-decoder

[rayon]:https://github.com/rayon-rs/rayon

## Scoped ARM64 color-conversion measurement (Z3, 2026-11-13)

Native Linux Neoverse-N2, Rust 1.89.0, generic ARM64/musl release, default
features, thin LTO and one codegen unit. Control: refreshed dev
`fa80e24b` plus PRs #525 (`5dbe923c`) and #526 (`b5ad1d9c`).
Invocation counters established NEON block conversion, not AVX2 row dispatch.
Only the reached NEON fixed-point color helper was simplified; IDCT,
upsampling, output bounds and SIMD policy are unchanged.

The primary metric equally weights 96 public RGB/RGBA/BGRA whole-image and
16-row converted-scanline workloads over two real-image scenes, odd 257x193
and 1025x769 sizes, and 4:4:4/4:2:2/4:2:0/4:4:0. Construction, setup,
allocations and destruction are timed. Each of two process sessions used
30 identical-control A/A pairs and 30 counterbalanced A/B pairs, eight
decodes per workload batch, and paired process-level bootstrap intervals.

| Session | Mean elapsed-time reduction | Paired 95% CI | A/A noise margin |
| --- | --- | --- | --- |
| 1 | 6.96% | 6.84% to 7.08% | 0.69% |
| 2 | 7.06% | 6.97% to 7.15% | 0.47% |

All 96 individual autodispatch workloads and the forced-scalar aggregate
passed their separately calibrated protection gates in both sessions.
Forced scalar was about 0.16-0.17% slower, within its protected noise bounds.
This is one-device crate evidence, not a universal ARM64 or browser claim.
Mac M2/M4 and Windows ARM64 remain unmeasured. Skia's separate x86-only
direct-packed-output performance gate is not changed by this kernel.