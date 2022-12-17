use clap::builder::PossibleValue;
use clap::ValueEnum;
use zune_core::colorspace::ColorSpace;

#[derive(Copy, Clone, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum IColorSpace
{
    RGB,
    GRAYSCALE,
    YCbCr,
    RGBA,
    RGBX
}
impl IColorSpace
{
    #[allow(dead_code)]
    pub const fn to_colorspace(self) -> ColorSpace
    {
        match self
        {
            IColorSpace::RGB => ColorSpace::RGB,
            IColorSpace::GRAYSCALE => ColorSpace::Luma,
            IColorSpace::YCbCr => ColorSpace::YCbCr,
            IColorSpace::RGBA => ColorSpace::RGBA,
            IColorSpace::RGBX => ColorSpace::RGBX
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
            Self::GRAYSCALE
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
            Self::GRAYSCALE => PossibleValue::new("grayscale")
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
