use zune_core::colorspace::ColorSpace;

use crate::errors::{ImgEncodeErrors, ImgOperationsErrors};
use crate::image::Image;

/// Encapsulates an image decoder.
///
/// All supported image decoders must implement this class
pub trait DecoderTrait<'a>
{
    /// Decode a buffer already in memory
    ///
    /// The buffer to be decoded is the one passed
    /// to the decoder when initializing the decoder
    ///
    /// # Returns
    /// - OK(Vec<u8>) -> Pixels decoded from the image
    ///
    /// # Errors
    ///  - Any image decoding errors will be propagated to the caller.
    ///
    /// # Example
    /// ```
    /// use zune_image::traits::DecoderTrait;
    /// use zune_jpeg::JpegDecoder;
    /// let mut decoder = JpegDecoder::new(&[0xFF,0xD8]);
    ///
    /// decoder.decode_buffer().unwrap();
    /// ```
    fn decode_buffer(&mut self) -> Result<Vec<u8>, crate::errors::ImgErrors>;

    /// Get width and height of the image
    ///
    /// # Returns
    /// - Some(width,height)
    /// - None -> If image hasn't been decoded and we can't extract
    ///  the width and height.
    fn get_dimensions(&self) -> Option<(usize, usize)>;

    /// Get the colorspace that the decoded pixels
    /// are stored in.
    fn get_out_colorspace(&self) -> ColorSpace;

    /// Get the name of the decoder
    fn get_name(&self) -> &'static str;
}

/// This encapsulates an image operation.
///
/// All operations that can be stored in a workflow
/// need to encapsulate this struct.
pub trait OperationsTrait
{
    /// Get the name of this operation
    fn get_name(&self) -> &'static str;

    /// Execute a simple operation on the image
    /// manipulating the image struct
    ///
    /// # Arguments
    /// - image: A mutable reference to an image which
    /// this operation will manipulate
    ///
    ///
    /// # Errors
    /// Any operations error will be propagated to the caller
    ///
    /// # Example
    /// ```
    /// use zune_image::image::Image;
    /// use zune_image::impls::grayscale::RgbToGrayScale;
    /// use zune_image::traits::OperationsTrait;
    ///
    /// let mut image = Image::new();
    /// // Convert to grayscale
    /// let rgb_to_grayscale = RgbToGrayScale::new();
    /// rgb_to_grayscale.execute_simple(&mut image);
    /// ```
    fn execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>;
}

pub trait EncoderTrait
{
    /// Get the name of the encoder
    fn get_name(&self) -> &'static str;

    /// Set colorspace which the encoder should store the image
    fn set_colorspace(&mut self, colorspace: ColorSpace);

    /// Encode and write to a file
    ///
    /// The file is stored internally by the decoder, e.g
    /// by asking for it during initialization
    ///
    /// # Arguments
    /// - image: An image which we are trying to encode.
    ///
    /// # Example
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::BufWriter;
    /// use zune_image::codecs::ppm::SPPMEncoder;
    /// use zune_image::image::Image;
    /// use zune_image::traits::EncoderTrait;
    ///
    /// let file = BufWriter::new(File::open("").unwrap());
    ///
    /// let mut encoder = SPPMEncoder::new(file);
    ///
    /// let image = Image::new();
    ///
    /// encoder.encode_to_file(&image);
    /// ```
    fn encode_to_file(&mut self, image: &Image) -> Result<(), ImgEncodeErrors>;
}
