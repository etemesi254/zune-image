use bitflags::bitflags;

use crate::bit_depth::BitDepth;
use crate::colorspace::ColorSpace;

bitflags! {
    /// Encoder options that are flags
    #[derive(Copy,Debug,Clone)]
    struct EncoderFlags:u64{
        /// Whether JPEG images should be encoded as progressive images
        const JPEG_ENCODE_PROGRESSIVE = 0b0000_0000_0000_0000_0000_0000_0000_0001;
        /// Whether JPEG images should use optimized huffman tables
        const JPEG_OPTIMIZED_HUFFMAN  = 0b0000_0000_0000_0000_0000_0000_0000_0010;

    }
}
impl Default for EncoderFlags
{
    fn default() -> Self
    {
        let mut options = EncoderFlags::empty();
        options.set(EncoderFlags::JPEG_ENCODE_PROGRESSIVE, false);
        options.set(EncoderFlags::JPEG_OPTIMIZED_HUFFMAN, false);

        options
    }
}

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
    num_threads: u8,
    effort:      u8,
    flags:       EncoderFlags
}

impl Default for EncoderOptions
{
    fn default() -> Self
    {
        Self {
            width:       0,
            height:      0,
            colorspace:  ColorSpace::RGB,
            quality:     80,
            depth:       BitDepth::Eight,
            num_threads: 4,
            effort:      4,
            flags:       EncoderFlags::default()
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

    /// Get height for which the image will be encoded in
    ///
    /// returns: usize
    ///
    /// # Panics
    /// If height is zero
    pub fn get_height(&self) -> usize
    {
        assert_ne!(self.height, 0);
        self.height
    }
    /// Get the depth for which the image will be encoded in
    pub const fn get_depth(&self) -> BitDepth
    {
        self.depth
    }
    /// Get the quality for which the image will be encoded with
    ///
    ///  # Lossy
    /// - Higher quality means some images take longer to write and
    /// are big but they look good
    ///
    /// - Lower quality means small images and low quality.
    ///
    /// # Lossless
    /// - High quality indicates more time is spent in making the file
    /// smaller
    ///
    /// - Low quality indicates less time is spent in making the file bigger
    pub const fn get_quality(&self) -> u8
    {
        self.quality
    }
    /// Get the colorspace for which the image will be encoded in
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.colorspace
    }
    pub const fn get_effort(&self) -> u8
    {
        self.effort
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

/// JPEG options
impl EncoderOptions
{
    pub const fn jpeg_encode_progressive(&self) -> bool
    {
        self.flags.contains(EncoderFlags::JPEG_ENCODE_PROGRESSIVE)
    }

    pub const fn jpeg_optimized_huffman_tables(&self) -> bool
    {
        self.flags.contains(EncoderFlags::JPEG_OPTIMIZED_HUFFMAN)
    }
}
