use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;

const FARBFELD_COLORSPACE: ColorSpace = ColorSpace::RGBA;
const FARBFELD_BIT_DEPTH: BitDepth = BitDepth::Sixteen;

#[derive(Copy, Clone, Debug)]
pub struct ZuneFarbFeldOptions
{
    max_width:  usize,
    max_height: usize
}

impl ZuneFarbFeldOptions
{
    pub fn set_max_width(&mut self, width: usize)
    {
        self.max_width = width;
    }
    pub fn set_max_height(&mut self, height: usize)
    {
        self.max_height = height;
    }
    pub const fn get_max_width(&self) -> usize
    {
        self.max_width
    }
    pub const fn get_max_height(&self) -> usize
    {
        self.max_height
    }
}

impl Default for ZuneFarbFeldOptions
{
    fn default() -> Self
    {
        Self {
            max_height: 1 << 17,
            max_width:  1 << 17
        }
    }
}

/// A simple Farbfeld lossless decoder
pub struct FarbFeldDecoder<'a>
{
    stream:          ZByteReader<'a>,
    width:           usize,
    height:          usize,
    decoded_headers: bool,
    options:         ZuneFarbFeldOptions
}

impl<'a> FarbFeldDecoder<'a>
{
    ///Create a new decoder.
    ///
    /// Data is the raw compressed farbfeld data
    pub fn new(data: &'a [u8]) -> FarbFeldDecoder<'a>
    {
        Self::new_with_options(data, ZuneFarbFeldOptions::default())
    }
    /// Create a new decoder with non default options as opposed to
    /// `new`
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(data: &'a [u8], option: ZuneFarbFeldOptions) -> FarbFeldDecoder<'a>
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
        // read magic
        if !self.stream.has(8/*magic*/ + 4/*width*/ + 4 /*height*/)
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

        if self.height > self.options.max_height
        {
            return Err("Image Height is greater than max height. Bump up max_height to support such images");
        }
        if self.width > self.options.max_width
        {
            return Err("Image width is greater than max width. Bump up max_width in options to support such images");
        }

        self.decoded_headers = true;
        Ok(())
    }
    /// Decode a farbfeld data returning raw pixels or an error
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

        for datum in data.iter_mut()
        {
            *datum = self.stream.get_u16_be();
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
    pub const fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        if self.decoded_headers
        {
            return Some((self.width, self.height));
        }
        None
    }
}
