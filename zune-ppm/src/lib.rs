use std::fmt::{Display, Formatter};
use std::io::Write;

use zune_core::colorspace::ColorSpace;
use zune_core::colorspace::ColorSpace::{GrayScale, RGB};

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
    width:      usize,
    height:     usize,
    writer:     &'a mut W,
}

impl<'a, W: Write> PPMEncoder<'a, W>
{
    pub fn new(width: usize, height: usize, dest: &'a mut W) -> PPMEncoder<W>
    {
        Self {
            version: PPMVersions::P6,
            colorspace: ColorSpace::RGB,
            writer: dest,
            width,
            height,
        }
    }
    pub fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        if self.colorspace == GrayScale
        {
            self.version = PPMVersions::P5;
        }
        else if colorspace == RGB
        {
            self.version = PPMVersions::P6;
        }
        self.colorspace = colorspace;
    }
    pub fn write_headers(&mut self)
    {
        let header = format!("{} {} {} 255\n", self.version, self.width, self.height);

        self.writer.write_all(header.as_bytes()).unwrap();
    }
    pub fn write_rgb(&mut self, (r, g, b): (&[u8], &[u8], &[u8]))
    {
        self.write_headers();
        r.iter()
            .zip(g.iter())
            .zip(b.iter())
            .for_each(|((a, b), c)| {
                self.writer.write_all(&[*a, *b, *c]).unwrap();
            });
    }
    pub fn write_rgb_interleaved(&mut self, rgb: &[u8])
    {
        let header = format!("P6 {} {} 255\n", self.width, self.height);

        self.writer.write_all(header.as_bytes()).unwrap();
        self.writer.write_all(rgb).unwrap();
    }

    pub fn write_grayscale(&mut self, g: &[u8])
    {
        let header = format!("P5 {} {} 255\n", self.width, self.height);

        self.writer.write_all(header.as_bytes()).unwrap();
        self.writer.write_all(g).unwrap();
    }
}
