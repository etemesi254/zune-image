use log::{debug, error, info};
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;
use zune_core::options::DecoderOptions;

use crate::constants::{
    QOI_MASK_2, QOI_OP_DIFF, QOI_OP_INDEX, QOI_OP_LUMA, QOI_OP_RGB, QOI_OP_RGBA, QOI_OP_RUN
};
use crate::errors::QoiErrors;

#[allow(non_camel_case_types)]
enum QoiColorspace
{
    sRGB,
    // SRGB with Linear alpha
    Linear
}

/// A Quite OK Image decoder
///
/// The decoder is initialized by calling `new`
/// and either of [`decode_headers`] to decode headers
/// or [`decode`] to return uncompressed pixels
///
/// Additional methods are provided that give more
/// details of the compressed image like width and height
/// are accessible after decoding headers
///
/// [`decode_headers`]:QoiDecoder::decode_headers
/// [`decode`]:QoiDecoder::decode
pub struct QoiDecoder<'a>
{
    width:             usize,
    height:            usize,
    colorspace:        ColorSpace,
    colorspace_layout: QoiColorspace,
    decoded_headers:   bool,
    stream:            ZByteReader<'a>,
    options:           DecoderOptions
}

impl<'a> QoiDecoder<'a>
{
    /// Create a new QOI format decoder with the default options
    ///
    /// # Arguments
    /// - `data`: The compressed qoi data
    ///
    /// # Returns
    /// - A decoder instance which will on calling `decode` will decode
    /// data
    /// # Example
    ///
    /// ```no_run
    /// let mut decoder = zune_qoi::QoiDecoder::new(&[]);
    /// // additional code
    /// ```
    pub fn new(data: &'a [u8]) -> QoiDecoder<'a>
    {
        QoiDecoder::new_with_options(DecoderOptions::default(), data)
    }
    /// Create a new QOI format decoder that obeys specified restrictions
    ///
    /// E.g can be used to set width and height limits to prevent OOM attacks
    ///
    /// # Arguments
    /// - `data`: The compressed qoi data
    /// - `options`: Decoder options that the decoder should respect
    ///
    /// # Example
    /// ```
    /// use zune_core::options::DecoderOptions;
    /// use zune_qoi::{QoiDecoder};
    /// // only decode images less than 10 in both width and height
    ///
    /// let  options = DecoderOptions::default().set_max_width(10).set_max_height(10);
    ///
    /// let mut decoder=QoiDecoder::new_with_options(options,&[]);
    /// ```
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(options: DecoderOptions, data: &'a [u8]) -> QoiDecoder<'a>
    {
        QoiDecoder {
            width:             0,
            height:            0,
            colorspace:        ColorSpace::RGB,
            colorspace_layout: QoiColorspace::Linear,
            decoded_headers:   false,
            stream:            ZByteReader::new(data),
            options:           options
        }
    }
    /// Decode a QOI header storing needed information into
    /// the decoder instance
    ///
    ///
    /// # Returns
    ///
    /// - On success: Nothing
    /// - On error: The error encountered when decoding headers
    ///     error type will be an instance of [QoiErrors]
    ///
    /// [QoiErrors]:crate::errors::QoiErrors
    pub fn decode_headers(&mut self) -> Result<(), QoiErrors>
    {
        let header_bytes = 4/*magic*/ + 8/*Width+height*/ + 1/*channels*/ + 1 /*colorspace*/;

        if !self.stream.has(header_bytes)
        {
            return Err(QoiErrors::InsufficientData(
                header_bytes,
                self.stream.remaining()
            ));
        }
        // match magic bytes.
        let magic = self.stream.get(4).unwrap();

        if magic != b"qoif"
        {
            return Err(QoiErrors::WrongMagicBytes);
        }

        // these were confirmed to be inbounds by has so use the non failing
        // routines
        let width = self.stream.get_u32_be() as usize;
        let height = self.stream.get_u32_be() as usize;
        let colorspace = self.stream.get_u8();
        let colorspace_layout = self.stream.get_u8();

        if width > self.options.get_max_width()
        {
            let msg = format!(
                "Width {} greater than max configured width {}",
                width,
                self.options.get_max_width()
            );
            return Err(QoiErrors::Generic(msg));
        }

        if height > self.options.get_max_height()
        {
            let msg = format!(
                "Height {} greater than max configured height {}",
                height,
                self.options.get_max_height()
            );
            return Err(QoiErrors::Generic(msg));
        }

        self.colorspace = match colorspace
        {
            3 => ColorSpace::RGB,
            4 => ColorSpace::RGBA,
            _ => return Err(QoiErrors::UnknownChannels(colorspace))
        };
        self.colorspace_layout = match colorspace_layout
        {
            0 => QoiColorspace::sRGB,
            1 => QoiColorspace::Linear,
            _ =>
            {
                if self.options.get_strict_mode()
                {
                    return Err(QoiErrors::UnknownColorspace(colorspace_layout));
                }
                else
                {
                    error!("Unknown/invalid colorspace value {colorspace_layout}, expected 0 or 1");
                    QoiColorspace::sRGB
                }
            }
        };
        self.width = width;
        self.height = height;

        info!("Image width: {:?}", self.width);
        info!("Image height: {:?}", self.height);
        info!("Image colorspace:{:?}", self.colorspace);
        self.decoded_headers = true;

        Ok(())
    }
    /// Decode the bytes of a QOI image data, returning the
    /// uncompressed bytes or  the error encountered during decoding
    ///
    /// Additional details about the encoded image can be found after calling this/[`decode_headers`]
    ///
    /// i.e the width and height. can be accessed by [`get_dimensions`] method.
    ///
    /// # Returns
    /// - On success: The decoded bytes. The length of the bytes will be
    /// - On error: An instance of [QoiErrors] which gives a reason why the image could not
    /// be decoded
    ///
    /// [`decode_headers`]:Self::decode_headers
    /// [`get_dimensions`]:Self::get_dimensions
    /// [QoiErrors]:crate::errors::QoiErrors
    #[allow(clippy::identity_op)]
    pub fn decode(&mut self) -> Result<Vec<u8>, QoiErrors>
    {
        self.decode_headers()?;

        match self.colorspace.num_components()
        {
            3 => self.decode_inner_generic::<3>(),
            4 => self.decode_inner_generic::<4>(),
            _ => unreachable!()
        }
    }
    fn decode_inner_generic<const SIZE: usize>(&mut self) -> Result<Vec<u8>, QoiErrors>
    {
        const LAST_BYTES: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

        let size = self.height * self.width * SIZE;

        let mut pixels = vec![0; size];

        let mut index = [[0_u8; 4]; 64];
        // starting pixel
        let mut px = [0, 0, 0, 255];

        let mut run = 0;

        for pix_chunk in pixels.chunks_exact_mut(SIZE)
        {
            if run > 0
            {
                run -= 1;
                pix_chunk.copy_from_slice(&px[0..SIZE]);
            }
            else if !self.stream.has(5)
            {
                // worst case should be chunk type + RGBA
                // too little bytes
                return Err(QoiErrors::InsufficientData(5, self.stream.remaining()));
            }
            else
            {
                let chunk = self.stream.get_u8();

                if SIZE == 3 && chunk == QOI_OP_RGB
                {
                    let packed_bytes = self.stream.get_fixed_bytes_or_zero::<3>();

                    px[0] = packed_bytes[0];
                    px[1] = packed_bytes[1];
                    px[2] = packed_bytes[2];
                }
                else if SIZE == 4 && chunk == QOI_OP_RGBA
                {
                    let packed_bytes = self.stream.get_fixed_bytes_or_zero::<4>();

                    px.copy_from_slice(&packed_bytes);
                }
                else if (chunk & QOI_MASK_2) == QOI_OP_INDEX
                {
                    px.copy_from_slice(&index[usize::from(chunk) & 63]);
                }
                else if (chunk & QOI_MASK_2) == QOI_OP_DIFF
                {
                    px[0] = px[0].wrapping_add(((chunk >> 4) & 0x03).wrapping_sub(2));
                    px[1] = px[1].wrapping_add(((chunk >> 2) & 0x03).wrapping_sub(2));
                    px[2] = px[2].wrapping_add(((chunk >> 0) & 0x03).wrapping_sub(2));
                }
                else if (chunk & QOI_MASK_2) == QOI_OP_LUMA
                {
                    let b2 = self.stream.get_u8();
                    let vg = (chunk & 0x3f).wrapping_sub(32);

                    px[0] = px[0].wrapping_add(vg.wrapping_sub(8).wrapping_add((b2 >> 4) & 0x0f));
                    px[1] = px[1].wrapping_add(vg);
                    px[2] = px[2].wrapping_add(vg.wrapping_sub(8).wrapping_add((b2 >> 0) & 0x0f));
                }
                else if (chunk & QOI_MASK_2) == QOI_OP_RUN
                {
                    run = usize::from(chunk & 0x3f);
                }

                // copy pixel
                pix_chunk.copy_from_slice(&px[0..SIZE]);

                let color_hash = {
                    // faster hash function
                    // Stolen from https://github.com/zakarumych/rapid-qoi/blob/c5359a53476001d8d170c3733e6ab22e8173f40f/src/lib.rs#L474-L478
                    let v = u64::from(u32::from_ne_bytes(px));
                    let s = ((v << 32) | v) & 0xFF00FF0000FF00FF;

                    (s.wrapping_mul(0x030007000005000Bu64.to_le()).swap_bytes() as u8 & 63) as usize
                };
                index[color_hash] = px;
            }
        }
        let remaining = self.stream.remaining_bytes();

        if remaining != LAST_BYTES
        {
            return Err(QoiErrors::GenericStatic(
                "Last bytes do not match QOI signature"
            ));
        }

        debug!("Finished decoding image");

        Ok(pixels)
    }

    /// Returns QOI colorspace or none if the headers haven't been
    ///
    /// Colorspace returned can either be [RGB] or [RGBA]
    ///
    /// # Returns
    /// - `Some(Colorspace)`: The colorspace present
    /// -  `None` : This indicates the image header wasn't decoded hence
    ///   colorspace is unknown
    ///
    /// [RGB]: zune_core::colorspace::ColorSpace::RGB
    /// [RGBA]: zune_core::colorspace::ColorSpace::RGB
    pub const fn get_colorspace(&self) -> Option<ColorSpace>
    {
        if self.decoded_headers
        {
            Some(self.colorspace)
        }
        else
        {
            None
        }
    }
    /// Return QOI default bit depth
    ///
    /// This is always 8
    ///
    /// # Returns
    /// - [`BitDepth::U8`]
    ///
    /// # Example
    ///
    /// ```
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_qoi::QoiDecoder;
    /// let decoder = QoiDecoder::new(&[]);
    /// assert_eq!(decoder.get_bit_depth(),BitDepth::U8)
    /// ```
    ///
    /// [`BitDepth::U8`]:zune_core::bit_depth::BitDepth::Eight
    pub const fn get_bit_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }

    /// Return the width and height of the image
    ///
    /// Or none if the headers haven't been decoded
    ///
    /// # Returns
    /// - `Some(width,height)` - If headers are decoded, this will return the stored
    /// width and height for that image
    /// - `None`: This indicates the image headers weren't decoded or an error
    /// occurred when decoding headers
    /// # Example
    ///
    /// ```no_run
    /// use zune_qoi::QoiDecoder;
    /// let mut decoder = QoiDecoder::new(&[]);
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
