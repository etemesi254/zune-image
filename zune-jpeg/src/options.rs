use std::num::NonZeroU32;

use zune_core::colorspace::ColorSpace;

/// Options available that influence decoding.
#[derive(Copy, Clone)]
pub struct ZuneJpegOptions
{
    /// Whether or not we wre allowed
    /// to use unsafe code
    use_unsafe:     bool,
    /// The output colorspace
    /// expected from a decode procedure.
    out_colorspace: ColorSpace,
    /// Limits for the decoder
    /// These prevent OOM exhaustion
    max_width:      u16,
    max_height:     u16,
    /// Maximum number of scans to allow in the image
    max_scans:      usize,
    /// Treat warnings as errors.
    strict_mode:    bool
}
impl Default for ZuneJpegOptions
{
    fn default() -> Self
    {
        Self {
            use_unsafe:     true,
            out_colorspace: ColorSpace::RGB,
            max_width:      1 << 15,
            max_height:     1 << 15,
            max_scans:      64,
            strict_mode:    false
        }
    }
}
impl ZuneJpegOptions
{
    /// Create a new option
    #[must_use]
    pub fn new() -> ZuneJpegOptions
    {
        Self::default()
    }
    /// Get the default output colorspace
    ///
    /// This is the colorspace the image will be in case decoding happens successfully
    #[must_use]
    pub const fn get_out_colorspace(&self) -> ColorSpace
    {
        self.out_colorspace
    }
    #[must_use]
    pub fn set_out_colorspace(mut self, colorspace: ColorSpace) -> ZuneJpegOptions
    {
        self.out_colorspace = colorspace;
        self
    }
    /// Check if we can use platform specific
    /// unsafe procedures for decoding.
    #[must_use]
    pub const fn get_use_unsafe(&self) -> bool
    {
        self.use_unsafe
    }
    /// Set whether we can use platform specific
    /// unsafe procedures for decoding.
    #[must_use]
    pub const fn set_use_unsafe(mut self, choice: bool) -> ZuneJpegOptions
    {
        self.use_unsafe = choice;
        self
    }
    /// Get the maximum width allowed for images
    ///
    /// Default is 16,384
    #[must_use]
    pub const fn get_max_width(&self) -> u16
    {
        self.max_width
    }
    /// Set maximum width allowed for images
    ///
    /// Can be used to prevent OOM scenarios where the library over-allocates
    /// too much memory for corrupt images
    #[must_use]
    pub fn set_max_width(mut self, max_width: u16) -> ZuneJpegOptions
    {
        self.max_width = max_width;
        self
    }
    /// Get maximum height allowed for images
    ///
    /// Default is 16,384
    #[must_use]
    pub const fn get_max_height(&self) -> u16
    {
        self.max_height
    }
    /// Set maximum height allowed for images
    ///
    /// Can be used to prevent OOM scenarios where the library over-allocates
    /// too much memory for corrupt images
    #[must_use]
    pub fn set_max_height(mut self, max_height: u16) -> ZuneJpegOptions
    {
        self.max_height = max_height;
        self
    }
    /// Get number of progressive scans allowed for decoding progressive images
    #[must_use]
    pub const fn get_max_scans(&self) -> usize
    {
        self.max_scans
    }
    /// Set number of maximum scans allowed for decoding progressive images
    ///
    /// Can be used to protect DOS hangs from corrupt images.
    /// Default is 64.
    #[must_use]
    pub fn set_max_scans(mut self, scans: usize) -> ZuneJpegOptions
    {
        self.max_scans = scans;
        self
    }
    /// Get if the library will treat warnings as errors.
    #[must_use]
    pub const fn get_strict_mode(&self) -> bool
    {
        self.strict_mode
    }
    /// Set whether to treat warnings as errors
    #[must_use]
    pub fn set_strict_mode(mut self, choice: bool) -> ZuneJpegOptions
    {
        self.strict_mode = choice;
        self
    }
}
