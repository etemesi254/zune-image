use crate::bit_depth::BitDepth;
use crate::colorspace::ColorSpace;

/// Options shared by some of the encoders in
/// the `zune-` family of image crates
#[derive(Debug, Copy, Clone)]
pub struct EncoderOptions
{
    width:       usize,
    height:      usize,
    colorspace:  ColorSpace,
    quality:     u8,
    depth:       BitDepth,
    num_threads: u8
}

impl Default for EncoderOptions
{
    fn default() -> Self
    {
        Self {
            width:       0,
            height:      0,
            colorspace:  ColorSpace::RGB,
            quality:     100,
            depth:       BitDepth::Eight,
            num_threads: 4
        }
    }
}

impl EncoderOptions
{
    /// Get the width for which the image will be encoded in
    pub const fn get_width(&self) -> usize
    {
        self.width
    }

    /// Get the height for which the image will be encoded in
    pub const fn get_height(&self) -> usize
    {
        self.height
    }
    /// Get the depth for which the image will be encoded in
    pub const fn get_depth(&self) -> BitDepth
    {
        self.depth
    }
    /// Get the quality for which the image will be encoded with
    pub const fn get_quality(&self) -> u8
    {
        self.quality
    }
    /// Get the colorspace for which the image will be encoded in
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.colorspace
    }

    /// Set width for the image to be encoded
    pub fn set_width(mut self, width: usize) -> Self
    {
        self.width = width;
        self
    }

    /// Set height for the image to be encoded
    pub fn set_height(mut self, height: usize) -> Self
    {
        self.height = height;
        self
    }
    /// Set depth for the image to be encoded
    pub fn set_depth(mut self, depth: BitDepth) -> Self
    {
        self.depth = depth;
        self
    }
    /// Set quality of the image to be encoded
    pub fn set_quality(mut self, quality: u8) -> Self
    {
        self.quality = quality.clamp(0, 100);
        self
    }
    /// Set colorspace for the image to be encoded
    pub fn set_colorspace(mut self, colorspace: ColorSpace) -> Self
    {
        self.colorspace = colorspace;
        self
    }
    /// Set the number of threads allowed for multithreaded encoding
    /// where supported
    ///
    /// Zero means use a single thread
    pub fn set_num_threads(mut self, threads: u8) -> Self
    {
        self.num_threads = threads;

        self
    }

    /// Return number of threads configured for multithreading
    /// where possible
    pub const fn get_num_threads(&self) -> u8
    {
        self.num_threads
    }
}
