use std::fmt::{Display, Formatter};
use std::io::Write;

use zune_core::colorspace::ColorSpace;
use zune_core::colorspace::ColorSpace::{Luma, RGB};

enum PPMVersions
{
    P5,
    P6,
}
impl Display for PPMVersions
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::P6 => write!(f, "P6"),
            Self::P5 => write!(f, "P5"),
        }
    }
}
pub struct PPMEncoder<'a, W: Write>
{
    version:    PPMVersions,
    colorspace: ColorSpace,

    width:  usize,
    height: usize,
    writer: &'a mut W,
}

impl<'a, W: Write> PPMEncoder<'a, W>
{
    pub fn new(
        width: usize, height: usize, colorspace: ColorSpace, dest: &'a mut W,
    ) -> PPMEncoder<W>
    {
        Self {
            version: PPMVersions::P6,
            colorspace,
            writer: dest,
            width,
            height,
        }
    }
    fn set_colorspace(&mut self)
    {
        if self.colorspace == Luma
        {
            self.version = PPMVersions::P5;
        }
        else if self.colorspace == RGB
        {
            self.version = PPMVersions::P6;
        }
        else
        {
            panic!();
        }
    }
    pub fn write_headers(&mut self)
    {
        self.set_colorspace();

        let header = format!("{} {} {} 255\n", self.version, self.width, self.height);

        self.writer.write_all(header.as_bytes()).unwrap();
    }
    pub fn write(&mut self, data: &[u8])
    {
        self.write_headers();
        self.writer.write_all(data).unwrap();
    }
}
