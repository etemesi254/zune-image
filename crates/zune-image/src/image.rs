/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

//! This module represents a single image, an image can consists of one or more
//!
//! And that's how we represent images.
//! Fully supported bit depths are 8 and 16 and float 32 which are expected to be in the range between 0.0 and 1.0,
//! see [channel](crate::channel) documentation for how that happens
//!
use std::fmt::Debug;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use zune_core::bit_depth::BitDepth;
use zune_core::colorspace::ColorSpace;

use crate::channel::{Channel, ChannelErrors};
use crate::core_filters::colorspace::ColorspaceConv;
use crate::core_filters::depth::Depth;
use crate::deinterleave::{deinterleave_f32, deinterleave_u16, deinterleave_u8};
use crate::errors::ImageErrors;
use crate::frame::Frame;
use crate::metadata::ImageMetadata;
use crate::traits::{OperationsTrait, ZuneInts};

/// Maximum supported color channels
pub const MAX_CHANNELS: usize = 4;

/// Represents a single image
#[derive(Clone)]
pub struct Image {
    pub(crate) frames:   Vec<Frame>,
    pub(crate) metadata: ImageMetadata
}

impl PartialEq<Self> for Image {
    fn eq(&self, other: &Self) -> bool {
        other.frames == self.frames
    }
}
impl Eq for Image {}

impl Image {
    /// Create a new image instance
    ///
    /// This constructs a single image frame (non-animated) with the
    /// configured dimensions,colorspace and depth
    ///  
    pub fn new(
        channels: Vec<Channel>, depth: BitDepth, width: usize, height: usize,
        colorspace: ColorSpace
    ) -> Image {
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
    /// Create an image from multiple frames.
    pub fn new_frames(
        frames: Vec<Frame>, depth: BitDepth, width: usize, height: usize, colorspace: ColorSpace
    ) -> Image {
        // setup metadata information
        let mut meta = ImageMetadata::default();

        meta.set_dimensions(width, height);
        meta.set_depth(depth);
        meta.set_colorspace(colorspace);

        Image {
            frames,
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
    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }
    /// Get image dimensions as a tuple of (width,height)
    pub const fn dimensions(&self) -> (usize, usize) {
        self.metadata.dimensions()
    }

    /// Get the image depth of this image
    pub const fn depth(&self) -> BitDepth {
        self.metadata.depth()
    }
    /// Set image depth
    ///
    /// Ensure that the image is in a certain depth before changing this
    /// otherwise bad things will happen
    pub fn set_depth(&mut self, depth: BitDepth) {
        self.metadata.set_depth(depth)
    }

    /// Return an immutable reference to the metadata of the image
    pub const fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }

    /// Return a mutable reference to the image metadata.
    ///
    /// Do not modify elements like width and height anyhowly, it may corrupt
    /// the image in ways only God knows
    pub fn metadata_mut(&mut self) -> &mut ImageMetadata {
        &mut self.metadata
    }

    /// Return an immutable reference to all image frames
    ///
    /// # Returns
    /// All frames in the image
    ///
    ///
    pub fn frames_ref(&self) -> &[Frame] {
        &self.frames
    }

    /// Return a mutable reference to all image frames.
    ///
    pub fn frames_mut(&mut self) -> &mut [Frame] {
        &mut self.frames
    }
    /// Return a reference to the underlying channels
    pub fn channels_ref(&self, ignore_alpha: bool) -> Vec<&Channel> {
        let colorspace = self.colorspace();

        self.frames_ref()
            .iter()
            .flat_map(|x| x.channels_ref(colorspace, ignore_alpha))
            .collect()
    }

    /// Return a mutable view into the image channels
    ///
    /// This gives mutable access to the chanel data allowing
    /// single or multithreaded manipulation of images
    pub fn channels_mut(&mut self, ignore_alpha: bool) -> Vec<&mut Channel> {
        let colorspace = self.colorspace();

        self.frames_mut()
            .iter_mut()
            .flat_map(|x| x.channels_mut(colorspace, ignore_alpha))
            .collect()
    }
    /// Get the colorspace this image is stored
    /// in
    pub const fn colorspace(&self) -> ColorSpace {
        self.metadata.colorspace
    }
    /// Flatten channels in this image.
    ///
    /// Flatten can be used to interleave all channels into one vector
    pub fn flatten_frames<T: Default + Copy + 'static + Pod>(&self) -> Vec<Vec<T>> {
        //
        assert_eq!(self.metadata.depth().size_of(), size_of::<T>());
        let colorspace = self.colorspace();

        self.frames_ref()
            .iter()
            .map(|x| x.flatten(colorspace))
            .collect()
    }
    /// Convert image to a byte representation interleaving
    /// image pixels where necessary
    ///
    /// # Note
    /// For images using anything larger than 8 bit,
    /// u8 as native endian is used
    /// i.e RGB data looks like `[R,R,G,G,G,B,B]`
    #[allow(dead_code)]
    pub(crate) fn to_u8(&self) -> Vec<Vec<u8>> {
        let colorspace = self.colorspace();
        if self.metadata.depth() == BitDepth::Eight {
            self.flatten_frames::<u8>()
        } else if self.metadata.depth() == BitDepth::Sixteen {
            self.frames_ref()
                .iter()
                .map(|z| z.u16_to_native_endian(colorspace))
                .collect()
        } else {
            todo!("Unimplemented")
        }
    }
    /// Convert the images to
    pub fn flatten_to_u8(&self) -> Vec<Vec<u8>> {
        if self.depth() == BitDepth::Eight {
            self.flatten_frames::<u8>()
        } else {
            let mut im_clone = self.clone();
            Depth::new(BitDepth::Eight).execute(&mut im_clone).unwrap();
            im_clone.flatten_frames::<u8>()
        }
    }
    #[allow(dead_code)]
    pub(crate) fn to_u8_be(&self) -> Vec<Vec<u8>> {
        let colorspace = self.colorspace();
        if self.metadata.depth() == BitDepth::Eight {
            self.flatten_frames::<u8>()
        } else if self.metadata.depth() == BitDepth::Sixteen {
            self.frames_ref()
                .iter()
                .map(|z| z.u16_to_big_endian(colorspace))
                .collect()
        } else {
            todo!("Unimplemented")
        }
    }
    /// Set new image dimensions
    ///
    /// # Warning
    ///
    /// This is potentially dangerous and should be used only when
    /// the underlying channels have been modified.
    ///
    /// # Arguments:
    /// - width: The new image width
    /// - height: The new imag height.
    ///
    /// Modifies the image in place
    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.metadata.set_dimensions(width, height);
    }

    /// Set the colorspace of the image.
    ///
    /// Do not do this without ensuring the image is in that colorspace
    pub(crate) fn set_colorspace(&mut self, colorspace: ColorSpace) {
        self.metadata.set_colorspace(colorspace);
    }

    /// Create an image with a static color in it
    ///
    ///  # Arguments
    /// - pixel: Value to fill the image with
    /// - colorspace: The image colorspace
    /// - width: Image width
    /// - height: Image height
    ///
    ///  # Supported Types
    /// - u8: BitDepth is treated as BitDepth::Eight
    /// - u16: BitDepth is treated as BitDepth::Sixteen
    /// - f32: BitDepth is treated as BitDepth::Float32
    ///
    /// # Example
    /// - Create a 800 by 800 RGB image of type u8
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::image::Image;
    /// let image = Image::fill::<u8>(212,ColorSpace::RGB,800,800);
    /// ```
    ///
    pub fn fill<T>(pixel: T, colorspace: ColorSpace, width: usize, height: usize) -> Image
    where
        T: Copy + Clone + 'static + ZuneInts<T> + Zeroable + Pod
    {
        let dims = width * height;

        let channels = vec![Channel::from_elm::<T>(dims, pixel); colorspace.num_components()];

        Image::new(channels, T::depth(), width, height, colorspace)
    }
    /// Create an image from a function
    ///
    /// The image width , height and colorspace need to be specified
    ///
    /// The function will receive two parameters, the first is the current x offset and y offset
    /// and for each it's expected to return  an array with `MAX_CHANNELS`
    ///
    /// # Arguments
    ///  - width : The width of the new image
    ///  - height: The height of the new image
    ///  - colorspace: The new colorspace of the image
    ///  - func: A function which will be called for every pixel position
    ///   the function is supposed to return pixels for that position
    ///      - y: The position in the y-axis, starts at 0, ends at image height
    ///      - x: The position in the x-axis, starts at 0, ends at image width
    ///      - pixels: A mutable region where you can write pixels to. The results
    ///         will be copied to the pixel positions at pixel x,y for image channels
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
    /// fn linear_gradient(y:usize,x:usize,pixels:&mut [u8;MAX_CHANNELS])
    /// {    
    ///     // this will create a linear band of colors from black to white and repeats
    ///     // until the whole image is visited
    ///     let luma = ((x+y) % 256) as u8;
    ///     pixels[0] = luma;
    ///      
    /// }
    /// let img  = Image::from_fn(30,20,ColorSpace::Luma,linear_gradient);
    /// ```
    /// We only set one element in our array but need to return an array with
    /// [MAX_CHANNELS] elements
    ///
    /// [MAX_CHANNELS]:MAX_CHANNELS
    pub fn from_fn<T, F>(width: usize, height: usize, colorspace: ColorSpace, func: F) -> Image
    where
        F: Fn(usize, usize, &mut [T; MAX_CHANNELS]),
        T: ZuneInts<T> + Copy + Clone + 'static + Default + Debug + Zeroable + Pod
    {
        match colorspace.num_components() {
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
        F: Fn(usize, usize, &mut [T; MAX_CHANNELS]),
        T: ZuneInts<T> + Copy + Clone + 'static + Default + Debug + Zeroable + Pod
    {
        let size = width * height * T::depth().size_of();

        let mut channels = vec![Channel::new_with_length::<T>(size); COMPONENTS];

        // convert the channels into mutable T's
        //
        // Iterate to number of components,
        // map the channels to &mut [T], using reintepret_as_mut
        // collect the items into a temporary vec
        // convert that vec to a fixed size array
        // panic if everything goes wrong
        let channels_ref: [&mut [T]; COMPONENTS] = channels
            .get_mut(0..COMPONENTS)
            .unwrap()
            .iter_mut()
            .map(|x| x.reinterpret_as_mut().unwrap())
            .collect::<Vec<&mut [T]>>()
            .try_into()
            .unwrap();

        let mut pxs = [T::default(); MAX_CHANNELS];

        for y in 0..height {
            for x in 0..width {
                (func)(y, x, &mut pxs);

                let offset = y * height + x;

                for i in 0..COMPONENTS {
                    channels_ref[i][offset] = pxs[i];
                }
            }
        }

        Image::new(channels, T::depth(), width, height, colorspace)
    }
}

/// Pixel constructors
impl Image {
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
    pub fn from_u8(pixels: &[u8], width: usize, height: usize, colorspace: ColorSpace) -> Image {
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
    ///The bit depth will be treated as [BitDepth::Sixteen](zune_core::bit_depth::BitDepth::Sixteen)
    ///
    /// # Returns
    /// An [`Image`](crate::image::Image) struct
    ///
    ///
    /// # Panics
    /// - If calculating image dimensions will overflow [`usize`]
    ///
    ///
    /// - If pixels length is not equal to expected length
    pub fn from_u16(pixels: &[u16], width: usize, height: usize, colorspace: ColorSpace) -> Image {
        let expected_len = checked_mul(width, height, 1, colorspace.num_components());

        assert_eq!(
            pixels.len(),
            expected_len,
            "Length mismatch, expected {expected_len} but found {} ",
            pixels.len()
        );

        let pixels = deinterleave_u16(pixels, colorspace).unwrap();

        Image::new(pixels, BitDepth::Sixteen, width, height, colorspace)
    }

    /// Create an image from raw f32 pixels
    ///
    /// Pixels are expected to be interleaved according to number of components in the colorspace
    ///
    /// e.g if image is RGBA, pixels should be in the form of `[R,G,B,A,R,G,B,A]`
    ///
    ///The bit depth will be treated as [BitDepth::Float32](zune_core::bit_depth::BitDepth::Float32)
    ///
    /// # Returns
    /// An [`Image`](crate::image::Image) struct
    ///
    ///
    /// # Panics
    /// - If calculating image dimensions will overflow [`usize`]
    ///
    /// - If pixels length is not equal to expected length
    pub fn from_f32(pixels: &[f32], width: usize, height: usize, colorspace: ColorSpace) -> Image {
        let expected_len = checked_mul(width, height, 1, colorspace.num_components());
        assert_eq!(
            pixels.len(),
            expected_len,
            "Length mismatch, expected {expected_len} but found {} ",
            pixels.len()
        );

        let pixels = deinterleave_f32(pixels, colorspace).unwrap();

        Image::new(pixels, BitDepth::Float32, width, height, colorspace)
    }
    pub fn frames_len(&self) -> usize {
        self.frames.len()
    }
}

/// Pixel manipulation methods
impl Image {
    /// Modify pixels in place using function `func`
    ///
    /// This iterates through all frames in the channel and calls
    /// a function on each mutable pixel
    ///
    /// # Arguments
    /// - func: Function which will modify the pixels
    ///     The arguments used are
    ///     - `y: usize`, the current position of the height we are currently in
    ///     - `x: usize`, the current position on the x axis we are in
    ///     - `[&mut T;MAX_CHANNELS]`, the pixels at `[y,x]` from the channels which
    ///        can be modified.
    ///        Even though it returns `MAX_CHANNELS`, only the image colorspace components
    ///        considered, so for Luma colorspace, we only use the first element in the array and the rest are
    ///         ignored
    ///
    /// # Returns
    ///  - Ok(()): Successful manipulation of image
    ///  - Err(ChannelErrors):  The channel could not be converted to type `T`
    ///
    /// # Example
    /// Modify pixels creating a gradient
    ///
    /// ```
    /// use zune_core::colorspace::ColorSpace;
    /// use zune_image::image::Image;
    /// // fill image with black pixel
    /// let mut image = Image::fill(0_u8,ColorSpace::RGB,800,800);
    ///
    /// // then modify the pixels
    /// // create a gradient
    /// image.modify_pixels_mut(|x,y,pix|     
    /// {
    ///     let r = (0.3 * x as f32) as u8;
    ///     let b = (0.3 * y as f32) as u8;
    ///     // modify channels directly
    ///     *pix[0] = r;
    ///     *pix[2] = b;
    /// }).unwrap();
    ///
    /// ```
    pub fn modify_pixels_mut<T, F>(&mut self, func: F) -> Result<(), ChannelErrors>
    where
        T: ZuneInts<T> + Default + Copy + 'static + Pod,
        F: Fn(usize, usize, [&mut T; MAX_CHANNELS])
    {
        let colorspace = self.colorspace();

        let (width, height) = self.dimensions();

        for frame in self.frames.iter_mut() {
            let mut pixel_muts: Vec<&mut [T]> = vec![];

            // convert all channels to type T
            for channel in frame.channels_mut(colorspace, false) {
                pixel_muts.push(channel.reinterpret_as_mut()?)
            }
            for y in 0..height {
                for x in 0..width {
                    let position = y * height + x;

                    // This must be kept in sync with
                    // MAX_CHANNELS, we can't do it another way
                    // since they are references
                    let mut output: [&mut T; MAX_CHANNELS] = [
                        &mut T::default(),
                        &mut T::default(),
                        &mut T::default(),
                        &mut T::default()
                    ];
                    // push pixels from channel to temporary output
                    for (i, j) in (pixel_muts.iter_mut()).zip(output.iter_mut()) {
                        *j = &mut i[position]
                    }

                    (func)(y, x, output);
                }
            }
        }
        Ok(())
    }
}

/// Image conversion routines
impl Image {
    /// Convert an image from one colorspace to another
    ///
    ///  # Arguments
    /// - to: The colorspace to convert image into
    ///
    pub fn convert_color(&mut self, to: ColorSpace) -> Result<(), ImageErrors> {
        ColorspaceConv::new(to).execute(self)
    }
    /// Convert an image from one depth to another
    ///
    /// # Arguments
    /// - to: The bit-depth to convert the image into
    pub fn convert_depth(&mut self, to: BitDepth) -> Result<(), ImageErrors> {
        Depth::new(to).execute(self)
    }
}

pub(crate) fn checked_mul(
    width: usize, height: usize, depth: usize, colorspace_components: usize
) -> usize {
    width
        .checked_mul(height)
        .unwrap()
        .checked_mul(depth)
        .unwrap()
        .checked_mul(colorspace_components)
        .unwrap()
}
