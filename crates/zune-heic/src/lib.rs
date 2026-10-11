//! A fast, safe HEIF/HEIC image decoder, with its own HEVC decoder.
//!
//! This crate decodes HEIF (ISO/IEC 23008-12) still images whose pictures are
//! coded with HEVC (ITU-T H.265), i.e. `.heic`/`.heif` files such as the
//! photos taken by iPhones. It contains its own HEVC intra decoder, so it has
//! no C dependencies.
//!
//! # Features
//! - **Bit-identical to libheif.** The decoded RGB output matches libheif
//!   (1.17) byte for byte, and the decoded YCbCr of every coded picture
//!   matches libde265. Both are checked in CI on every change.
//! - **Fast.** 1.3–2.7× faster than libheif, faster than the pure Rust `heic`
//!   crate (see [Performance](#performance)):
//!   - SIMD inverse transforms (IDCT/IDST): SSE4.1 on x86_64 and NEON on
//!     aarch64, selected at runtime, with a portable fallback.
//!   - Multi-threaded decoding: grid images (e.g. iPhone photos, made of
//!     512×512 tiles) decode their tiles in parallel; single images decode
//!     CTU rows in parallel when the stream allows it (wavefront parallel
//!     processing), and deblocking and SAO are split over the CPUs as well.
//!   - Tiles are converted straight into the output image, without
//!     intermediate full-image copies.
//! - **Safe.** The decoder is safe Rust (`#![deny(unsafe_code)]`); `unsafe`
//!   is limited to the SIMD transform kernels and the Apple VideoToolbox
//!   bindings.
//! - **Hardware decoding on Apple platforms** through VideoToolbox, on by
//!   default on macOS (see [`DecoderOptions::hvec_set_use_videotoolbox`]).
//! - **Image properties handled:** grids, clean aperture cropping (`clap`),
//!   rotation (`irot`) and mirroring (`imir`) in the order the file lists
//!   them, colour description (`colr` nclx, or the HEVC VUI) for the
//!   YCbCr → RGB conversion, ICC profiles and Exif metadata.
//! - **Alpha.** Alpha planes (auxiliary alpha images, single or grid) are
//!   decoded into RGBA / LumaA output; premultiplied alpha (`prem`) is
//!   reported by [`HeifDecoder::is_alpha_premultiplied`].
//! - **Greyscale** (monochrome HEVC) images decode to [`Luma`](zune_core::colorspace::ColorSpace::Luma).
//! - **`no_std`** (with `alloc`) when the `std` feature is disabled.
//!
//! # Usage
//! Add the crate to the dependencies in `Cargo.toml`
//!
//! ```toml
//! [dependencies]
//! zune-heic = "0.5"
//! ```
//!
//! # Examples
//!
//! ## Decode a HEIC file
//!
//! [`HeifDecoder::decode`] returns interleaved 8-bit pixels in the order of
//! [`HeifDecoder::colorspace`] (usually RGB), with the cropping, rotation and
//! mirroring stored in the file already applied.
//!
//! ```no_run
//! use zune_heic::HeifDecoder;
//! use zune_heic::zune_core::bytestream::ZCursor;
//!
//! let data = std::fs::read("image.heic").unwrap();
//! let mut decoder = HeifDecoder::new(ZCursor::new(&data));
//! let pixels = decoder.decode().unwrap();
//!
//! let width = decoder.width().unwrap();
//! let height = decoder.height().unwrap();
//! let colorspace = decoder.colorspace().unwrap();
//! assert_eq!(pixels.len(), width * height * colorspace.num_components());
//! ```
//!
//! ## Read the dimensions without decoding
//!
//! [`HeifDecoder::decode_headers`] only parses the file structure, which is
//! cheap: the image data is not decoded.
//!
//! ```no_run
//! use zune_heic::HeifDecoder;
//! use zune_heic::zune_core::bytestream::ZCursor;
//!
//! let data = std::fs::read("image.heic").unwrap();
//! let mut decoder = HeifDecoder::new(ZCursor::new(&data));
//! decoder.decode_headers().unwrap();
//! println!(
//!     "{}x{} {:?}",
//!     decoder.width().unwrap(),
//!     decoder.height().unwrap(),
//!     decoder.colorspace().unwrap()
//! );
//! ```
//!
//! ## Decode into an existing buffer
//!
//! ```no_run
//! use zune_heic::HeifDecoder;
//! use zune_heic::zune_core::bytestream::ZCursor;
//!
//! let data = std::fs::read("image.heic").unwrap();
//! let mut decoder = HeifDecoder::new(ZCursor::new(&data));
//! decoder.decode_headers().unwrap();
//!
//! let mut pixels = vec![0_u8; decoder.output_buffer_size().unwrap()];
//! decoder.decode_into(&mut pixels).unwrap();
//! ```
//!
//! ## Read from a file without loading it first
//!
//! The decoder reads from anything implementing
//! [`ZByteReaderTrait`](zune_core::bytestream::ZByteReaderTrait), e.g. a
//! buffered file with the `std` feature.
//!
//! ```no_run
//! use std::fs::File;
//! use std::io::BufReader;
//! use zune_heic::HeifDecoder;
//!
//! let file = BufReader::new(File::open("image.heic").unwrap());
//! let mut decoder = HeifDecoder::new(file);
//! let pixels = decoder.decode().unwrap();
//! ```
//!
//! ## Metadata
//!
//! ```no_run
//! use zune_heic::HeifDecoder;
//! use zune_heic::zune_core::bytestream::ZCursor;
//!
//! let data = std::fs::read("image.heic").unwrap();
//! let mut decoder = HeifDecoder::new(ZCursor::new(&data));
//! decoder.decode_headers().unwrap();
//!
//! if let Some(exif) = decoder.exif_data() {
//!     // raw TIFF-structured Exif, starting with `II` or `MM`
//!     println!("{} bytes of Exif", exif.len());
//! }
//! if let Some(icc) = decoder.icc_data() {
//!     println!("{} byte ICC profile", icc.len());
//! }
//! ```
//!
//! ## Options
//!
//! Decoding is configured with [`DecoderOptions`]:
//!
//! ```no_run
//! use zune_heic::HeifDecoder;
//! use zune_heic::zune_core::bytestream::ZCursor;
//! use zune_heic::zune_core::options::DecoderOptions;
//!
//! let options = DecoderOptions::default()
//!     // use the software decoder on macOS too
//!     .hvec_set_use_videotoolbox(false)
//!     // the image data (`mdat`) is read into memory; allow up to 64 MiB
//!     .set_hevc_max_mdat_size(64 << 20)
//!     // threads used to decode grid tiles in parallel
//!     .set_num_threads(8)
//!     // refuse images larger than this
//!     .set_max_width(16384)
//!     .set_max_height(16384);
//!
//! let data = std::fs::read("image.heic").unwrap();
//! let mut decoder = HeifDecoder::new_with_options(ZCursor::new(&data), options);
//! let pixels = decoder.decode().unwrap();
//! ```
//!
//! # Performance
//!
//! Decoding an iPhone photo (`fuzz-samples/IMG_4862.HEIC`, a 4284×5712 grid
//! of 49 tiles) to RGB on an Apple M3 Pro (18 GB RAM), measured with
//! `cargo bench -p zune-benches --bench decode_heic --features libheif`
//! (`benchmarks/benches/decode_heic.rs` in the repository; the `libheif`
//! feature needs the system libheif):
//!
//! | Decoder | Time |
//! |---|---|
//! | zune-heic, VideoToolbox (hardware) | 43.5 ms |
//! | **zune-heic, software** | **84.2 ms** |
//! | [`heic`](https://crates.io/crates/heic) crate (pure Rust) | 114.7 ms |
//! | libheif (through `libheif-rs`) | 202.7 ms |
//!
//! The software decoder is about 2.4× faster than libheif and 1.4× faster than
//! the `heic` crate there; only the hardware decoder is faster.
//!
//! On a 2-core x86_64 machine (Xeon @ 2.8 GHz, Linux), against libheif 1.17.6
//! with libde265 1.0.15 (`heif_decode_image` with its default settings), best
//! of 7 runs, on the samples in `fuzz-samples/`:
//!
//! | Image | Size | libheif | zune-heic |
//! |---|---|---|---|
//! | iPhone photo, 49-tile grid | 4284×5712 | 569 ms | 384 ms |
//! | grid, 48 tiles | 4000×3000 | 368 ms | 284 ms |
//! | single image | 2880×1800 | 243 ms | 119 ms |
//! | single image | 1920×1920 | 115 ms | 42 ms |
//! | single image | 800×800 | 86 ms | 61 ms |
//!
//! # Crate features
//!
//! - `std` (default): multi-threaded decoding, runtime CPU feature detection
//!   for the SIMD transforms, reading from `std::io` types and the Apple
//!   VideoToolbox decoder. Without it the crate is `no_std` + `alloc`; SIMD
//!   kernels are then chosen at compile time (`-C target-feature`).
//! - `simd`: use `core::simd` (portable SIMD) transforms; requires a nightly
//!   compiler.
//! - `dump-tiles`: `HeifDecoder::decode_tiles_yuv`, which returns every coded
//!   picture's HEVC bitstream and its decoded YCbCr planes, for testing the
//!   decoder against a reference decoder.
//!
//! # Limitations
//!
//! - Only the primary image is decoded (no thumbnails, depth maps or image
//!   sequences).
//! - 8-bit, intra-coded HEVC only (what HEIF still images use), in 4:2:0 or
//!   monochrome; 10-bit, 4:2:2 and 4:4:4 images are rejected.
//! - Premultiplied alpha is returned as stored (like libheif); see
//!   [`HeifDecoder::is_alpha_premultiplied`].
//! - With VideoToolbox, the YCbCr → RGB conversion is not the libheif one, so
//!   the output is not bit-identical to libheif.
//!
//! # Patents
//!
//! HEIC images are compressed with HEVC (H.265), and HEVC is covered by
//! patents in many countries. Licenses for these patents are offered by
//! patent pools (such as Access Advance and Via LA) and by individual patent
//! holders.
//!
//! This crate's license (MIT, Apache-2.0 or Zlib, at your option) covers the
//! source code only; it does not grant any patent rights. Whether you need a
//! patent license to use, ship or sell software that includes this decoder
//! depends on your product, your use of it and the countries involved.
//! Obtaining any such license is your responsibility. If in doubt, consult a
//! lawyer; this notice is not legal advice.
//!
//! Decoding with Apple VideoToolbox (see [Features](#features)) uses the HEVC
//! decoder that ships with macOS instead of the one in this crate.
//!
//! [`DecoderOptions`]: zune_core::options::DecoderOptions
//! [`DecoderOptions::hvec_set_use_videotoolbox`]: zune_core::options::DecoderOptions::hvec_set_use_videotoolbox
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unexpected_cfgs)]
// The decoder itself is safe Rust; `unsafe` is only allowed in the opt-in
// platform modules below (SIMD transforms, Apple VideoToolbox FFI).
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(feature = "simd", feature(portable_simd))]
#![warn(
    clippy::correctness,
    clippy::perf,
    clippy::pedantic,
    clippy::inline_always,
    clippy::missing_errors_doc,
    clippy::panic
)]
#![allow(
    clippy::needless_return,
    clippy::similar_names,
    clippy::inline_always,
    clippy::needless_range_loop,
    clippy::doc_markdown,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::module_name_repetitions,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    clippy::struct_excessive_bools
)]
#[macro_use]
extern crate alloc;
extern crate core;
#[cfg(feature = "std")]
#[allow(unsafe_code)] // FFI to Apple VideoToolbox
mod apple_videotoolbox;
mod colour;
mod decoder;
mod errors;
mod header_structs;
mod headers;
mod hevc_decoder;
// debug tracing macros, private to the crate
pub(crate) use hevc_decoder::macros::{dbg_print, dbg_println, debug_more};
mod processor;
mod transform;
mod utils;

pub extern crate zune_core;

pub use decoder::HeifDecoder;
#[cfg(feature = "dump-tiles")]
pub use decoder::DecodedTile;
pub use errors::HeicErrors;
pub use hevc_decoder::nal_parser::NalError;
