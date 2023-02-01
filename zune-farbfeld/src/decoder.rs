use alloc::vec;
use alloc::vec::Vec;

use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

const FARBFELD_COLORSPACE: ColorSpace = ColorSpace::RGBA;
const FARBFELD_BIT_DEPTH: BitDepth = BitDepth::Sixteen;

/// A simple Farbfeld lossless decoder.
///
/// One can modify the decoder accepted dimensions
/// via `DecoderOptions`
pub struct FarbFeldDecoder<'a>
{
    stream:          ZByteReader<'a>,
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    options:         DecoderOptions
}

impl<'a> FarbFeldDecoder<'a>
{
    ///Create a new decoder.
    ///
    /// Data is the raw compressed farbfeld data
    pub fn new(data: &'a [u8]) -> FarbFeldDecoder<'a>
    {
        Self::new_with_options(data, DecoderOptions::default())
    }
    /// Create a new decoder with non default options as opposed to
    /// `new`
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(data: &'a [u8], option: DecoderOptions) -> FarbFeldDecoder<'a>
    {
        FarbFeldDecoder {
            stream:          ZByteReader::new(data),
            height:          0,
            width:           0,
            decoded_headers: false,
            options:         option
        }
    }
    /// Decode a header for this specific image
    pub fn decode_headers(&mut self) -> Result<(), &'static str>
    {
        const HEADER_SIZE: usize = 8/*magic*/ + 4/*width*/ + 4 /*height*/;
        // read magic
        if !self.stream.has(HEADER_SIZE)
        {
            return Err("Not enough bytes for header, need 16");
        }
        let magic_value = self.stream.get_u64_be().to_be_bytes();

        if &magic_value != b"farbfeld"
        {
            return Err("Farbfeld magic bytes not found");
        }
        // 32 bit BE width
        self.width = self.stream.get_u32_be() as usize;
        // 32 BE height
        self.height = self.stream.get_u32_be() as usize;

        info!("Image width: {}", self.width);
        info!("Image height: {}", self.height);

        if self.height > self.options.get_max_height()
        {
            return Err("Image Height is greater than max height. Bump up max_height to support such images");
        }
        if self.width > self.options.get_max_width()
        {
            return Err("Image width is greater than max width. Bump up max_width in options to support such images");
        }

        self.decoded_headers = true;
        Ok(())
    }
    /// Decode a farbfeld data returning raw pixels or an error
    ///
    ///
    /// # Example
    /// ```
    /// use zune_farbfeld::FarbFeldDecoder;
    /// let mut decoder = FarbFeldDecoder::new(b"NOT A VALID FILE");
    ///
    /// assert!(decoder.decode().is_err());
    ///
    /// ```
    pub fn decode(&mut self) -> Result<Vec<u16>, &'static str>
    {
        self.decode_headers()?;

        let size = (FARBFELD_COLORSPACE.num_components()/*RGBA*/)
            .saturating_mul(self.width)
            .saturating_mul(self.height);

        let mut data = vec![0; size];

        if !self.stream.has(size * FARBFELD_BIT_DEPTH.size_of())
        {
            return Err("Incomplete data");
        }

        // 4x16-Bit BE unsigned integers [RGBA] / pixel, row-major
        let remaining_bytes = self.stream.remaining_bytes();

        assert_eq!(remaining_bytes.len(), data.len() * 2);

        for (datum, pix) in data.iter_mut().zip(remaining_bytes.chunks_exact(2))
        {
            *datum = u16::from_be_bytes(pix.try_into().unwrap());
        }
        Ok(data)
    }

    /// Returns farbfeld default image colorspace.
    ///
    /// This is always RGBA
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        FARBFELD_COLORSPACE
    }
    /// Return farbfeld default bit depth
    ///
    /// This is always 16
    pub const fn get_bit_depth(&self) -> BitDepth
    {
        FARBFELD_BIT_DEPTH
    }

    /// Return the width and height of the image
    ///
    /// Or none if the headers haven't been decoded
    ///
    /// ```no_run
    /// use zune_farbfeld::FarbFeldDecoder;
    /// let mut decoder = FarbFeldDecoder::new(&[]);
    ///
    /// decoder.decode_headers().unwrap();
    /// // get dimensions now.
    /// let (w,h)=decoder.get_dimensions().unwrap();
    /// ```
    pub const fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if self.decoded_headers
        {
            return Some((self.width, self.height));
        }
        None
    }
}
