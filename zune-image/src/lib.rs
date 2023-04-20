//! A fast and simple image processing library
//!
//! This ties up most of the independent crates in
//!
//!
//!
//! # Things the library is not good at.
//!
//! - Per pixel access methods, while there are methods provided for
//! such things such as [from_fn](crate::image::Image::from_fn) and
//! [modify_pixels_mut](crate::image::Image::modify_pixels_mut) the images
//! are represented as planar hence there will be a lot of cache misses as opposed
//! to others that represents pixels as one single continuous buffer.
//!
//! If you plan on doing multiple per pixel manipulations, [image] crate may be a better
//! fit for your needs
//!
//![image]:https://crates.io/crates/image
#![allow(clippy::redundant_field_names, clippy::uninlined_format_args)]
extern crate core;

pub mod channel;
pub mod codecs;
pub mod deinterleave;
pub mod errors;
pub mod frame;
pub mod image;
pub mod impls;
pub mod metadata;
mod ops;
mod serde;
mod tests;
pub mod traits;
pub mod workflow;
