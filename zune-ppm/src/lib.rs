use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::io::{Error, Write};

use zune_core::colorspace::ColorSpace;

/// Errors occurring during encoding
pub enum PPMErrors
{
    Static(&'static str),
    IOErrors(io::Error)
}

impl From<io::Error> for PPMErrors
{
    fn from(err: Error) -> Self
    {
        PPMErrors::IOErrors(err)
    }
}

impl Debug for PPMErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            PPMErrors::Static(ref errors) =>
            {
                writeln!(f, "{}", errors)
            }
            PPMErrors::IOErrors(ref err) =>
            {
                writeln!(f, "{}", err)
            }
        }
    }
}

enum PPMVersions
{
    P5,
    P6
}

impl Display for PPMVersions
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::P6 => write!(f, "P6"),
            Self::P5 => write!(f, "P5")
        }
    }
}

/// A PPM encoder
pub struct PPMEncoder<'a, W: Write>
{
    writer: &'a mut W
}

impl<'a, W: Write> PPMEncoder<'a, W>
{
    pub fn new(writer: &'a mut W) -> PPMEncoder<'a, W>
    {
        Self { writer }
    }

    fn write_headers(
        &mut self, version: PPMVersions, width: usize, height: usize, max_val: usize
    ) -> Result<(), PPMErrors>
    {
        let header = format!("{} {} {} {}\n", version, width, height, max_val);

        self.writer.write_all(header.as_bytes())?;

        Ok(())
    }

    pub fn encode_ppm(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u8]
    ) -> Result<(), PPMErrors>
    {
        let version = get_ppm_version(colorspace)?;

        self.write_headers(version, width, height, 255)?;
        self.writer.write_all(data)?;

        Ok(())
    }

    pub fn encode_ppm_u16(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u16]
    ) -> Result<(), PPMErrors>
    {
        let version = get_ppm_version(colorspace)?;

        self.write_headers(version, width, height, 65535)?;

        // NOTE: Cae we can save memory here, but it becomes slow
        // so we cheat by having our own copy

        // Convert to big endian since netbpm uses big endian
        // so we emulate that
        let owned_data = data
            .iter()
            .flat_map(|x| x.to_be_bytes())
            .collect::<Vec<u8>>();

        self.writer.write_all(&owned_data)?;

        Ok(())
    }
}

/// A PAM encoder.
pub struct PAMEncoder<'a, W>
{
    writer: &'a mut W
}

impl<'a, W: Write> PAMEncoder<'a, W>
{
    pub fn new(writer: &'a mut W) -> PAMEncoder<'a, W>
    {
        Self { writer }
    }
    fn write_headers_pam(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, depth: usize
    ) -> Result<(), PPMErrors>
    {
        let tuple_type = convert_tuple_type_to_pam(colorspace);

        let header = format!(
            "P7 WIDTH {} HEIGHT {} DEPTH {} MAXVAL {} TUPLETYPE {} ENDHDR\n",
            width,
            height,
            colorspace.num_components(),
            depth,
            tuple_type
        );
        self.writer.write_all(header.as_bytes())?;

        Ok(())
    }
    pub fn encode_pam(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u8]
    ) -> Result<(), PPMErrors>
    {
        self.write_headers_pam(width, height, colorspace, 255)?;

        self.writer.write_all(data)?;

        Ok(())
    }

    /// Encode u16's as a PAM file
    ///
    /// Layout of the bits are big endian to emulate netbpm style.
    pub fn encode_pam_u16(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u16]
    ) -> Result<(), PPMErrors>
    {
        self.write_headers_pam(width, height, colorspace, 65535)?;

        // NOTE: Cae we can save memory here, but it becomes slow
        // so we cheat by having our own copy

        // Convert to big endian since netbpm uses big endian
        // so we emulate that
        let owned_data = data
            .iter()
            .flat_map(|x| x.to_be_bytes())
            .collect::<Vec<u8>>();

        self.writer.write_all(&owned_data)?;

        Ok(())
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

fn get_ppm_version(colorspace: ColorSpace) -> Result<PPMVersions, PPMErrors>
{
    if colorspace == ColorSpace::Luma
    {
        Ok(PPMVersions::P5)
    }
    else if colorspace == ColorSpace::RGB
    {
        Ok(PPMVersions::P6)
    }
    else
    {
        Err(PPMErrors::Static("Unsupported colorspace for PPM"))
    }
}
