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
use std::mem::size_of;

use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::{ColorCharacteristics, ColorSpace};
use zune_imageprocs::traits::NumOps;

use crate::channel::Channel;
use crate::deinterleave::{deinterleave_u16, deinterleave_u8};
use crate::errors::ImageErrors;
use crate::frame::Frame;
use crate::impls::depth::Depth;
use crate::metadata::ImageMetadata;
use crate::traits::{OperationsTrait, ZuneInts};

/// Maximum supported color channels
pub const MAX_CHANNELS: usize = 4;

/// Represents a single image
#[derive(Clone)]
pub struct Image
{
    pub(crate) frames:   Vec<Frame>,
    pub(crate) metadata: ImageMetadata
}

impl Image
{
    /// Create a new image channel
    pub fn new(
        channels: Vec<Channel>, depth: BitDepth, width: usize, height: usize,
        colorspace: ColorSpace
    ) -> Image
    {
        // setup metadata information
        let mut meta = ImageMetadata::default();

        meta.set_dimensions(width, height);
        meta.set_depth(depth);
        meta.set_colorspace(colorspace);

        Image {
            frames:   vec![Frame::new(channels)],
            metadata: meta
        }
    }

    /// Return true if the current image contains more than
    /// one frame indicating it is animated
    ///
    /// # Returns
    ///  
    /// - true : Image contains a series of frames which can be animated
    /// - false: Image contains a single frame  
    ///
    pub fn is_animated(&self) -> bool
    {
        self.frames.len() > 1
    }
    /// Get image dimensions as a tuple of (width,height)
    pub const fn get_dimensions(&self) -> (usize, usize)
    {
        self.metadata.get_dimensions()
    }

    /// Get the image depth of this image
    pub const fn get_depth(&self) -> BitDepth
    {
        self.metadata.get_depth()
    }
    /// Set image depth
    pub fn set_depth(&mut self, depth: BitDepth)
    {
        self.metadata.set_depth(depth)
    }
    pub fn set_color_trc(&mut self, color_trc: ColorCharacteristics)
    {
        self.metadata.set_color_trc(color_trc);
    }
    /// Set default gamma for this image
    ///
    /// For more information see [ImageMetadata::set_gamma](crate::metadata::ImageMetadata::set_default_gamma)
    pub fn set_default_gamma(&mut self, gamma: f32)
    {
        self.metadata.set_default_gamma(gamma)
    }
    /// Return the image's  gamma value.
    ///
    ///This is the value that will be used to convert images to linear
    ///image in case
    pub const fn get_default_gamma(&self) -> Option<f32>
    {
        self.metadata.default_gamma
    }
    pub const fn get_metadata(&self) -> &ImageMetadata
    {
        &self.metadata
    }

    /// Return an immutable reference to all image frames
    ///
    /// # Returns
    /// All frames in the image
    ///
    ///
    pub fn get_frames_ref(&self) -> &[Frame]
    {
        &self.frames
    }

    pub fn get_frames_mut(&mut self) -> &mut [Frame]
    {
        &mut self.frames
    }
    /// Return a reference to the underlying channels
    pub fn get_channels_ref(&self, ignore_alpha: bool) -> Vec<&Channel>
    {
        let colorspace = self.get_colorspace();

        self.frames
            .iter()
            .flat_map(|x| x.get_channels_ref(colorspace, ignore_alpha))
            .collect()
    }

    /// Return a mutable view into the image channels
    ///
    /// This gives mutable access to the chanel data allowing
    /// single or multithreaded manipulation of images
    pub fn get_channels_mut(&mut self, ignore_alpha: bool) -> Vec<&mut Channel>
    {
        let colorspace = self.get_colorspace();

        self.frames
            .iter_mut()
            .flat_map(|x| x.get_channels_mut(colorspace, ignore_alpha))
            .collect()
    }
    /// Get the colorspace this image is stored
    /// in
    pub const fn get_colorspace(&self) -> ColorSpace
    {
        self.metadata.colorspace
    }
    /// Flatten channels in this image.
    ///
    /// Flatten can be used to interleave all channels into one vector
    pub fn flatten_frames<T: Default + Copy + 'static + ZuneInts<T>>(&self) -> Vec<Vec<T>>
    {
        //
        assert_eq!(self.metadata.get_depth().size_of(), size_of::<T>());
        let colorspace = self.get_colorspace();

        self.frames.iter().map(|x| x.flatten(colorspace)).collect()
    }
    /// Convert image to a byte representation interleaving
    /// image pixels where necessary
    ///
    /// # Note
    /// For images using anything larger than 8 bit,
    /// u8 as native endian is used
    /// i.e RGB data looks like `[R,R,G,G,G,B,B]`
    pub fn to_u8(&self) -> Vec<Vec<u8>>
    {
        let colorspace = self.get_colorspace();
        if self.metadata.get_depth() == BitDepth::Eight
        {
            self.flatten_frames::<u8>()
        }
        else if self.metadata.get_depth().size_of() == 2
        {
            self.frames
                .iter()
                .map(|z| z.u16_to_native_endian(colorspace))
                .collect()
        }
        else
        {
            todo!("Unimplemented")
        }
    }

    /// Force flattening of all frames to RGBA format
    ///
    /// This will iterate through all
    pub fn flatten_rgba_frames_u8(&mut self, out_pixel: Vec<&mut [u8]>)
    {
        if self.metadata.depth != BitDepth::Eight
        {
            // convert depth if it doesn't match
            let operation = Depth::new(BitDepth::Eight);

            operation.execute(self).unwrap();
        }
        let colorspace = self.get_colorspace();

        for (frame, out) in self.frames.iter_mut().zip(out_pixel)
        {
            frame.flatten_rgba(colorspace, out).unwrap();
        }
    }
    pub fn set_dimensions(&mut self, width: usize, height: usize)
    {
        self.metadata.set_dimensions(width, height);
    }

    pub fn set_colorspace(&mut self, colorspace: ColorSpace)
    {
        self.metadata.set_colorspace(colorspace);
    }

    /// Create an image with a static color in it
    pub fn fill<T: Copy + Clone + NumOps<T> + 'static + ZuneInts<T>>(
        pixel: T, depth: BitDepth, colorspace: ColorSpace, width: usize, height: usize
    ) -> Result<Image, ImageErrors>
    {
        if core::mem::size_of::<T>() != depth.size_of()
        {
            return Err(ImageErrors::from(
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
        T: ZuneInts<T> + Copy + Clone + 'static
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
        T: ZuneInts<T> + Copy + Clone + 'static
    {
        let size = width * height * COMPONENTS * T::depth().size_of();

        let mut channels = vec![Channel::new_with_capacity::<T>(size); COMPONENTS];

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

// Conversions
impl Image
{
    /// Create a new image from a raw pixels
    ///
    /// The image depth is treated as [BitDepth::U8](zune_core::bit_depth::BitDepth::Eight)
    /// and formats which pack images into lower bit-depths are expected to expand them before
    /// using this function
    ///
    /// Pixels are expected to be interleaved according to the colorspace
    /// I.e if the image is RGB, pixel layout should be `[R,G,B,R,G,B]`
    /// if it's Luma with alpha, pixel layout should be `[L,A,L,A]`
    ///
    /// # Returns
    /// An [`Image`](crate::image::Image) struct
    ///
    /// # Panics
    /// - In case calculating image dimensions overflows a [`usize`]
    /// this indicates that the array cannot be indexed by usize,hence values are invalid
    ///
    /// - If the length of pixels doesn't match the expected length
    pub fn from_u8(pixels: &[u8], width: usize, height: usize, colorspace: ColorSpace) -> Image
    {
        let expected_len = checked_mul(width, height, 1, colorspace.num_components());

        assert_eq!(
            pixels.len(),
            expected_len,
            "Length mismatch, expected {expected_len} but found {} ",
            pixels.len()
        );

        let pixels = deinterleave_u8(pixels, colorspace).unwrap();

        Image::new(pixels, BitDepth::Eight, width, height, colorspace)
    }
    /// Create an image from raw u16 pixels
    ///
    /// Pixels are expected to be interleaved according to number of components in the colorspace
    ///
    /// e.g if image is RGBA, pixels should be in the form of `[R,G,B,A,R,G,B,A]`
    ///
    /// The bit depth should not be [BitDepth::U8](zune_core::bit_depth::BitDepth::Eight), this
    /// function will panic if so
    ///
    /// # Returns
    /// An [`Image`](crate::image::Image) struct
    ///
    ///
    /// # Panics
    /// - If calculating image dimensions will overflow [`usize`]
    ///
    /// - If image `depth.size_of()` is not 2
    ///
    /// - If pixels length is not equal to expected length
    pub fn from_u16(
        pixels: &[u16], width: usize, height: usize, depth: BitDepth, colorspace: ColorSpace
    ) -> Image
    {
        let expected_len = checked_mul(width, height, 1, colorspace.num_components());

        assert_eq!(
            pixels.len(),
            expected_len,
            "Length mismatch, expected {expected_len} but found {} ",
            pixels.len()
        );
        assert_eq!(depth.size_of(), 2);

        let pixels = deinterleave_u16(pixels, colorspace).unwrap();

        Image::new(pixels, depth, width, height, colorspace)
    }
}

fn checked_mul(width: usize, height: usize, depth: usize, colorspace_components: usize) -> usize
{
    width
        .checked_mul(height)
        .unwrap()
        .checked_mul(depth)
        .unwrap()
        .checked_mul(colorspace_components)
        .unwrap()
}
