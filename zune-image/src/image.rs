use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;

pub struct Image
{
    channels:   Vec<Vec<u16>>,
    depth:      BitDepth,
    width:      usize,
    height:     usize,
    colorspace: ColorSpace,
}
impl Image
{
    pub fn new(
        channels: Vec<Vec<u16>>, depth: BitDepth, width: usize, height: usize,
        colorspace: ColorSpace,
    ) -> Image
    {
        Image {
            channels,
            depth,
            width,
            height,
            colorspace,
        }
    }
    /// Get image dimensions as a tuple of (width,height)
    pub const fn get_dimensions(&self) -> (usize, usize)
    {
        (self.width, self.height)
    }
    /// Get the image depth of this image
    pub const fn get_depth(&self) -> BitDepth
    {
        self.depth
    }
    pub fn get_channels_ref(&self, alpha: bool) -> &[Vec<u16>]
    {
        // check if alpha channel is present in colorspace
        if alpha && self.colorspace.has_alpha()
        {
            // do not take the last one,
            // we assume the last one contains the alpha channel
            // in it.
            // TODO: Is this a bad assumption
            &self.channels[0..self.colorspace.num_components() - 1]
        }
        else
        {
            &self.channels[0..self.colorspace.num_components()]
        }
    }

    /// Return a mutable view into the image channels
    ///
    /// This gives mutable access to the chanel data allowing
    /// single or multithreaded manipulation of images
    pub fn get_channels_mut(&mut self, alpha: bool) -> &mut [Vec<u16>]
    {
        // check if alpha channel is present in colorspace
        if alpha && self.colorspace.has_alpha()
        {
            // do not take the last one,
            // we assume the last one contains the alpha channel
            // in it.
            // TODO: Is this a bad assumption
            &mut self.channels[0..self.colorspace.num_components() - 1]
        }
        else
        {
            &mut self.channels[0..self.colorspace.num_components()]
        }
    }
    /// Get the colorspace this image is stored
    /// in
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.colorspace
    }
    /// Flatten channels in this image.
    ///
    /// This represents all image data in a single
    /// continuous vector of unsigned 16 bit integers
    ///
    /// For 8 byte implementations see
    /// [flatten_u8](Self::flatten_u8)
    pub fn flatten(&self) -> Vec<u16>
    {
        let dims = self.width * self.height * self.colorspace.num_components();

        let mut out_pixel = vec![0; dims];

        match self.colorspace.num_components()
        {
            1 =>
            {
                out_pixel.copy_from_slice(&self.channels[0]);
            }
            2 =>
            {
                let luma_channel = &self.channels[0];
                let alpha_channel = &self.channels[1];

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(2)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0] = *luma;
                    out[1] = *alpha;
                }
            }
            3 =>
            {
                let first_channel = &self.channels[0];
                let second_channel = &self.channels[1];
                let third_channel = &self.channels[2];

                for (((out, first), second), third) in out_pixel
                    .chunks_exact_mut(3)
                    .zip(first_channel)
                    .zip(second_channel)
                    .zip(third_channel)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                }
            }
            4 =>
            {
                let first_channel = &self.channels[0];
                let second_channel = &self.channels[1];
                let third_channel = &self.channels[2];
                let fourth_channel = &self.channels[3];

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(3)
                    .zip(first_channel)
                    .zip(second_channel)
                    .zip(third_channel)
                    .zip(fourth_channel)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                    out[3] = *fourth;
                }
            }
            // panics, all the way down
            _ => unreachable!(),
        }

        out_pixel
    }
    /// Flatten channels in this image
    ///
    /// This represents all image channels in
    /// one continuous vector of unsigned 8 bits
    /// ( 1 byte implementation),
    ///
    /// Channels are interleaved according to the colorspace
    /// i.e if colorspace is RGB, the vector will contain
    /// data in the format `[R,G,B,R,G,B,R,G,B,R,G,B]`
    pub fn flatten_u8(&self) -> Vec<u8>
    {
        let dims = self.width * self.height * self.colorspace.num_components();

        let mut out_pixel = vec![0; dims];

        match self.colorspace.num_components()
        {
            1 =>
            {
                for (out_px, in_px) in out_pixel.iter_mut().zip(&self.channels[0])
                {
                    *out_px = *in_px as u8;
                }
            }
            2 =>
            {
                let luma_channel = &self.channels[0];
                let alpha_channel = &self.channels[1];

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(2)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0] = *luma as u8;
                    out[1] = *alpha as u8;
                }
            }
            3 =>
            {
                let first_channel = &self.channels[0];
                let second_channel = &self.channels[1];
                let third_channel = &self.channels[2];

                for (((out, first), second), third) in out_pixel
                    .chunks_exact_mut(3)
                    .zip(first_channel)
                    .zip(second_channel)
                    .zip(third_channel)
                {
                    out[0] = *first as u8;
                    out[1] = *second as u8;
                    out[2] = *third as u8;
                }
            }
            4 =>
            {
                let first_channel = &self.channels[0];
                let second_channel = &self.channels[1];
                let third_channel = &self.channels[2];
                let fourth_channel = &self.channels[3];

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(3)
                    .zip(first_channel)
                    .zip(second_channel)
                    .zip(third_channel)
                    .zip(fourth_channel)
                {
                    out[0] = *first as u8;
                    out[1] = *second as u8;
                    out[2] = *third as u8;
                    out[3] = *fourth as u8;
                }
            }
            // panics, all the way down
            _ => unreachable!(),
        }

        out_pixel
    }
    pub fn set_dimensions(&mut self, width: usize, height: usize)
    {
        self.width = width;
        self.height = height;
    }

    pub fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        self.colorspace = colorspace;
    }

    pub fn set_channels(&mut self, channels: Vec<Vec<u16>>)
    {
        self.channels = channels;
    }
}
