//! Contains image manipulation algorithms
//!
//! This contains structs that implement `OperationsTrait`
//! meaning they can manipulate images
pub mod box_blur;
pub mod brighten;
pub mod colorspace;
pub mod contrast;
pub mod convolve;
pub mod crop;
pub mod depth;
pub mod flip;
pub mod flop;
pub mod gamma;
pub mod gaussian_blur;
pub mod grayscale;
pub mod invert;
pub mod median;
pub mod mirror;
pub mod orientation;
pub mod premul_alpha;
pub mod resize;
pub mod scharr;
pub mod sobel;
pub mod statistics;
pub mod stretch_contrast;
pub mod threshold;
pub mod transpose;
pub mod unsharpen;
