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
use log::info;
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;
use zune_imageprocs::traits::NumOps;

use crate::channel::Channel;
use crate::errors::ImgErrors;
use crate::impls::depth::Depth;
use crate::traits::{OperationsTrait, ZuneInts};

/// Maximum supported color channels
pub const MAX_CHANNELS: usize = 4;

/// Represents a single image
#[derive(Clone)]
pub struct Image
{
    pub(crate) channels:   Vec<Channel>,
    pub(crate) depth:      BitDepth,
    pub(crate) width:      usize,
    pub(crate) height:     usize,
    pub(crate) colorspace: ColorSpace
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
        info!("Called flatten operation");
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

    /// Force flattening to RGBA
    ///
    /// This internally converts channel to a u8 representation if it's not
    /// in that value
    pub fn flatten_rgba(&mut self, out_pixel: &mut [u8])
    {
        if self.depth != BitDepth::Eight
        {
            // convert depth if it doesn't match
            let operation = Depth::new(BitDepth::Eight);

            operation.execute(self).unwrap();
        }

        match self.colorspace.num_components()
        {
            1 =>
            {
                let luma_channel = self.channels[0].reinterpret_as::<u8>().unwrap();

                for (out, luma) in out_pixel.chunks_exact_mut(4).zip(luma_channel)
                {
                    out[0] = *luma;
                    out[1] = *luma;
                    out[2] = *luma;
                    out[3] = 255;
                }
            }
            2 =>
            {
                let luma_channel = self.channels[0].reinterpret_as::<u8>().unwrap();
                let alpha_channel = self.channels[1].reinterpret_as::<u8>().unwrap();

                for ((out, luma), alpha) in out_pixel
                    .chunks_exact_mut(4)
                    .zip(luma_channel)
                    .zip(alpha_channel)
                {
                    out[0] = *luma;
                    out[1] = *luma;
                    out[2] = *luma;
                    out[3] = *alpha;
                }
            }
            3 =>
            {
                let c1 = self.channels[0].reinterpret_as::<u8>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u8>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u8>().unwrap();

                for (((out, first), second), third) in
                    out_pixel.chunks_exact_mut(4).zip(c1).zip(c2).zip(c3)
                {
                    out[0] = *first;
                    out[1] = *second;
                    out[2] = *third;
                    out[3] = 255;
                }
            }
            4 =>
            {
                let c1 = self.channels[0].reinterpret_as::<u8>().unwrap();
                let c2 = self.channels[1].reinterpret_as::<u8>().unwrap();
                let c3 = self.channels[2].reinterpret_as::<u8>().unwrap();
                let c4 = self.channels[3].reinterpret_as::<u8>().unwrap();

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
    /// Create an image from a function
    ///
    /// The image width , height and colorspace need to be specified
    ///
    /// The function will receive two parameters, the first is the current x offset and y offset
    /// and for each it's expected to return  an array with `MAX_CHANNELS`
    ///
    /// # Limitations.
    ///
    /// Due to constrains imposed by the library, the response has to be an array containing
    /// [MAX_CHANNELS], depending on the number of components the colorspace uses
    /// some elements may be ignored.
    ///
    /// E.g for the following code
    ///
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::image::{Image, MAX_CHANNELS};
    ///
    /// fn dead_simple(x:usize,y:usize)->[u8;MAX_CHANNELS]
    /// {    
    ///     let mut arr = [0;MAX_CHANNELS];
    ///     // this will create a linear band of colors from black to white and repeats
    ///     // until the whole image is visited
    ///     let luma = ((x+y) % 256) as u8;
    ///     arr[0] = luma;
    ///     // then return it
    ///     arr    
    /// }
    /// let img  = Image::from_fn(30,20,ColorSpace::Luma,dead_simple);
    /// ```
    /// We only set one element in our array but need to return an array with
    /// [MAX_CHANNELS] elements
    ///
    /// [MAX_CHANNELS]:MAX_CHANNELS
    pub fn from_fn<F, T>(width: usize, height: usize, colorspace: ColorSpace, func: F) -> Image
    where
        F: Fn(usize, usize) -> [T; MAX_CHANNELS],
        T: ZuneInts<T> + Copy + Clone
    {
        match colorspace.num_components()
        {
            1 => Image::from_fn_inner::<_, _, 1>(width, height, func, colorspace),
            2 => Image::from_fn_inner::<_, _, 2>(width, height, func, colorspace),
            3 => Image::from_fn_inner::<_, _, 3>(width, height, func, colorspace),
            4 => Image::from_fn_inner::<_, _, 4>(width, height, func, colorspace),
            _ => unreachable!()
        }
    }

    /// Template code to use with from_fn which engraves component number
    /// as a constant in compile time.
    ///
    /// This allows further optimizations by the compiler
    /// like removing bounds check in the inner loop
    fn from_fn_inner<F, T, const COMPONENTS: usize>(
        width: usize, height: usize, func: F, colorspace: ColorSpace
    ) -> Image
    where
        F: Fn(usize, usize) -> [T; MAX_CHANNELS],
        T: ZuneInts<T> + Copy + Clone
    {
        let size = width * height * COMPONENTS * T::depth().size_of();

        let mut channels = vec![Channel::new_with_capacity(size); COMPONENTS];

        let channels_ref: &mut [Channel; COMPONENTS] =
            channels.get_mut(0..COMPONENTS).unwrap().try_into().unwrap();

        for y in 0..height
        {
            for x in 0..width
            {
                let value = (func)(x, y);

                for i in 0..COMPONENTS
                {
                    channels_ref[i].push(value[i]);
                }
            }
        }

        Image::new(channels, T::depth(), width, height, colorspace)
    }
}
