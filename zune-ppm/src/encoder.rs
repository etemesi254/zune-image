use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::io::Error;

use zune_core::bit_depth::BitType;
use zune_core::bytestream::ZByteWriter;
use zune_core::colorspace::ColorSpace;
use zune_core::options::EncoderOptions;

/// Errors occurring during encoding
pub enum PPMEncodeErrors
{
    Static(&'static str),
    TooShortInput(usize, usize),
    UnsupportedColorspace(ColorSpace),
    IOErrors(io::Error)
}

impl From<io::Error> for PPMEncodeErrors
{
    fn from(err: Error) -> Self
    {
        PPMEncodeErrors::IOErrors(err)
    }
}

impl Debug for PPMEncodeErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            PPMEncodeErrors::Static(ref errors) =>
            {
                writeln!(f, "{errors}")
            }
            PPMEncodeErrors::IOErrors(ref err) =>
            {
                writeln!(f, "{err}")
            }
            PPMEncodeErrors::TooShortInput(expected, found) =>
            {
                writeln!(f, "Expected input of length {expected} but found {found}")
            }
            PPMEncodeErrors::UnsupportedColorspace(colorspace) =>
            {
                writeln!(f, "Unsupported colorspace {colorspace:?} for ppm")
            }
        }
    }
}

pub enum PPMVersions
{
    P5,
    P6,
    P7
}

impl Display for PPMVersions
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::P6 => write!(f, "P6"),
            Self::P5 => write!(f, "P5"),
            Self::P7 => write!(f, "P7")
        }
    }
}

/// A PPM encoder
pub struct PPMEncoder<'a>
{
    data:    &'a [u8],
    options: EncoderOptions
}

impl<'a> PPMEncoder<'a>
{
    /// Create a new encoder which will encode the specified
    /// data whose format is contained in the options.
    ///
    /// # Note
    /// To encode 16 bit data,it still must be provided as u8 bytes
    /// in native endian.
    ///
    /// One can use [`u16::to_ne_bytes`] for this if data is in a u16 slice
    ///
    /// [`u16::to_ne_bytes`]:u16::to_ne_bytes
    pub fn new(data: &'a [u8], options: EncoderOptions) -> PPMEncoder<'a>
    {
        PPMEncoder { data, options }
    }

    fn encode_headers(&self, stream: &mut ZByteWriter) -> Result<(), PPMEncodeErrors>
    {
        let version = version_for_colorspace(self.options.colorspace).ok_or(
            PPMEncodeErrors::UnsupportedColorspace(self.options.colorspace)
        )?;

        let width = self.options.width;
        let height = self.options.height;
        let components = self.options.colorspace.num_components();
        let max_val = self.options.depth.max_value();
        let colorspace = self.options.colorspace;

        let header = match version
        {
            PPMVersions::P5 | PPMVersions::P6 =>
            {
                format!("{version}\n{width}\n{height}\n{max_val}\n")
            }
            PPMVersions::P7 =>
            {
                let tuple_type = convert_tuple_type_to_pam(colorspace);

                format!(
                    "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH {components}\nMAXVAL {max_val}\nTUPLTYPE {tuple_type}\n ENDHDR\n",
                )
            }
        };

        stream.write_all(header.as_bytes()).unwrap();

        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, PPMEncodeErrors>
    {
        let expected = calc_expected_size(self.options);
        let found = self.data.len();

        if expected != found
        {
            return Err(PPMEncodeErrors::TooShortInput(expected, found));
        }
        let out_size = calc_out_size(self.options);

        let mut out = vec![0; out_size];

        let mut stream = ZByteWriter::new(&mut out);

        self.encode_headers(&mut stream)?;

        match self.options.depth.bit_type()
        {
            BitType::Eight => stream.write_all(self.data).unwrap(),
            BitType::Sixteen =>
            {
                // chunk in two and write to stream
                for slice in self.data.chunks_exact(2)
                {
                    let byte = u16::from_ne_bytes(slice.try_into().unwrap());
                    stream.write_u16_be(byte)
                }
            }
            _ => unreachable!()
        }
        assert!(!stream.eof());
        let position = stream.position();

        // truncate to how many bytes we wrote
        out.truncate(position);

        Ok(out)
    }
}

fn version_for_colorspace(colorspace: ColorSpace) -> Option<PPMVersions>
{
    match colorspace
    {
        ColorSpace::Luma => Some(PPMVersions::P5),
        ColorSpace::RGB => Some(PPMVersions::P6),
        ColorSpace::RGBA | ColorSpace::LumaA => Some(PPMVersions::P7),
        _ => None
    }
}

fn convert_tuple_type_to_pam(colorspace: ColorSpace) -> &'static str
{
    match colorspace
    {
        ColorSpace::Luma => "GRAYSCALE",
        ColorSpace::RGB => "RGB",
        ColorSpace::LumaA => "GRAYSCALE_ALPHA",
        ColorSpace::RGBA => "RGB_ALPHA",
        _ => unreachable!()
    }
}

const PPM_HEADER_SIZE: usize = 100;

#[inline]
fn calc_out_size(options: EncoderOptions) -> usize
{
    options
        .width
        .checked_mul(options.depth.size_of())
        .unwrap()
        .checked_mul(options.height)
        .unwrap()
        .checked_mul(options.colorspace.num_components())
        .unwrap()
        .checked_add(PPM_HEADER_SIZE)
        .unwrap()
}

fn calc_expected_size(options: EncoderOptions) -> usize
{
    calc_out_size(options).checked_sub(PPM_HEADER_SIZE).unwrap()
}
