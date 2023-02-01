use bitflags::bitflags;

use crate::colorspace::ColorSpace;

fn decoder_strict_mode() -> DecoderFlags
{
    let mut flags = DecoderFlags::empty();

    flags.set(DecoderFlags::INFLATE_CONFIRM_ADLER, true);
    flags.set(DecoderFlags::PNG_CONFIRM_CRC, true);
    flags.set(DecoderFlags::JPG_ERROR_ON_NON_CONFORMANCE, true);

    flags.set(DecoderFlags::ZUNE_USE_UNSAFE, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE3, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE41, true);

    flags
}

/// Fast decoder options
///
/// Enables all intrinsics + unsafe routines
///
/// Disables png adler and crc checking.
fn fast_options() -> DecoderFlags
{
    let mut flags = DecoderFlags::empty();

    flags.set(DecoderFlags::INFLATE_CONFIRM_ADLER, false);
    flags.set(DecoderFlags::PNG_CONFIRM_CRC, false);
    flags.set(DecoderFlags::JPG_ERROR_ON_NON_CONFORMANCE, false);

    flags.set(DecoderFlags::ZUNE_USE_UNSAFE, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE3, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE41, true);

    flags
}

/// Command line options error resilient and fast
///
/// Features
/// - Ignore CRC and Adler in png
/// - Do not error out on non-conformance in jpg
/// - Use unsafe paths
fn cmd_options() -> DecoderFlags
{
    let mut flags = DecoderFlags::empty();

    flags.set(DecoderFlags::INFLATE_CONFIRM_ADLER, false);
    flags.set(DecoderFlags::PNG_CONFIRM_CRC, false);
    flags.set(DecoderFlags::JPG_ERROR_ON_NON_CONFORMANCE, false);

    flags.set(DecoderFlags::ZUNE_USE_UNSAFE, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX, true);
    flags.set(DecoderFlags::ZUNE_USE_AVX2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE2, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE3, true);
    flags.set(DecoderFlags::ZUNE_USE_SSE41, true);

    flags
}

bitflags! {
    /// Decoder options that are flags
    ///
    /// NOTE: When you extend this, add true or false to
    /// all options above that return a `DecoderFlag`
    pub struct  DecoderFlags:u64{
        /// Whether the decoder should confirm and report adler mismatch
        const INFLATE_CONFIRM_ADLER         = 0b0000_0000_0000_0000_0000_0000_0000_0001;
        /// Whether the PNG decoder should confirm crc
        const PNG_CONFIRM_CRC               = 0b0000_0000_0000_0000_0000_0000_0000_0010;
        /// Whether the png decoder should error out on image non-conformance
        const JPG_ERROR_ON_NON_CONFORMANCE  = 0b0000_0000_0000_0000_0000_0000_0000_0100;
        /// Whether the decoder should use unsafe  platform specific intrinsics
        ///
        /// This will also shut down platform specific intrinsics `(ZUNE_USE_{EXT})` value
        const ZUNE_USE_UNSAFE               = 0b0000_0000_0000_0000_0000_0000_0000_1000;
        /// Whether we should use SSE2.
        ///
        /// This should be enabled for all x64 platforms but can be turned off if
        /// `ZUNE_USE_UNSAFE` is false
        const ZUNE_USE_SSE2                 =  0b0000_0000_0000_0000_0000_0000_0001_0000;
        /// Whether we should use SSE3 instructions where possible.
        const ZUNE_USE_SSE3                 =  0b0000_0000_0000_0000_0000_0000_0010_0000;
        /// Whether we should use sse4.1 instructions where possible.
        const ZUNE_USE_SSE41                =  0b0000_0000_0000_0000_0000_0000_0100_0000;
        /// Whether we should use avx instructions where possible.
        const ZUNE_USE_AVX                  =  0b0000_0000_0000_0000_0000_0000_1000_0000;
        /// Whether we should use avx2 instructions where possible.
        const ZUNE_USE_AVX2                 =  0b0000_0000_0000_0000_0000_0001_0000_0000;
    }
}

/// Decoder options
///
/// Not all options are respected by decoders
/// Each option specifies decoders that respect it
///
/// To remove the annoyance of getters and setters
/// all exposed options are declared public.
#[derive(Debug, Copy, Clone)]
pub struct DecoderOptions
{
    /// Maximum width for which decoders will
    /// not try to decode images larger than
    /// the specified width.
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    max_width:      usize,
    /// Maximum height for which decoders will not
    /// try to decode images larger than the
    /// specified height
    ///
    /// - Default value: 16384
    /// - Respected by: `all decoders`
    max_height:     usize,
    /// Output colorspace
    ///
    /// The jpeg decoder allows conversion to a separate colorspace
    /// than the input.
    ///
    /// I.e you can convert a RGB jpeg image to grayscale without
    /// first decoding it to RGB to get
    ///
    /// - Default value: `ColorSpace::RGB`
    /// - Respected by: `jpeg`
    out_colorspace: ColorSpace,

    /// Maximum number of scans allowed
    /// for progressive jpeg images
    ///
    /// Progressive jpegs have scans
    ///
    /// - Default value:100
    /// - Respected by: `jpeg`
    max_scans: usize,

    flags: DecoderFlags
}

impl DecoderOptions
{
    /// Create the decoder with options  setting most configurable
    /// options to be their safe counterparts
    ///
    /// This is the same as `default` option as default initializes
    /// options to the  safe variant.
    ///
    /// Note, decoders running on this will be slower as it disables
    /// platform specific intrinsics
    pub fn new_safe() -> DecoderOptions
    {
        DecoderOptions::default()
    }

    /// Create the decoder with options setting the configurable options
    /// to the fast  counterparts
    ///
    /// This enables platform specific code paths and disables intrinsics
    pub fn new_fast() -> DecoderOptions
    {
        let flag = fast_options();
        DecoderOptions::default().set_decoder_flags(flag)
    }

    pub fn new_cmd() -> DecoderOptions
    {
        let flag = cmd_options();
        DecoderOptions::default().set_decoder_flags(flag)
    }
}

impl DecoderOptions
{
    /// Get maximum width configured by the decoder
    pub const fn get_max_width(&self) -> usize
    {
        self.max_width
    }

    /// Get maximum width configured by the decoder
    pub const fn get_max_height(&self) -> usize
    {
        self.max_height
    }

    /// Get maximum scans for which the jpeg decoder
    /// should not go above for progressive images
    pub const fn jpeg_get_max_scans(&self) -> usize
    {
        self.max_width
    }
    /// Return true whether the decoder should be in strict mode
    /// And reject most errors
    pub fn get_strict_mode(&self) -> bool
    {
        let flags = DecoderFlags::JPG_ERROR_ON_NON_CONFORMANCE
            | DecoderFlags::PNG_CONFIRM_CRC
            | DecoderFlags::INFLATE_CONFIRM_ADLER;

        self.flags.contains(flags)
    }
    /// Return true if the decoder should use unsafe
    /// routines where possible
    pub const fn get_use_unsafe(&self) -> bool
    {
        self.flags.contains(DecoderFlags::ZUNE_USE_UNSAFE)
    }
    pub const fn jpeg_get_out_colorspace(&self) -> ColorSpace
    {
        self.out_colorspace
    }

    pub fn set_max_width(mut self, width: usize) -> Self
    {
        self.max_width = width;
        self
    }
    /// Set maximum height for which the decoder should not try
    /// decoding images greater than that height
    pub fn set_max_height(mut self, height: usize) -> Self
    {
        self.max_height = height;
        self
    }
    /// Set expected colorspace for which the jpeg output is expected to be in
    pub fn jpeg_set_out_colorspace(mut self, colorspace: ColorSpace) -> Self
    {
        self.out_colorspace = colorspace;
        self
    }

    ///  Whether the routines can use unsafe platform specific
    /// intrinsics when necessary
    ///
    /// Platform intrinsics are implemented for operations which
    /// the compiler can't auto-vectorize, or we can do a marginably
    /// better job at it
    ///
    /// All decoders with unsafe routines respect it.
    ///
    /// Treat this with caution, disabling it will cause slowdowns but
    /// it's provided for mainly for debugging use.
    ///
    /// -Default value : true
    /// - Respected by: `png` and `jpeg`(decoders with unsafe routines)
    pub fn set_use_unsafe(mut self, yes: bool) -> Self
    {
        // first clear the flag
        self.flags.set(DecoderFlags::ZUNE_USE_UNSAFE, yes);
        self
    }
    /// Get maximum scans for which the jpeg decoder should
    /// not exceed when reconstructing images.
    pub fn jpeg_set_max_scans(mut self, max_scans: usize) -> Self
    {
        self.max_scans = max_scans;
        self
    }

    fn set_decoder_flags(mut self, flags: DecoderFlags) -> Self
    {
        self.flags = flags;
        self
    }
    /// Set whether the decoder should be in strict mode
    pub fn set_strict_mode(mut self, yes: bool) -> Self
    {
        let flags = DecoderFlags::JPG_ERROR_ON_NON_CONFORMANCE
            | DecoderFlags::PNG_CONFIRM_CRC
            | DecoderFlags::INFLATE_CONFIRM_ADLER;

        self.flags.set(flags, yes);
        self
    }
    /// Whether the inflate decoder should confirm
    /// adler  checksums
    pub const fn inflate_get_confirm_adler(&self) -> bool
    {
        self.flags.contains(DecoderFlags::INFLATE_CONFIRM_ADLER)
    }
    /// Set whether the inflate decoder should confirm
    /// adler  checksums
    pub fn inflate_set_confirm_adler(mut self, yes: bool) -> Self
    {
        self.flags.set(DecoderFlags::INFLATE_CONFIRM_ADLER, yes);
        self
    }
    /// Whether the inflate decoder should confirm
    /// crc 32 checksums
    pub const fn png_get_confirm_crc(&self) -> bool
    {
        self.flags.contains(DecoderFlags::PNG_CONFIRM_CRC)
    }
    /// Set whether the png decoder should confirm
    /// CRC 32 checksums
    pub fn png_set_confirm_crc(mut self, yes: bool) -> Self
    {
        self.flags.set(DecoderFlags::PNG_CONFIRM_CRC, yes);
        self
    }
}

impl Default for DecoderOptions
{
    fn default() -> Self
    {
        Self {
            out_colorspace: ColorSpace::RGB,
            max_width:      1 << 14,
            max_height:     1 << 14,
            max_scans:      100,
            flags:          decoder_strict_mode()
        }
    }
}
