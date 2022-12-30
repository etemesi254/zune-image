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
                writeln!(f, "{errors}")
            }
            PPMErrors::IOErrors(ref err) =>
            {
                writeln!(f, "{err}")
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
pub struct PPMEncoder<'a, W: Write>
{
    writer: &'a mut W
}

impl<'a, W: Write> PPMEncoder<'a, W>
{
    /// Create a new PPM encoder that writes to `writer`
    pub fn new(writer: &'a mut W) -> PPMEncoder<'a, W>
    {
        Self { writer }
    }

    /// Write Headers for P5 and P6 formats
    fn write_headers(
        &mut self, version: PPMVersions, width: usize, height: usize, max_val: usize
    ) -> Result<(), PPMErrors>
    {
        let header = format!("{version}\n{width}\n{height}\n{max_val}\n");

        self.writer.write_all(header.as_bytes())?;

        Ok(())
    }

    /// Write headers for P7 format
    fn write_headers_pam(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, depth: usize
    ) -> Result<(), PPMErrors>
    {
        let tuple_type = convert_tuple_type_to_pam(colorspace);

        let header = format!(
            "P7\nWIDTH {}\nHEIGHT {}\nDEPTH {}\nMAXVAL {}\nTUPLTYPE {}\n ENDHDR\n",
            width,
            height,
            colorspace.num_components(),
            depth,
            tuple_type
        );
        self.writer.write_all(header.as_bytes())?;

        Ok(())
    }
    fn encode_pam_u8(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u8]
    ) -> Result<(), PPMErrors>
    {
        self.write_headers_pam(width, height, colorspace, 255)?;

        self.writer.write_all(data)?;

        Ok(())
    }
    fn encode_pam_u16(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, data: &[u16]
    ) -> Result<(), PPMErrors>
    {
        self.write_headers_pam(width, height, colorspace, 255)?;

        // Create big endian bytes from data
        let owned_data = data
            .iter()
            .flat_map(|x| x.to_be_bytes())
            .collect::<Vec<u8>>();

        self.writer.write_all(&owned_data)?;

        Ok(())
    }

    /// Encode`data` as 8 bit PPM file
    ///
    /// Recommended version is P6, which allows one to save RGB
    pub fn encode_u8(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, version: PPMVersions,
        data: &[u8]
    ) -> Result<(), PPMErrors>
    {
        if width * height * colorspace.num_components() != data.len()
        {
            return Err(PPMErrors::Static(
                "Data length does not match image dimensions"
            ));
        }
        match version
        {
            PPMVersions::P5 | PPMVersions::P6 =>
            {
                let version = get_ppm_version(colorspace)?;

                self.write_headers(version, width, height, 255)?;
                self.writer.write_all(data)?;
            }
            PPMVersions::P7 =>
            {
                self.encode_pam_u8(width, height, colorspace, data)?;
            }
        }

        Ok(())
    }

    /// Encode data as 16 bit PPM
    fn encode_ppm_u16(
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

    pub fn encode_u16(
        &mut self, width: usize, height: usize, colorspace: ColorSpace, version: PPMVersions,
        data: &[u16]
    ) -> Result<(), PPMErrors>
    {
        match version
        {
            PPMVersions::P5 | PPMVersions::P6 =>
            {
                self.encode_ppm_u16(width, height, colorspace, data)
            }

            PPMVersions::P7 => self.encode_pam_u16(width, height, colorspace, data)
        }
    }
}

pub fn version_for_colorspace(colorspace: ColorSpace) -> Option<PPMVersions>
{
    match colorspace
    {
        ColorSpace::Luma => Some(PPMVersions::P5),
        ColorSpace::RGB => Some(PPMVersions::P6),
        ColorSpace::RGBA | ColorSpace::RGBX | ColorSpace::LumaA => Some(PPMVersions::P7),
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
