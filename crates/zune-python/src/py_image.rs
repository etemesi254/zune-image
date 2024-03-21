/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
mod numpy_bindings;

use std::any::TypeId;
use std::fs::read;

use numpy::{dtype, Element, PyArray2, PyArray3, PyUntypedArray};
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use zune_core::bit_depth::BitType;
use zune_core::bytestream::ZCursor;
use zune_core::colorspace::ColorSpace as ZColorSpace;
use zune_core::log::warn;
use zune_core::options::DecoderOptions;
use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::core_filters::depth::Depth;
use zune_image::image::Image as ZImage;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::auto_orient::AutoOrient;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::blend::Blend;
use zune_imageprocs::box_blur::BoxBlur;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::Flip;
use zune_imageprocs::flop::Flop;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::histogram::ChannelHistogram;
use zune_imageprocs::hsv_adjust::HsvAdjust;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::median::Median;
use zune_imageprocs::scharr::Scharr;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::threshold::Threshold;
use zune_imageprocs::transpose::Transpose;

use crate::py_enums::{ColorSpace, ImageDepth, ImageFormat, ImageThresholdType, ZImageErrors};

/// Execute a single filter on an image
///
/// This executes anything that implements OperationsTrait, returning an error if the
/// operation returned an error or okay if operation was successful

#[allow(clippy::needless_pass_by_value)]
fn exec_filter<T: OperationsTrait>(
    img: &mut Image, filter: T, in_place: bool
) -> PyResult<Option<Image>> {
    let exec = |image: &mut Image| -> PyResult<()> {
        if let Err(e) = filter.execute(&mut image.image) {
            return Err(PyErr::new::<PyException, _>(format!(
                "Error converting: {e:?}"
            )));
        }
        Ok(())
    };
    if in_place {
        exec(img)?;
        Ok(None)
    } else {
        let mut im_clone = img.clone();
        exec(&mut im_clone)?;

        Ok(Some(im_clone))
    }
}

/// The main image class.
///
///
/// # Instantiating
/// One can create a new image class by using one of the static methods to construct one
/// either by using `from_` methods (from_numpy,from_bytes) or using `open` routines to
/// decode a file in disk.
///
/// # Methods.
/// To carry out operations/filters on the image, one can use the methods attached to the class.
/// e.g to get the width after decoding an image, one can use `image.width()` function to achieve that.
///
/// All operations have an `in_place` argument which is usually set to `False`.
/// `in_place` modifies as to whether operations are running in the current copy or if the library
/// should create a copy modify the copy and return it preserving the current copy.
///
/// # Animated images.
/// The library supports animated images from the following formats
/// - png: Animated PNG: The images will be decoded and any blending done,
/// - jpeg-xl: Animated JXL: Decoding is offloaded to the jxl crate, images are rendered to be individual frames
#[pyclass]
#[derive(Clone)]
pub struct Image {
    image: ZImage
}

impl Image {
    pub(crate) fn new(image: ZImage) -> Image {
        Image { image }
    }
}

#[pymethods]
impl Image {
    /// Applies a bilateral filter to an image.
    ///
    /// it applies the bilateral filtering as described in [here](https://homepages.inf.ed.ac.uk/rbf/CVonline/LOCAL_COPIES/MANDUCHI1/Bilateral_Filtering.html)
    ///
    /// The filter can reduce unwanted noise while keeping edges fairly sharp.
    ///
    /// Sigma values: For simplicity, you can set the 2 sigma values to be the same. If they are small (< 10), the filter will not have much effect, whereas if they are large (> 150), they will have a very strong effect, making the image look "cartoonish".
    ///
    /// # Arguments
    /// - d	Diameter of each pixel neighborhood that is used during filtering. If it is non-positive, it is computed from sigma_space.
    ///
    /// - sigma_color	Filter sigma in the color space.
    ///  A larger value of the parameter means that farther colors within the pixel neighborhood (see sigmaSpace)
    ///  will be mixed together, resulting in larger areas of semi-equal color.
    ///-  sigma_space	Filter sigma in the coordinate space.
    ///  A larger value of the parameter means that farther pixels will influence each other as
    ///   long as their colors are close enough (see sigma_color ).
    ///   When d>0, it specifies the neighborhood size regardless of sigma_space. Otherwise, d is proportional to sigma_space.
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (d, sigma_color, sigma_space, in_place = false))]
    pub fn bilateral(
        &mut self, d: i32, sigma_color: f32, sigma_space: f32, in_place: bool
    ) -> PyResult<Option<Image>> {
        let filter = BilateralFilter::new(d, sigma_color, sigma_space);
        exec_filter(self, filter, in_place)
    }
    /// Get the image dimensions as a tuple of width and height
    ///
    /// # Returns
    /// -  A tuple in the format `(width,height)`
    pub fn dimensions(&self) -> (usize, usize) {
        self.image.dimensions()
    }
    /// Get the image width
    ///
    /// # Returns
    /// Image width
    pub fn width(&self) -> usize {
        self.image.dimensions().0
    }
    /// Get the image height
    ///
    /// # Returns
    /// Image height
    pub fn height(&self) -> usize {
        self.image.dimensions().1
    }
    /// Get the image colorspace
    ///
    /// # Returns
    /// - The current image colorspace
    ///
    /// # See
    /// - [convert_colorspace](Image::convert_colorspace) : Convert from one colorspace to another
    pub fn colorspace(&self) -> ColorSpace {
        ColorSpace::from(self.image.colorspace())
    }
    /// Convert from one colorspace to another
    ///
    /// # Arguments
    /// - to: The new colorspace to convert to
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (to, in_place = false))]
    pub fn convert_colorspace(
        &mut self, to: ColorSpace, in_place: bool
    ) -> PyResult<Option<Image>> {
        let color = to.to_colorspace();
        exec_filter(self, ColorspaceConv::new(color), in_place)
    }
    /// Return the image depth
    ///
    /// # Returns
    /// - The image depth
    ///
    /// This also gives you the internal representation of an image
    ///  - Eight: u8 (1 byte per pixel)
    ///  - Sixteen: u16 (2 bytes per pixel)
    ///  - F32: Float f32 (4 bytes per pixel, float type)
    ///  
    pub fn depth(&self) -> ImageDepth {
        ImageDepth::from(self.image.depth())
    }
    /// Save an image to a format
    ///
    /// Not all image formats have encoders enabled
    /// so check by calling `PyImageFormat.has_encoder()` which returns a boolean
    /// indicating if the image format has an encoder
    ///
    /// # Arguments
    ///  - file: Filename to save the file to
    ///  - format:  The format to save the file in
    ///
    /// # Returns
    ///  - Nothing on success, or Exception  on error
    pub fn save(&self, file: String, format: ImageFormat) -> PyResult<()> {
        if let Err(e) = self.image.save_to(file, format.to_imageformat()) {
            return Err(PyErr::new::<PyException, _>(format!(
                "Error encoding: {e:?}"
            )));
        }
        Ok(())
    }

    /// Crop an image
    ///
    /// # Arguments
    /// - width: Out width, how wide the new image should be
    /// - height: Out height, how tall the new image should be
    /// - x : How many pixels horizontally from the origin should the cropping start from
    /// - y : How many pixels vertically from the origin should the cropping start from.
    ///
    ///  - in_place: Whether to carry out the crop in place or create a clone for which to crop
    ///
    /// Origin is defined from the top left of the image.
    ///
    #[pyo3(signature = (width, height, x, y, in_place = false))]
    pub fn crop(
        &mut self, width: usize, height: usize, x: usize, y: usize, in_place: bool
    ) -> PyResult<Option<Image>> {
        exec_filter(self, Crop::new(width, height, x, y), in_place)
    }
    /// Transpose the image.
    ///
    /// This rewrites pixels into `dst(i,j)=src(j,i)`
    ///
    /// # Arguments
    /// - inplace: Whether to transpose the image in place or generate a clone
    /// and transpose the new clone
    #[pyo3(signature = (in_place = false))]
    pub fn transpose(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Transpose, in_place)
    }

    /// Convert from one depth to another
    ///
    /// The following are the depth conversion details
    ///  
    /// - INT->Float : Convert to float and divide by max value for the previous integer type(255 for u8,65535 for u16).
    /// - Float->Int : Multiply by max value of the new depth (255->Eight,65535->16)
    /// - smallInt->Int :  Multiply by (MAX_LARGE_INT/MAX_SMALL_INT)
    /// - LargeInt->SmallInt: Divide by (MAX_LARGE_INT/MAX_SMALL_INT)  
    ///
    /// # Arguments
    /// - to: The new depth to convert to
    /// - in_place: Whether to perform the conversion in place or to create a copy and convert that
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (to, in_place = false))]
    pub fn convert_depth(&mut self, to: ImageDepth, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Depth::new(to.to_depth()), in_place)
    }
    /// Applies a fixed-level threshold to each array element.
    ///
    /// Thresholding works best for grayscale images, passing a colored image
    /// does not implicitly convert it to grayscale, you need to do that explicitly
    ///
    /// # Arguments
    ///  - value: Non-zero value assigned to the pixels for which the condition is satisfied
    ///  - method: The thresholding method used, defaults to binary which generates a black
    /// and white image from a grayscale image
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (value, method = ImageThresholdType::Binary, in_place = false))]
    pub fn threshold(
        &mut self, value: f32, method: ImageThresholdType, in_place: bool
    ) -> PyResult<Option<Image>> {
        exec_filter(self, Threshold::new(value, method.to_threshold()), in_place)
    }
    /// Invert (negate) an image
    ///
    /// # Arguments
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn invert(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Invert, in_place)
    }

    /// Blur the image using a box blur operation
    ///
    /// # Arguments
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (radius, in_place = false))]
    pub fn box_blur(&mut self, radius: usize, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, BoxBlur::new(radius), in_place)
    }

    /// Adjust exposure of image filter
    ///
    ///#  Arguments
    /// - exposure: Set the exposure correction, allowed range is from -3.0 to 3.0. Default should be zero
    /// - black: Set black level correction: Allowed range from -1.0 to 1.0. Default is zero.
    ///
    /// For 8 bit and 16 bit images, values are clamped to their limits,
    /// for floating point, no clamping occurs
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (exposure, black_point = 0.0, in_place = false))]
    pub fn exposure(
        &mut self, exposure: f32, black_point: f32, in_place: bool
    ) -> PyResult<Option<Image>> {
        exec_filter(self, Exposure::new(exposure, black_point), in_place)
    }

    /// Creates a vertical mirror image by reflecting
    /// the pixels around the central x-axis.
    ///
    ///
    /// ```text
    ///
    ///old image     new image
    /// ┌─────────┐   ┌──────────┐
    /// │a b c d e│   │j i h g f │
    /// │f g h i j│   │e d c b a │
    /// └─────────┘   └──────────┘
    /// ```
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn flip(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Flip, in_place)
    }

    /// Creates a horizontal mirror image by
    /// reflecting the pixels around the central y-axis
    ///
    ///```text
    ///old image     new image
    ///┌─────────┐   ┌──────────┐
    ///│a b c d e│   │e d b c a │
    ///│f g h i j│   │j i h g f │
    ///└─────────┘   └──────────┘
    ///```
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (in_place = false))]
    pub fn flop(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Flop, in_place)
    }
    /// Gamma adjust an image
    ///
    /// This currently only supports 8 and 16 bit depth images since it applies an optimization
    /// that works for those depths.
    ///
    /// # Arguments
    /// - gamma: Ranges typical range is from 0.8-2.3
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (gamma, in_place = false))]
    pub fn gamma(&mut self, gamma: f32, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Gamma::new(gamma), in_place)
    }

    /// Blur the image using a gaussian blur filter
    ///
    /// # Arguments
    ///   - sigma: Strength of blur
    ///  - in_place: Whether to perform the operation in-place or to clone and return a copy
    ///
    /// # Returns
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (sigma, in_place = false))]
    pub fn gaussian_blur(&mut self, sigma: f32, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, GaussianBlur::new(sigma), in_place)
    }

    /// Auto orient the image based on the exif metadata
    ///
    ///
    /// This operation is also a no-op if the image does not have
    /// exif metadata
    #[pyo3(signature = (in_place = false))]
    pub fn auto_orient(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, AutoOrient, in_place)
    }

    /// Calculate the sobel derivative of an image
    ///
    /// This uses the standard 3x3 [Gx and Gy matrix](https://en.wikipedia.org/wiki/Sobel_operator)
    ///
    /// Gx matrix
    /// ```text
    ///   -1, 0, 1,
    ///   -2, 0, 2,
    ///   -1, 0, 1
    /// ```
    /// Gy matrix
    /// ```text
    /// -1,-2,-1,
    ///  0, 0, 0,
    ///  1, 2, 1
    /// ```
    ///
    ///  # Arguments
    /// - in-place: Whether to carry the operation in place or clone and operate on the copy
    #[pyo3(signature = (in_place = false))]
    pub fn sobel(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Sobel, in_place)
    }
    /// Calculate the scharr derivative of an image
    ///
    /// The image is convolved with the following 3x3 matrix
    ///
    ///
    /// Gx matrix
    /// ```text
    ///   -3, 0,  3,
    ///  -10, 0, 10,
    ///   -3, 0,  3
    /// ```
    /// Gy matrix
    /// ```text
    /// -3,-10,-3,
    ///  0,  0, 0,
    ///  3, 10, 3
    /// ```
    ///
    ///  # Arguments
    /// - in-place: Whether to carry the operation in place or clone and operate on the copy
    #[pyo3(signature = (in_place = false))]
    pub fn scharr(&mut self, in_place: bool) -> PyResult<Option<Image>> {
        exec_filter(self, Scharr, in_place)
    }

    /// Linearly stretches the contrast in an image in place,
    /// sending lower to image minimum and upper to image maximum.
    ///
    /// Arguments:
    ///
    /// - lower: Lower minimum value for which pixels below this are clamped to the value
    /// - upper: Upper maximum value for which pixels above are clamped to the value
    ///
    ///
    /// Returns:
    ///
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (lower, upper, in_place = false))]
    pub fn stretch_contrast(
        &mut self, lower: f32, upper: f32, in_place: bool
    ) -> PyResult<Option<Image>> {
        let stretch_contrast = StretchContrast::new(lower, upper);

        exec_filter(self, stretch_contrast, in_place)
    }
    /// Convert the image bytes to a numpy array
    ///
    /// For grayscale data the array is a 2-D numpy array of
    ///  `[width,height]`
    ///
    /// For other image types, the array is a
    ///  3-D numpy array of
    /// `[width,height,colorspace_components]` dimensions/
    /// This means that e.g for a 256x256 rgb image numpy shape will return `(256,256,3)`
    ///
    /// Colorspace is important in determining output.
    ///
    /// RGB colorspace is arranged as `R`,`G`,`B` , BGR is arranged as `B`,`G`,`R`
    ///
    ///
    /// # Array type:
    ///
    /// The array type is determined by the  image depths/ image bit-type
    ///
    /// The following mappings are considered.
    ///
    /// - ZImageDepth::Eight -> dtype=uint8
    /// - ZImageDepth::Sixteen -> dtype=uint16
    /// - ZimageDepth::F32  -> dtype=float32
    ///
    /// ## Dtype
    ///  
    ///
    /// # Returns:
    ///
    ///  A numpy representation of the image if okay.
    ///
    /// An error in case something went wrong
    pub fn to_numpy<'py>(&self, py: Python<'py>) -> PyResult<&'py PyUntypedArray> {
        match self.image.depth().bit_type() {
            BitType::U8 => Ok(self.to_numpy_generic::<u8>(py, ImageDepth::U8)?),
            BitType::U16 => Ok(self.to_numpy_generic::<u16>(py, ImageDepth::U16)?),
            BitType::F32 => Ok(self.to_numpy_generic::<f32>(py, ImageDepth::F32)?),
            d => Err(PyErr::new::<PyException, _>(format!(
                "Error converting to depth {d:?}"
            )))
        }
    }
    /// Open an image from a file path
    ///
    ///
    /// - Arguments
    ///
    /// file: A string pointing to the file path
    #[staticmethod]
    fn open(file: String) -> PyResult<Image> {
        decode_file(file)
    }
    #[staticmethod]
    fn from_bytes(bytes: &[u8]) -> PyResult<Image> {
        decode_image(bytes)
    }

    /// Convert a numpy array into an image.
    ///
    /// The elements in the numpy array are treated as pixels
    ///
    ///
    /// The numpy array can be a 2 dimensional array for which the image will be treated as grayscale/luma
    /// or a three dimensional array for which the image colorspace is determined by the dimensions of the third axis
    ///
    ///
    /// The array is expected to be contiguous and the array should not be mutably borrowed from the size
    #[staticmethod]
    fn from_numpy(array: &PyUntypedArray, colorspace: Option<ColorSpace>) -> PyResult<Image> {
        from_numpy(array, colorspace)
    }

    /// Blend two images together.
    ///
    /// The formula for blend is
    ///
    /// ```text
    /// dest =  (src_alpha * src_image) + (1 - src_alpha) * self.image
    /// ```
    ///
    /// Alpha channel is ignored
    ///
    ///
    /// # Arguments
    /// - image: The image we are blending with, this is considered as the source image(src_image) in the above formula.
    /// - src_alpha: The source alpha. A value of 1 will cause the source to completely fill the image, a value of
    ///  0 will cause the destination image to completely fill the image.
    ///
    ///
    /// Returns:
    ///
    ///  - If `in_place=True`: Nothing on success, on error returns error that occurred
    ///  - If `in_place=False`: An image copy on success on error, returns error that occurred
    #[pyo3(signature = (image,src_alpha, in_place = false))]
    pub fn blend(
        &mut self, image: &Image, src_alpha: f32, in_place: bool
    ) -> PyResult<Option<Image>> {
        let filter = Blend::new(&image.image, src_alpha);
        exec_filter(self, filter, in_place)
    }

    /// Calculate the image histogram
    ///
    // The return type is a vector of vectors, with the interpretation of each vector depending on the colorspace
    /// E.g if image is in RGBA, the vector would be of len 4, each the first innermost vector would give you
    /// `R` channel histogram details, the last giving you "A" histogram details
    pub fn histogram(&mut self) -> PyResult<Vec<Vec<u32>>> {
        let histogram = ChannelHistogram::new();
        histogram
            .execute_impl(&mut self.image)
            .map_err(|c| PyErr::new::<PyException, _>(format!("Erro {c}")))?;
        // references, dangling pointers, the usual...
        let x = histogram.histogram().unwrap().to_vec();
        Ok(x)
    }
    /// Adjust the hue saturation and lightness of an image
    ///
    /// - hue: The hue rotation argument. This is usually a value between 0 and 360 degrees
    ///
    /// -saturation: The saturation scaling factor, a value of 0 produces a grayscale image, 1 has no effect, other values lie withing that spectrum
    ///  > 1 produces vibrant cartoonish color
    ///
    /// - lightness: The lightness scaling factor, a value greater than 0, values less than or equal to zero
    /// produce a black image, higher values increase the brightness of the image, 1.0 doesn't do anything
    /// to image lightness

    #[pyo3(signature = (hue,saturation,lightness, in_place = false))]
    pub fn hsl_adjust(
        &mut self, hue: f32, saturation: f32, lightness: f32, in_place: bool
    ) -> PyResult<Option<Image>> {
        let filter = HsvAdjust::new(hue, saturation, lightness);
        exec_filter(self, filter, in_place)
    }

    /// Applies a median filter of given radius to an image. Each output pixel is the median
    /// of the pixels in a `(2 * radius + 1) * (2 * radius + 1)` kernel of pixels in the input image.
    ///
    /// - radius: Median filter radius
    #[pyo3(signature=(radius,in_place=false))]
    pub fn median_filter(&mut self, radius: usize, in_place: bool) -> PyResult<Option<Image>> {
        let filter = Median::new(radius);
        exec_filter(self, filter, in_place)
    }
}

#[allow(clippy::cast_possible_truncation)]
fn convert_2d<T: Element + 'static>(numpy: &PyArray2<T>) -> PyResult<ZImage> {
    let dims = numpy.shape();
    if TypeId::of::<T>() == TypeId::of::<u8>() {
        let downcasted: &PyArray2<u8> = numpy.downcast()?;
        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_u8(bytes, dims[1], dims[0], ZColorSpace::Luma));
    }
    if TypeId::of::<T>() == TypeId::of::<u16>() {
        let downcasted: &PyArray2<u16> = numpy.downcast()?;
        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_u16(bytes, dims[1], dims[0], ZColorSpace::Luma));
    }
    if TypeId::of::<T>() == TypeId::of::<f32>() {
        let downcasted: &PyArray2<f32> = numpy.downcast()?;

        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_f32(bytes, dims[1], dims[0], ZColorSpace::Luma));
    }
    if TypeId::of::<T>() == TypeId::of::<f64>() {
        warn!("The library doesn't natively support f64, the data will be converted to f32");
        let downcasted: &PyArray2<f64> = numpy.downcast()?;

        let dc = downcasted.try_readonly()?;
        let bytes = dc
            .as_slice()?
            .iter()
            .map(|x| *x as f32)
            .collect::<Vec<f32>>();
        return Ok(ZImage::from_f32(
            &bytes,
            dims[1],
            dims[0],
            ZColorSpace::Luma
        ));
    }
    Err(PyErr::new::<PyException, _>(format!(
        "The type {:?} is not supported supported types are u16,u8, and f32  (the types f64 is converted to f32)",
        numpy.dtype()
    )))
}

#[allow(clippy::cast_possible_truncation)]
pub fn convert_3d<T: Element + 'static>(
    numpy: &PyArray3<T>, suggested_colorspace: Option<ColorSpace>
) -> PyResult<ZImage> {
    let dims = numpy.shape();
    let mut expected_colorspace: ZColorSpace = match dims[2] {
        1 => ZColorSpace::Luma,
        2 => ZColorSpace::LumaA,
        3 => ZColorSpace::RGB,
        4 => ZColorSpace::RGBA,
        _ => {
            return Err(PyErr::new::<PyException, _>(format!(
                "The dimension {:?} is not supported",
                numpy.dtype()
            )));
        }
    };
    if let Some(x) = suggested_colorspace {
        let c: ZColorSpace = x.to_colorspace();
        if c.num_components() != dims[2] {
            return Err(PyErr::new::<PyException, _>(format!(
                "The specified colorspace {:?} does not match the elements in the third dimension, expected a shape of ({},{},{}) for {:?} but found ({},{},{})", c,
                dims[1], dims[0], c.num_components(), c, dims[0], dims[1], dims[2]
            )));
        }
        expected_colorspace = c;
    }

    if TypeId::of::<T>() == TypeId::of::<u8>() {
        let downcasted: &PyArray3<u8> = numpy.downcast()?;
        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_u8(
            bytes,
            dims[1],
            dims[0],
            expected_colorspace
        ));
    }
    if TypeId::of::<T>() == TypeId::of::<u16>() {
        let downcasted: &PyArray3<u16> = numpy.downcast()?;
        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_u16(
            bytes,
            dims[1],
            dims[0],
            expected_colorspace
        ));
    }
    if TypeId::of::<T>() == TypeId::of::<f32>() {
        let downcasted: &PyArray3<f32> = numpy.downcast()?;

        let dc = downcasted.try_readonly()?;
        let bytes = dc.as_slice()?;
        return Ok(ZImage::from_f32(
            bytes,
            dims[1],
            dims[0],
            expected_colorspace
        ));
    }
    if TypeId::of::<T>() == TypeId::of::<f64>() {
        warn!("The library doesn't natively support f64, the data will be converted to f32");
        let downcasted: &PyArray3<f64> = numpy.downcast()?;

        let dc = downcasted.try_readonly()?;
        let bytes = dc
            .as_slice()?
            .iter()
            .map(|x| *x as f32)
            .collect::<Vec<f32>>();
        return Ok(ZImage::from_f32(
            &bytes,
            dims[1],
            dims[0],
            expected_colorspace
        ));
    }

    Err(PyErr::new::<PyException, _>(format!(
        "The type {:?} is not supported supported types are u16,u8, and f32  (the types f64 is converted to f32)",
        numpy.dtype()
    )))
}

/// Convert a numpy array into an image.
///
/// The elements in the numpy array are treated as pixels
///
///
/// The numpy array can be a 2 dimensional array for which the image will be treated as grayscale/luma
/// or a three dimensional array for which the image colorspace is determined by the dimensions of the third axis
///
/// The array is expected to be contiguous and the array should not be mutably borrowed from the size
///  
/// Floating pont data is expected to be in the range [0.0-1.0]
///
/// # Supported types
/// - `float32`,`uint8`,`uint16` - Data is ingested as is
///  - float64` - Image is converted into f32 type
/// - `uint32`  - Image is converted into u16 type using a saturating cast
pub fn from_numpy(array: &PyUntypedArray, colorspace: Option<ColorSpace>) -> PyResult<Image> {
    return Python::with_gil::<_, PyResult<Image>>(|py| {
        let d_type = array.dtype();
        let dims = array.ndim();
        if dims == 2 {
            if d_type.is_equiv_to(dtype::<u8>(py)) {
                let c: &PyArray2<u8> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_2d(c)?
                });
            }
            if d_type.is_equiv_to(dtype::<u16>(py)) {
                let c: &PyArray2<u16> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_2d(c)?
                });
            }
            if d_type.is_equiv_to(dtype::<f32>(py)) {
                let c: &PyArray2<f32> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_2d(c)?
                });
            }
            if d_type.is_equiv_to(dtype::<f64>(py)) {
                let c: &PyArray2<f64> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_2d(c)?
                });
            }
            if d_type.is_equiv_to(dtype::<u32>(py)) {
                let c: &PyArray2<u32> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_2d(c)?
                });
            }
        }

        if dims == 3 {
            if d_type.is_equiv_to(dtype::<u8>(py)) {
                let c: &PyArray3<u8> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_3d(c, colorspace)?
                });
            }
            if d_type.is_equiv_to(dtype::<u16>(py)) {
                let c: &PyArray3<u16> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_3d(c, colorspace)?
                });
            }
            if d_type.is_equiv_to(dtype::<f32>(py)) {
                let c: &PyArray3<f32> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_3d(c, colorspace)?
                });
            }
            if d_type.is_equiv_to(dtype::<f64>(py)) {
                let c: &PyArray3<f64> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_3d(c, colorspace)?
                });
            }
            if d_type.is_equiv_to(dtype::<u32>(py)) {
                let c: &PyArray3<u32> = array.downcast()?;
                // single dimension
                return Ok(Image {
                    image: convert_3d(c, colorspace)?
                });
            }
        }
        Err(PyErr::new::<PyException, _>(format!(
            "Unsupported dimension/dtype  dtype=>{d_type},dimensions=>{dims} consult documentation for supported types"
        )))
    });
}

pub fn decode_image(bytes: &[u8]) -> PyResult<Image> {
    let im_result = ZImage::read(ZCursor::new(bytes), DecoderOptions::new_fast());
    match im_result {
        Ok(result) => Ok(Image::new(result)),
        Err(err) => Err(PyErr::new::<PyException, _>(format!(
            "Error decoding: {err:?}"
        )))
    }
}

impl From<ZImageErrors> for pyo3::PyErr {
    fn from(value: ZImageErrors) -> Self {
        PyErr::new::<PyException, _>(format!("{:?}", value.error))
    }
}

/// Decode a file path containing an image
pub fn decode_file(file: String) -> PyResult<Image> {
    match read(file) {
        Ok(bytes) => Ok(Image::new(
            ZImage::read(ZCursor::new(bytes), DecoderOptions::new_fast())
                .map_err(ZImageErrors::from)?
        )),
        Err(e) => Err(PyErr::new::<PyException, _>(format!("{e}")))
    }
}
