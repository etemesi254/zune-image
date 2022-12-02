//! This module represents a single image
//!
//!
//! An image is represented as
//!
//! - separated channels
//!     - of a certain bit depth
//!         - representing a colorspace
//!             -    with the same width and height
//!
//! And that's how we represent images.
//! Fully supported bit depths are 8 and 16, see channels for how that happens
//!
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::traits::NumOps;

use crate::channel::Channel;
use crate::errors::ImgErrors;

/// Maximum supported color channels
pub const MAX_CHANNELS: usize = 4;

/// Represents a single image
pub struct Image
{
    channels:   Vec<Channel>,
    depth:      BitDepth,
    width:      usize,
    height:     usize,
    colorspace: ColorSpace
}

impl Image
{
    /// Create a new image channel
    pub fn new(
        channels: Vec<Channel>, depth: BitDepth, width: usize, height: usize,
        colorspace: ColorSpace
    ) -> Image
    {
        Image {
            channels,
            depth,
            width,
            height,
            colorspace
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
    /// Return a reference to the underlying channels
    pub fn get_channels_ref(&self, alpha: bool) -> &[Channel]
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
    pub fn get_channels_mut(&mut self, alpha: bool) -> &mut [Channel]
    {
        // check if alpha channel is present in colorspace
        if !alpha && self.colorspace.has_alpha()
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
    /// Flatten can be used to interleave all channels into one vector
    pub fn flatten<T: Default + Copy>(&self) -> Vec<T>
    {
        let dims = self.width * self.height * self.colorspace.num_components();

        let mut out_pixel = vec![T::default(); dims];

        match self.colorspace.num_components()
        {
            1 => out_pixel.copy_from_slice(self.channels[0].reinterpret_as::<T>().unwrap()),

            2 =>
            {
                let luma_channel = self.channels[0].reinterpret_as::<T>().unwrap();
                let alpha_channel = self.channels[1].reinterpret_as::<T>().unwrap();

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
                let c1 = self.channels[0].reinterpret_as::<T>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<T>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<T>().unwrap();

                for (((out, first), second), third) in
                    out_pixel.chunks_exact_mut(3).zip(c1).zip(c2).zip(c3)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                }
            }
            4 =>
            {
                let c1 = self.channels[0].reinterpret_as::<T>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<T>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<T>().unwrap();
                let c4 = self.channels[3].reinterpret_as::<T>().unwrap();

                for ((((out, first), second), third), fourth) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(c1)
                    .zip(c2)
                    .zip(c3)
                    .zip(c4)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                    out[3] = *fourth;
                }
            }
            // panics, all the way down
            _ => unreachable!()
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

    pub fn set_dimensions(&mut self, width: usize, height: usize)
    {
        self.width = width;
        self.height = height;
    }

    pub fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        self.colorspace = colorspace;
    }

    pub fn set_channels(&mut self, channels: Vec<Channel>)
    {
        self.channels = channels;
    }

    /// Fill the image with
    pub fn fill<T: Copy + Clone + NumOps<T>>(
        pixel: T, depth: BitDepth, colorspace: ColorSpace, width: usize, height: usize
    ) -> Result<Image, ImgErrors>
    {
        if core::mem::size_of::<T>() != depth.size_of()
        {
            return Err(ImgErrors::from(
                "Size of T does not match bit depth, this is invalid"
            ));
        }
        let dims = width * height * depth.size_of();

        let channels = vec![Channel::from_elm::<T>(dims, pixel); colorspace.num_components()];

        let img = Image::new(channels, depth, width, height, colorspace);

        Ok(img)
    }
}
