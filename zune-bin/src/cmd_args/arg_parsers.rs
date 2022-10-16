use clap::builder::PossibleValue;
use clap::{ArgMatches, ValueEnum};
use zune_core::colorspace::ColorSpace;
use zune_jpeg::ZuneJpegOptions;

#[derive(Copy, Clone, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum IColorSpace
{
    RGB,
    GRAYSCALE,
    YCbCr,
    RGBA,
    RGBX,
}
impl IColorSpace
{
    pub const fn to_colorspace(self) -> ColorSpace
    {
        match self
        {
            IColorSpace::RGB => ColorSpace::RGB,
            IColorSpace::GRAYSCALE => ColorSpace::Luma,
            IColorSpace::YCbCr => ColorSpace::YCbCr,
            IColorSpace::RGBA => ColorSpace::RGBA,
            IColorSpace::RGBX => ColorSpace::RGBX,
        }
    }
}
impl ValueEnum for IColorSpace
{
    fn value_variants<'a>() -> &'a [Self]
    {
        &[
            Self::RGBX,
            Self::RGBA,
            Self::RGB,
            Self::YCbCr,
            Self::GRAYSCALE,
        ]
    }

    fn to_possible_value(&self) -> Option<PossibleValue>
    {
        Some(match self
        {
            Self::RGBX => PossibleValue::new("rgbx"),
            Self::RGBA => PossibleValue::new("rgba"),
            Self::RGB => PossibleValue::new("rgb"),
            Self::YCbCr => PossibleValue::new("ycbcr"),
            Self::GRAYSCALE => PossibleValue::new("grayscale"),
        })
    }
}
impl std::str::FromStr for IColorSpace
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        for variant in Self::value_variants()
        {
            if variant.to_possible_value().unwrap().matches(s, false)
            {
                return Ok(*variant);
            }
        }
        Err(format!("Invalid variant: {}", s))
    }
}
pub fn get_four_pair_args(input: &str) -> Result<[usize; 4], String>
{
    // format like imagemagick
    //[width]:[height]:[+x]:[+y]
    let mut result = [0; 4];

    let split = input.split(':');
    let mut counter = 0;
    for (single, pos) in split.zip(result.iter_mut())
    {
        *pos = str::parse::<usize>(single).map_err(|x| x.to_string())?;
        counter += 1;
    }
    if counter != 4
    {
        return Err(format!("Not enough arguments for crop {counter}"));
    }

    Ok(result)
}

pub fn parse_options_to_jpeg(arguments: &ArgMatches) -> ZuneJpegOptions
{
    let max_height = *arguments
        .get_one::<usize>("max-height")
        .unwrap_or(&(1 << 16));
    let max_width = *arguments
        .get_one::<usize>("max-width")
        .unwrap_or(&(1 << 16));
    let colorspace = *arguments
        .get_one::<IColorSpace>("colorspace")
        .unwrap_or(&IColorSpace::RGB);
    let use_strict = *arguments.get_one::<bool>("strict").unwrap_or(&false);

    let z_colorspace = colorspace.to_colorspace();

    ZuneJpegOptions::new()
        .set_out_colorspace(z_colorspace)
        .set_max_height(max_height as u16)
        .set_max_width(max_width as u16)
        .set_strict_mode(use_strict)
}
