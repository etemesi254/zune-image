use log::{debug, info};
use zune_core::bit_depth::BitDepth;
use zune_core::bytestream::ZByteReader;
use zune_core::colorspace::ColorSpace;

use crate::constants::{
    QOI_MASK_2, QOI_OP_DIFF, QOI_OP_INDEX, QOI_OP_LUMA, QOI_OP_RGB, QOI_OP_RGBA, QOI_OP_RUN
};
use crate::errors::QoiErrors;

/// Configuration options for the decoder
#[derive(Copy, Clone, Debug)]
pub struct ZuneQoiOptions
{
    max_width:  usize,
    max_height: usize
}

impl ZuneQoiOptions
{
    pub fn set_max_width(mut self, width: usize) -> Self
    {
        self.max_width = width;
        self
    }
    pub fn set_max_height(mut self, height: usize) -> Self
    {
        self.max_height = height;

        self
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

impl Default for ZuneQoiOptions
{
    fn default() -> Self
    {
        Self {
            max_height: 1 << 17,
            max_width:  1 << 17
        }
    }
}

#[allow(non_camel_case_types)]
enum QoiColorspace
{
    sRGB,
    // SRGB with Linear alpha
    Linear
}

/// A Quite OK Image decoder
pub struct QoiDecoder<'a>
{
    width:             usize,
    height:            usize,
    colorspace:        ColorSpace,
    colorspace_layout: QoiColorspace,
    decoded_headers:   bool,
    stream:            ZByteReader<'a>,
    options:           ZuneQoiOptions
}

impl<'a> QoiDecoder<'a>
{
    /// Create a new QOI format decoder
    ///
    ///
    /// ```no_run
    /// let mut decoder = zune_qoi::QoiDecoder::new(&[]);
    /// // additional code
    /// ```
    pub fn new(data: &'a [u8]) -> QoiDecoder<'a>
    {
        QoiDecoder::new_with_options(ZuneQoiOptions::default(), data)
    }
    /// Create a new QOI format decoder that obeys specified restrictions
    ///
    /// E.g can be used to set width and height limits to prevent OOM attacks
    ///
    /// #Example
    /// ```
    /// use zune_qoi::{QoiDecoder, ZuneQoiOptions};
    /// // only decode images less than 10 in bytes
    ///
    /// let options = ZuneQoiOptions::default().set_max_height(10);
    /// let mut decoder=QoiDecoder::new_with_options(options,&[]);
    /// ```
    #[allow(clippy::redundant_field_names)]
    pub fn new_with_options(options: ZuneQoiOptions, data: &'a [u8]) -> QoiDecoder<'a>
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
    /// the headers
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
        let magic = self.stream.get_as_ref(4).unwrap();

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

        if width > self.options.max_width
        {
            let msg = format!(
                "Width {} greater than max configured width {}",
                width, self.options.max_width
            );
            return Err(QoiErrors::Generic(msg));
        }

        if height > self.options.max_height
        {
            let msg = format!(
                "Height {} greater than max configured height {}",
                height, self.options.max_height
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
            _ => return Err(QoiErrors::UnknownColorspace(colorspace_layout))
        };
        self.width = width;
        self.height = height;

        info!("Image width: {:?}", self.width);
        info!("Image height: {:?}", self.height);
        info!("Image colorspace:{:?}", self.colorspace);
        self.decoded_headers = true;

        Ok(())
    }
    /// Decode the bytes of a QOI image into
    #[allow(clippy::identity_op)]
    pub fn decode(&mut self) -> Result<Vec<u8>, QoiErrors>
    {
        const LAST_BYTES: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

        self.decode_headers()?;
        let size = self.height * self.width * self.colorspace.num_components();

        let mut pixels = vec![0; size];

        let mut index = [[0_u8; 4]; 64];
        // starting pixel
        let mut px = [0, 0, 0, 255];

        let channel_count = self.colorspace.num_components();

        let mut run = 0;

        for pix_chunk in pixels.chunks_exact_mut(self.colorspace.num_components())
        {
            if run > 0
            {
                run -= 1;
            }
            else if !self.stream.has(4)
            {
                // too little bytes
                return Err(QoiErrors::InsufficientData(4, self.stream.remaining()));
            }
            else
            {
                let chunk = self.stream.get_u8();

                if chunk == QOI_OP_RGB
                {
                    let packed_bytes = self.stream.get_fixed_bytes_or_zero::<3>();

                    px[0] = packed_bytes[0];
                    px[1] = packed_bytes[1];
                    px[2] = packed_bytes[2];
                }
                else if chunk == QOI_OP_RGBA
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

                let color_hash = (usize::from(px[0]) * 3
                    + usize::from(px[1]) * 5
                    + usize::from(px[2]) * 7
                    + usize::from(px[3]) * 11)
                    % 64;

                index[color_hash] = px;
            }
            // copy pixel
            pix_chunk.copy_from_slice(&px[0..channel_count]);
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
    ///
    /// Colorspace returned can either be 8 or 16
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
    /// ```
    /// use zune_core::bit_depth::BitDepth;
    /// use zune_qoi::QoiDecoder;
    /// let decoder = QoiDecoder::new(&[]);
    /// assert_eq!(decoder.get_bit_depth(),BitDepth::Eight)
    /// ```
    pub const fn get_bit_depth(&self) -> BitDepth
    {
        BitDepth::Eight
    }

    /// Return the width and height of the image
    ///
    /// Or none if the headers haven't been decoded
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
