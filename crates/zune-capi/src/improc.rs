use std::ffi::c_float;

use zune_image::core_filters::colorspace::ColorspaceConv;
use zune_image::core_filters::depth::Depth;
use zune_image::traits::OperationsTrait;
use zune_imageprocs::auto_orient::AutoOrient;
use zune_imageprocs::bilateral_filter::BilateralFilter;
use zune_imageprocs::blend::Blend;
use zune_imageprocs::brighten::Brighten;
use zune_imageprocs::contrast::Contrast;
use zune_imageprocs::crop::Crop;
use zune_imageprocs::exposure::Exposure;
use zune_imageprocs::flip::Flip;
use zune_imageprocs::flop::Flop;
use zune_imageprocs::gamma::Gamma;
use zune_imageprocs::gaussian_blur::GaussianBlur;
use zune_imageprocs::invert::Invert;
use zune_imageprocs::median::Median;
use zune_imageprocs::scharr::Scharr;
use zune_imageprocs::sobel::Sobel;
use zune_imageprocs::stretch_contrast::StretchContrast;
use zune_imageprocs::transpose::Transpose;

use crate::enums::{ZImageColorspace, ZImageDepth};
use crate::errno::{ZStatus, ZStatusType};
use crate::ZImage;

fn exec_imgproc<T>(image: *mut ZImage, filter: T, status: *mut ZStatus)
where
    T: OperationsTrait
{
    if status.is_null() {
        return;
    }
    if image.is_null() {
        unsafe {
            *status = ZStatus::new("Image is null", ZStatusType::ZilImageIsNull);
        };
        return;
    }
    let image = unsafe { &mut *image };

    if let Err(err) = filter.execute_impl(image) {
        unsafe {
            *status = ZStatus::new(err.to_string(), ZStatusType::ZilImageOperationError);
        }
    }
}

/// Adjust the contrast of an image in place
///
/// \param image: Non-null image
/// \param contrast: Amount to adjust contrast by
/// \param status: Reports whether image operation was successful, should not be null
///
/// if any of `image` or `status` is null, nothing happens
///
#[no_mangle]
pub extern "C" fn zil_imgproc_adjust_contrast(
    image: *mut ZImage, contrast: c_float, status: *mut ZStatus
) {
    let filter = Contrast::new(contrast);
    exec_imgproc(image, filter, status);
}

/// Auto orient image based on exif tag
///
/// \param image: Non null image struct
/// \param status: Non null status reference
///
/// This is a no op in case image doesn't have an exif orientation flag
#[no_mangle]
pub extern "C" fn zil_imgproc_auto_orient(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, AutoOrient, status)
}

/// Apply a bilateral filter to an image
///
///\param d: Diameter of each pixel neighborhood that is used during filtering. If it is non-positive, it is computed from sigma_space.
///
///\param sigma_color: Filter sigma in the color space.
///  A larger value of the parameter means that farther colors within the pixel neighborhood (see sigmaSpace)
///  will be mixed together, resulting in larger areas of semi-equal color.
///
///\param sigma_space: Filter sigma in the coordinate space.
///  A larger value of the parameter means that farther pixels will influence each other as
///   long as their colors are close enough (see sigma_color ).
///   When d>0, it specifies the neighborhood size regardless of sigma_space. Otherwise, d is proportional to sigma_space.
#[no_mangle]
pub extern "C" fn zil_imgproc_bilateral_filter(
    image: *mut ZImage, d: i32, sigma_color: f32, sigma_space: f32, status: *mut ZStatus
) {
    exec_imgproc(
        image,
        BilateralFilter::new(d, sigma_color, sigma_space),
        status
    );
}

/// \brief Blend two images together based an alpha value
/// which is used to determine the `opacity` of pixels during blending
///
///
/// The formula for blending is
///
/// \code
/// dest =(src_alpha) * src  + (1-src_alpha) * dest
/// \endcode
///
/// `src_alpha` is expected to be between 0.0 and 1.0
///
/// \param image1: Image to which another image will be overlaid
/// \param image2: Image which will be overlaid on image 1, must have same dimensions,depth and colorspace
/// \param src_alpha: Source alpha, between 0 and 1, 1-> copy src to dest, 0 leave as is
/// \param status Image operation status, query this to tell you if the operation succeded
#[no_mangle]
pub extern "C" fn zil_imgproc_blend(
    image1: *mut ZImage, image2: *const ZImage, src_alpha: f32, status: *mut ZStatus
) {
    if status.is_null() {
        return;
    }
    if image2.is_null() {
        unsafe {
            *status = ZStatus::new("Image2 is null", ZStatusType::ZilImageIsNull);
        }
        return;
    }
    let blend_src = unsafe { &*image2 };
    let filter = Blend::new(blend_src, src_alpha);
    exec_imgproc(image1, filter, status);
}

/// Adjust image exposure
///
/// Formula used is
///
/// \code
/// pix = clamp((pix - black) * exposure)
/// \endcode
///  
/// where `pix` is the current image pixel
///
/// \param image: Non null image
///
/// \param exposure: Amount to adjust by
///
/// \param black_point: Amount to subtract from each pixel before converting,
///
/// \param status: Image status
///
#[no_mangle]
pub extern "C" fn zil_imgproc_exposure(
    image: *mut ZImage, exposure: f32, black_point: f32, status: *mut ZStatus
) {
    let filter = Exposure::new(exposure, black_point);
    exec_imgproc(image, filter, status)
}

/// Change image bit depth of the image
///
/// On successful execution, image depth will be the specified one by the `to` parameter
///
/// /param image: Non-null image struct
/// /param to: Depth to convert this image into
/// /param status: Image operation status, after execution query this to determine if execution
/// was successful
#[no_mangle]
pub extern "C" fn zil_imgproc_change_depth(
    image: *mut ZImage, to: ZImageDepth, status: *mut ZStatus
) {
    let depth = Depth::new(to.to_depth());
    exec_imgproc(image, depth, status);
}

/// Change image colorspace to a different one
///
/// On successful execution, image colorspace will be the one specified by the `to` parameter
///
/// \param image: Non-null image struct
/// \param to: New colorspace for the image
/// \param status: Result of image operation, query this to see if operation was successful
#[no_mangle]
pub extern "C" fn zil_imgproc_convert_colorspace(
    image: *mut ZImage, to: ZImageColorspace, status: *mut ZStatus
) {
    let colorspace = ColorspaceConv::new(to.to_colorspace());
    exec_imgproc(image, colorspace, status)
}

/// Crop an image, creating a smaller image from a bigger image
///
///
/// Origin (0,0) is from top left
///
/// \param image: Image to be cropped
/// \param new_width: New width of expected image
/// \param new_height: New height of expected image
/// \param x: How far from x origin the new image should be
/// \param y: How far from the y origin the new image should be
///
/// \param status: Image operation reporter
///
#[no_mangle]
pub extern "C" fn zil_imgproc_crop(
    image: *mut ZImage, new_width: usize, new_height: usize, x: usize, y: usize,
    status: *mut ZStatus
) {
    let filter = Crop::new(new_width, new_height, x, y);
    exec_imgproc(image, filter, status)
}

/// Flip an image by reflecting pixels on its x-axis
///
/// \code
/// old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │j i h g f │
/// │f g h i j│   │e d c b a │
/// └─────────┘   └──────────┘
/// \endcode
///
/// \param image: Image to flip
/// \param status: Image execution reporter
#[no_mangle]
pub extern "C" fn zil_imgproc_flip(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Flip, status)
}

/// Flop an image by reflecting pixels on its y-axis
///
/// \code
/// old image     new image
/// ┌─────────┐   ┌──────────┐
/// │a b c d e│   │e d b c a │
/// │f g h i j│   │j i h g f │
/// └─────────┘   └──────────┘
///
/// \endcode
///
/// \param image: Image to flop
/// \param status: Image execution reporter
#[no_mangle]
pub extern "C" fn zil_imgproc_flop(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Flop, status)
}

/// Gamma adjust an image
///
/// Formula used is
///
/// \code
/// max_value = maximum byte value
/// gamma_value =  passed gamma value
/// pixel = pixel.powf(gamma_value)/max_value;
///
/// \endcode
///
/// \param image: Image to apply gamma correction to
/// \param gamma: Gamma value
/// \param status: Image operations reporter
///
#[no_mangle]
pub extern "C" fn zil_imgproc_gamma(image: *mut ZImage, gamma: f32, status: *mut ZStatus) {
    exec_imgproc(image, Gamma::new(gamma), status)
}

/// Invert image pixels
///
/// Formula
///
/// \code
/// max_value -> maximum value of an image depth
///
/// pixel = max_value-pixel
///
/// \endcode
///
#[no_mangle]
pub extern "C" fn zil_imgproc_invert(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Invert, status)
}

/// Brighten an image
///
/// This increases or reduces an image pixels by a specific value `value`
///
/// Formula
///
/// \code
/// pixel = pixel+value
/// \endcode
///
/// \param image: Mutable image, should not be null
/// \param value: Value to be added to image, should be between -1 and 1 where -1 is total darkness and 1 is total brightness
/// \param status: Image status recorder
#[no_mangle]
pub extern "C" fn zil_imgproc_brighten(image: *mut ZImage, value: f32, status: *mut ZStatus) {
    exec_imgproc(image, Brighten::new(value), status)
}

/// Perform a gaussian blur on an image
///
/// \param sigma: How much to blur by, a greater value leads to more pronounced blurs
///
#[no_mangle]
pub extern "C" fn zil_imgproc_gaussian_blur(image: *mut ZImage, sigma: f32, status: *mut ZStatus) {
    exec_imgproc(image, GaussianBlur::new(sigma), status)
}

/// Linearly stretch the contrast of an image  in place, sending lower
/// values to `lower` and higher values  to `higher`
///
/// \param lower: Lower value, for which any pixel less than this will be clamped
/// to this
///
/// \param higher: Higher value, for which any pixel greater than this will be clamped
/// to this
#[no_mangle]
pub extern "C" fn zil_imgproc_stretch_contrast(
    image: *mut ZImage, lower: f32, higher: f32, status: *mut ZStatus
) {
    exec_imgproc(image, StretchContrast::new(lower, higher), status)
}

/// Transpose an image
///
/// This mirrors the image along the image top left to bottom-right
/// diagonal
///
/// Done by swapping X and Y indices of the array representation
#[no_mangle]
pub extern "C" fn zil_imgproc_transpose(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Transpose::new(), status)
}

/// Carry out a sobel operator
///
/// This operation calculates the gradient of the image,
/// which represents how quickly pixel values change from
/// one point to another in both the horizontal and vertical directions.
/// The magnitude and direction of the gradient can be used to detect edges in an image.
///
/// The matrix for sobel is
///
/// Gx matrix
/// \code
///   -1, 0, 1,
///   -2, 0, 2,
///   -1, 0, 1
/// \endcode
/// Gy matrix
/// \code
/// -1,-2,-1,
///  0, 0, 0,
///  1, 2, 1
/// \endcode
///
/// The window is a 3x3 window.
#[no_mangle]
pub extern "C" fn zil_imgproc_sobel(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Sobel::new(), status)
}

/// Carry out scharr operations
/// The matrix for scharr is
///
/// Gx matrix
/// \code
///   -3, 0,  3,
///  -10, 0, 10,
///   -3, 0,  3
/// \endcode
/// Gy matrix
/// \code
/// -3,-10,-3,
///  0,  0, 0,
///  3, 10, 3
/// \endcode
///
/// The window is a 3x3 window.

#[no_mangle]
pub extern "C" fn zil_imgproc_scharr(image: *mut ZImage, status: *mut ZStatus) {
    exec_imgproc(image, Scharr::new(), status)
}

/// Carry out a median blur operation on the image
///
/// Applies a median filter of given dimensions to an image. Each output pixel is the median
/// of the pixels in a `(2 * radius + 1) * (2 * radius + 1)` kernel of pixels in the input image.
///
///
/// \param radius: The radius of the window
#[no_mangle]
pub extern "C" fn zil_imgproc_median_blur(image: *mut ZImage, radius: usize, status: *mut ZStatus) {
    exec_imgproc(image, Median::new(radius), status)
}
