use zune_core::colorspace::ColorSpace;
#[non_exhaustive]
pub enum ImageChannels
{
    OneChannel(Vec<u8>),
    TwoChannels([Vec<u8>; 2]),
    ThreeChannels([Vec<u8>; 3]),
    FourChannels([Vec<u8>; 4]),
    Interleaved(Vec<u8>),
    Uninitialized,
}
impl ImageChannels
{
    /// Get three pixels from the buffer
    /// but return `default` if the access would be out of bounds.
    ///
    pub fn get_three_channel_pixel_no_fail(
        &self, x: usize, y: usize, stride: usize, default: u8,
    ) -> (u8, u8, u8)
    {
        let (mut c1, mut c2, mut c3) = (default, default, default);

        if let ImageChannels::ThreeChannels(channels) = self
        {
            let pos = y * stride + x;

            c1 = *channels[0].get(pos).unwrap_or(&default);
            c2 = *channels[1].get(pos).unwrap_or(&default);
            c3 = *channels[2].get(pos).unwrap_or(&default);
        }
        (c1, c2, c3)
    }

    pub fn apply_three_channels_inplace<F>(&mut self, function: F) -> Option<()>
    where
        F: Fn(&mut u8, &mut u8, &mut u8),
    {
        if let ImageChannels::ThreeChannels(channels) = self
        {
            let (c0, rem) = channels.split_at_mut(1);
            let (c1, c2) = rem.split_at_mut(1);

            c0[0]
                .iter_mut()
                .zip(c1[0].iter_mut())
                .zip(c2[0].iter_mut())
                .for_each(|((a, b), c)| {
                    function(a, b, c);
                });

            return Some(());
        }
        None
    }
    /// Return true or false whether the color components have been separated
    /// Into different components.
    pub const fn is_interleaved(&self) -> bool
    {
        matches!(self, Self::Uninitialized | Self::Interleaved(_))
    }
}

pub struct Image
{
    colorspace: ColorSpace,
    pixels:     ImageChannels,
    dimensions: (usize, usize),
}
impl Default for Image
{
    fn default() -> Self
    {
        Image {
            colorspace: ColorSpace::Unknown,
            pixels:     ImageChannels::Uninitialized,
            dimensions: (0, 0),
        }
    }
}
impl Image
{
    pub fn new() -> Image
    {
        Image::default()
    }

    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.colorspace
    }
    pub fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        self.colorspace = colorspace;
    }

    pub const fn get_channel_ref(&self) -> &ImageChannels
    {
        &self.pixels
    }
    pub fn get_channel_mut(&mut self) -> &mut ImageChannels
    {
        &mut self.pixels
    }
    pub fn set_image_channel(&mut self, pixels: ImageChannels)
    {
        self.pixels = pixels;
    }

    pub const fn get_dimensions(&self) -> (usize, usize)
    {
        self.dimensions
    }
    pub fn set_dimensions(&mut self, width: usize, height: usize)
    {
        self.dimensions = (width, height);
    }
}
