use crate::bilateral_filter::BilateralFilter;
use crate::brighten::Brighten;
use crate::contrast::Contrast;
use crate::flip::FlipDirection;
use crate::transfer_curve::{ConversionType, TransferCurve, TransferFunction};
use crate::mirror::MirrorMode;
use crate::resize::{ResizeDimensions, ResizeMethod};
use crate::threshold::ThresholdMethod;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::metadata::AlphaState;
use zune_image::traits::OperationsTrait;

fn apply_op<O: OperationsTrait>(mut img: Image, op: O) -> Result<Image, ImageErrors> {
    op.execute(&mut img)?;
    Ok(img)
}

/// Extension trait providing ergonomic, chainable image filters.
///
/// This trait is only implemented for single-image operations that:
/// - consume and return a single `Image`
/// - do not operate on image stacks (`Vec<Image>`)
/// - internally delegate to `OperationsTrait` implementations
///
/// Multi-image operations (e.g. stitching, compositing) are intentionally
/// excluded from this API and should be executed via `OperationsTrait::execute_multiple`.
///
/// # Design intent
///
/// This trait exists purely for ergonomics. It provides a fluent, pipeline-style
/// API over the underlying operation system.
///
/// All methods:
/// - consume `self` (to enable chaining without clones)
/// - return `Result<Self, ImageErrors>` for safe propagation
///
/// # Example
/// ```rust
/// use zune_core::colorspace::ColorSpace;
/// use zune_image::image::Image;
/// use zune_imageprocs::FilterExt;
/// let img = Image::fill(128_u8,ColorSpace::RGB,100,100);
/// // now carry out multiple operations at once,
/// // this will
/// //  1. Multiply the image pixels by 0.2
/// //  2. Blur the image in a 5x5 window
/// let img = img
///     .exposure(0.2,0.0)?
///     .median_blur(2)?;
///
/// Ok::<(),zune_image::errors::ImageErrors>(())
/// ```
pub trait FilterExt: Sized {
    /// Automatically orients the image based on its EXIF metadata.
    ///
    /// This reads the EXIF `Orientation` tag (if present) and applies the necessary
    /// rotations or flips to ensure the image appears correctly oriented. After processing,
    /// the internal EXIF orientation tag is reset to `1` (Normal) to prevent double-rotation.
    ///
    /// **Note:** This requires the `exif` feature to be enabled in your `Cargo.toml`.
    /// Without it, this method acts as a silent no-op.
    ///
    /// # Arguments
    ///
    /// This method takes no additional arguments.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` that is correctly oriented, or an error if:
    /// - the image colorspace is unsupported by the underlying transform operations
    /// - the bit depth is unsupported
    /// - internal invariants fail during execution
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if any of the underlying rotation or flipping operations fail.
    #[cfg(feature = "exif")]
    fn auto_orient(self) -> Result<Self, ImageErrors>;
    /// Applies a bilateral filter to the image to reduce noise while preserving edges.
    ///
    /// # Arguments
    ///
    /// * `d` - Diameter of each pixel neighborhood used during filtering.
    ///   If non-positive, it is automatically computed from `sigma_space`.
    /// * `sigma_color` - Filter sigma in the color space. A larger value means farther colors
    ///   within the neighborhood will be mixed together, creating larger areas of semi-equal color.
    /// * `sigma_space` - Filter sigma in the coordinate space. A larger value means farther pixels
    ///   will influence each other, provided their colors are close enough.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the filter applied, or an error if:
    /// - the image bit depth is unsupported (currently only `U8` and `U16` are supported).
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn bilateral_filter(
        self, d: i32, sigma_color: f32, sigma_space: f32,
    ) -> Result<Self, ImageErrors>;
    /// Applies a box blur to the image.
    ///
    /// A box blur averages the pixels within a given radius to create a smoothing effect.
    /// This implementation is highly optimized ($O(1)$ per pixel relative to radius),
    /// making it extremely fast even for very large blur radii.
    ///
    /// # Arguments
    ///
    /// * `radius` - The radius of the blur.
    ///   - A radius of `0` or `1` does nothing.
    ///   - Larger values produce a more pronounced blur.
    ///   - Even values are automatically bumped to the next odd value to maintain symmetry.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the blur applied, or an error if:
    /// - the image bit depth is unsupported (currently supports `U8`, `U16`, `F32`).
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn box_blur(self, radius: usize) -> Result<Self, ImageErrors>;
    /// Adjusts the brightness of the image.
    ///
    /// # Arguments
    ///
    /// * `by` - Brightness adjustment factor
    ///   - `0.0` → no change
    ///   - `1.0` → maximum brightness increase
    ///   - `-1.0` → maximum darkness (clamped)
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with brightness adjusted, or an error if:
    /// - the image colorspace is unsupported
    /// - the bit depth is unsupported
    /// - internal invariants fail during execution
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn brighten(self, by: f32) -> Result<Self, ImageErrors>;
    /// Applies a 4x5 color matrix transformation to the image.
    ///
    /// This converts the image to RGBA, multiplies each pixel's color values by the
    /// provided matrix, and converts the image back to its original colorspace.
    /// Matrix offset values (the 5th column) should be normalized between `0.0` and `1.0`.
    ///
    /// # Arguments
    ///
    /// * `matrix` - A `4x5` array of `f32` representing the transformation matrix.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the color transformation applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal colorspace conversions fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn color_matrix(self, matrix: [[f32; 5]; 4]) -> Result<Self, ImageErrors>;
    /// Flips or rotates the image based on the provided direction.
    ///
    /// This operation performs geometric reflections or 180-degree rotations by safely
    /// reordering the underlying pixel data.
    ///
    /// # Arguments
    ///
    /// * `direction` - A `FlipDirection` specifying how the image should be transformed
    ///   (`Horizontal`, `Vertical`, or `Rotate180`).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the flip applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn flip(self, direction: FlipDirection) -> Result<Self, ImageErrors>;
    /// Transforms the image's colors to match a target ICC color profile.
    ///
    /// This converts the image from its current embedded color profile to the specified
    /// target profile (e.g., sRGB, Display P3). If the image lacks an embedded ICC
    /// profile, this method does nothing.
    ///
    /// **Note:** This requires the `cms` feature to be enabled in your `Cargo.toml`.
    ///
    /// # Arguments
    ///
    /// * `target_profile` - A `ColorProfiles` enum variant specifying the destination color space.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the transformed pixel data and updated embedded metadata,
    /// or an error if:
    /// - the image bit depth or colorspace is unsupported by the CMS engine.
    /// - the internal color transform fails to build or execute.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation or profile encoding fails.
    #[cfg(feature = "cms")]
    fn color_transform(
        self, target_profile: crate::color_transform::ColorProfiles,
    ) -> Result<Self, ImageErrors>;
    /// Applies a 2D convolution matrix to the image.
    ///
    /// Convolution recalculates each pixel based on a weighted matrix (kernel) of its
    /// neighbors. This is useful for custom blurs, sharpening, and edge detection.
    ///
    /// # Arguments
    ///
    /// * `weights` - A 1D `Vec<f32>` representing the 2D matrix. Its length **must** be
    ///   exactly `9` (3x3), `25` (5x5), or `49` (7x7).
    /// * `scale` - A multiplier applied to the final convolution sum. Usually set to
    ///   `1.0 / sum_of_weights` to preserve image brightness.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the convolution applied, or an error if:
    /// - the `weights` vector length is not 9, 25, or 49.
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn convolve(self, weights: Vec<f32>, scale: f32) -> Result<Self, ImageErrors>;
    /// Adjusts the contrast of the image.
    ///
    /// # Arguments
    ///
    /// * `contrast` - The contrast adjustment level.
    ///   - Values `> 0.0` increase contrast.
    ///   - Values `< 0.0` decrease contrast.
    ///   - `0.0` results in no change.
    ///   - The practical working range is typically `-255.0` to `255.0`.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with adjusted contrast, or an error if:
    /// - the image bit depth is unsupported (currently only supports 8-bit images).
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn contrast(self, contrast: f32) -> Result<Self, ImageErrors>;
    /// Crops the image to a specified rectangular region.
    ///
    /// # Arguments
    ///
    /// * `width` - The width of the newly cropped image.
    /// * `height` - The height of the newly cropped image.
    /// * `x` - The horizontal starting offset from the left edge of the original image.
    /// * `y` - The vertical starting offset from the top edge of the original image.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` containing only the cropped pixels, or an error if:
    /// - the requested crop window falls outside the bounds of the original image.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the crop boundaries are invalid or the underlying operation fails.
    fn crop(self, width: usize, height: usize, x: usize, y: usize) -> Result<Self, ImageErrors>;
    /// Adjusts the exposure and black level of the image.
    ///
    /// # Arguments
    ///
    /// * `exposure` - A linear multiplier applied to the pixel values.
    ///   - `1.0` means no change.
    ///   - `2.0` doubles the brightness.
    ///   - `0.5` halves the brightness.
    /// * `black` - The black level offset, normalized between `0.0` and `1.0`.
    ///   It is scaled automatically depending on the image's bit depth (e.g., multiplied
    ///   by 255 for 8-bit images).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with adjusted exposure, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn exposure(self, exposure: f32, black: f32) -> Result<Self, ImageErrors>;
    /// Applies a custom mathematical expression to every pixel in the image.
    ///
    /// This uses a JIT-compiled expression evaluator to process images procedurally.
    /// You can write equations using pixel coordinates (`i`, `j`), image dimensions (`w`, `h`),
    /// specific color channels (`r`, `g`, `b`), or global statistics (`img_mean`).
    ///
    /// **Note:** This requires the `exmex` feature to be enabled.
    ///
    /// # Arguments
    ///
    /// * `expression` - A string representing the mathematical equation. You can prefix
    ///   it with a channel target (e.g., `"r: val * 1.2"`) to isolate the effect.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the expression applied, or an error if:
    /// - the expression string is mathematically invalid or contains unknown variables.
    /// - the image colorspace is incompatible with the requested variables.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if parsing the expression fails.
    #[cfg(feature = "exmex")]
    fn fx<S: Into<String>>(self, expression: S) -> Result<Self, ImageErrors>;
    /// Applies gamma correction to the image.
    ///
    /// Gamma correction adjusts the overall brightness of the image's midtones without
    /// clipping the absolute black or white pixels.
    ///
    /// # Arguments
    ///
    /// * `value` - The gamma value to apply.
    ///   - Values `< 1.0` will lighten the image.
    ///   - Values `> 1.0` will darken the image.
    ///   - Typical ranges are between `0.8` and `2.4`.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with gamma correction applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn gamma(self, value: f32) -> Result<Self, ImageErrors>;
    /// Applies a fast Gaussian blur to the image.
    ///
    /// This uses an optimized 3-pass box blur approximation that provides near-perfect
    /// Gaussian smoothing but runs in constant time $O(1)$ relative to the blur radius.
    ///
    /// # Arguments
    ///
    /// * `sigma` - The standard deviation of the Gaussian distribution.
    ///   - A higher `sigma` creates a stronger, wider blur.
    ///   - A `sigma` of `0.0` or less results in no blur.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the blur applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn gaussian_blur(self, sigma: f32) -> Result<Self, ImageErrors>;

    /// Adjusts the Hue, Saturation, and Value of the image.
    ///
    /// This converts the image to RGBA temporarily to safely matrix-multiply the color
    /// channels without altering the alpha channel, then returns it to its original colorspace.
    ///
    /// # Arguments
    ///
    /// * `hue` - The hue rotation in degrees (typically `0.0` to `360.0`).
    /// * `saturation` - The saturation multiplier.
    ///   - `0.0` produces a grayscale image.
    ///   - `1.0` leaves saturation unchanged.
    ///   - `> 1.0` increases vibrancy.
    /// * `lightness` - The value/brightness multiplier.
    ///   - `0.0` produces a black image.
    ///   - `1.0` leaves brightness unchanged.
    ///   - `> 1.0` makes the image brighter.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the color adjustments applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn hsv_adjust(self, hue: f32, saturation: f32, lightness: f32) -> Result<Self, ImageErrors>;
    /// Converts the image between linear and gamma-encoded color spaces.
    ///
    /// Mathematical image operations generally yield more physically accurate results
    /// when performed on linear pixel data. Use this method to safely convert your image
    /// back and forth.
    ///
    /// # Arguments
    ///
    /// * `transfer_function` - The specific transfer curve to use (e.g., `TransferFunction::SRGB`).
    /// * `conversion_type` - Whether to linearize the image (`GammaToLinear`) or encode it (`LinearToGamma`).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the transfer curve applied and its internal `is_linear`
    /// metadata updated. Returns an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn transfer_curve(
        self, transfer_function: TransferFunction, conversion_type: ConversionType,
    ) -> Result<Self, ImageErrors>;
    /// Inverts the colors of the image, producing a negative.
    ///
    /// This mathematically subtracts every pixel's color value from the maximum
    /// value allowed by the image's bit depth. The alpha channel is ignored.
    ///
    /// # Arguments
    ///
    /// This method takes no additional arguments.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with inverted colors, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn invert(self) -> Result<Self, ImageErrors>;
    /// Applies a median filter to reduce noise while preserving edges.
    ///
    /// The median filter evaluates a window of `(2 * radius + 1)` squared pixels around
    /// each target pixel and replaces it with the median value. It is particularly
    /// useful for removing speckle or salt-and-pepper noise.
    ///
    /// # Arguments
    ///
    /// * `radius` - The radius of the neighboring area to search. A radius of `0` or `1`
    ///   does nothing. A radius of `2` results in a 5x5 search window.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the median filter applied, or an error if:
    /// - the image bit depth is unsupported (currently only supports `U8` and `U16`).
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn median_blur(self, radius: usize) -> Result<Self, ImageErrors>;
    /// Mirrors the image along a central axis.
    ///
    /// This creates perfect symmetry by duplicating one half of the image and
    /// reflecting it over the center line onto the other half.
    ///
    /// # Arguments
    ///
    /// * `mode` - A [`MirrorMode`] specifying which half of the image to preserve and reflect.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the mirror effect applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn mirror(self, mode: MirrorMode) -> Result<Self, ImageErrors>;

    /// Converts the image to or from a premultiplied alpha state.
    ///
    /// Premultiplying multiplies the RGB channels by the alpha channel. This is
    /// often required by graphics APIs (like WebGL or Vulkan) and provides better
    /// results when scaling images. Un-premultiplying reverses this process.
    ///
    /// If the image lacks an alpha channel, or is already in the requested state,
    /// this does nothing.
    ///
    /// # Arguments
    ///
    /// * `to` - The target `AlphaState` (`PreMultiplied` or `NonPreMultiplied`).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the alpha state converted, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn premultiply_alpha(self, to: AlphaState) -> Result<Self, ImageErrors>;
    /// Resizes the image using a specified dimension constraint and resampling algorithm.
    ///
    /// This method automatically handles linear color space conversions and alpha
    /// premultiplication to guarantee mathematically accurate, high-quality scaling.
    ///
    /// # Arguments
    ///
    /// * `dims` - A `ResizeDimensions` variant defining how the target size should be calculated
    ///   (e.g., preserving aspect ratio, forcing dimensions, scaling by percentage).
    /// * `method` - The `ResizeMethod` algorithm to use (e.g., `Lanczos3` for quality, `Bilinear` for speed).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` scaled to the calculated dimensions, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail during linearization or resampling.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying resizing or color space conversions fail.
    fn resize(self, dims: ResizeDimensions, method: ResizeMethod) -> Result<Self, ImageErrors>;
    /// Rotates the image by an arbitrary angle in degrees.
    ///
    /// The image's bounding box is automatically expanded to fit the rotated image.
    /// Orthogonal angles (90, 180, 270) are highly optimized and bypass trigonometric math.
    ///
    /// # Arguments
    ///
    /// * `angle` - The clockwise rotation angle in degrees.
    /// * `bg_color` - A normalized value (`0.0` to `1.0`) for the exposed background corners.
    ///   Use `0.0` for black/transparent, or `1.0` for white/opaque.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` containing the rotated pixels, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn rotate(self, angle: f32, bg_color: f32) -> Result<Self, ImageErrors>;
    /// Sharpens the image using an Unsharp Mask.
    ///
    /// This enhances edges and local contrast by comparing the image to a blurred
    /// version of itself and amplifying the differences.
    ///
    /// # Arguments
    ///
    /// * `sigma` - The radius of the Gaussian blur used to find edges. `1.0` to `3.0` is typical.
    /// * `threshold` - The minimum difference (0-65535) required to sharpen a pixel. Use higher
    ///   values to avoid sharpening noise.
    /// * `percentage` - The strength of the sharpening (e.g., `100` = 100% strength).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the sharpening applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn sharpen(self, sigma: f32, threshold: u16, percentage: u8) -> Result<Self, ImageErrors>;
    /// Linearly stretches the contrast of the image.
    ///
    /// This remaps the image's tonal range so that the specified `lower` value becomes
    /// completely black, the `upper` value becomes completely white, and everything in
    /// between is stretched linearly.
    ///
    /// # Arguments
    ///
    /// * `lower` - The value below which all pixels will be clamped to minimum (black).
    /// * `upper` - The value above which all pixels will be clamped to maximum (white).
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with stretched contrast, or an error if:
    /// - `upper` is not strictly greater than `lower`.
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn stretch_contrast(self, lower: f32, upper: f32) -> Result<Self, ImageErrors>;
    /// Applies a fixed-level threshold to the image.
    ///
    /// This is used to binarize images, truncate highlights, or drop low-intensity
    /// shadows depending on the chosen `ThresholdMethod`. It is highly recommended
    /// to convert the image to Grayscale before using this.
    ///
    /// # Arguments
    ///
    /// * `threshold` - The cutoff value. This is automatically clamped and cast to the
    ///   image's underlying bit depth (e.g., `0.0` to `255.0` for 8-bit).
    /// * `method` - The `ThresholdMethod` defining how pixels above and below the cutoff are treated.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the threshold applied, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn threshold(self, threshold: f32, method: ThresholdMethod) -> Result<Self, ImageErrors>;
    /// Transposes the image, swapping its rows and columns.
    ///
    /// This reflects the pixels across the top-left to bottom-right diagonal.
    /// The width and height of the image will be swapped.
    ///
    /// # Arguments
    ///
    /// This method takes no additional arguments.
    ///
    /// # Returns
    ///
    /// Returns a new `Image` with the transposed pixels, or an error if:
    /// - the image bit depth is unsupported.
    /// - internal operation invariants fail.
    ///
    /// # Errors
    ///
    /// Returns `ImageErrors` if the underlying operation fails.
    fn transpose(self) -> Result<Self, ImageErrors>;
}

impl FilterExt for Image {
    #[cfg(feature = "exif")]
    fn auto_orient(self) -> Result<Self, ImageErrors> {
        apply_op(self, crate::auto_orient::AutoOrient)
    }
    fn bilateral_filter(
        self, d: i32, sigma_color: f32, sigma_space: f32,
    ) -> Result<Self, ImageErrors> {
        apply_op(self, BilateralFilter::new(d, sigma_color, sigma_space))
    }
    fn box_blur(self, radius: usize) -> Result<Self, ImageErrors> {
        apply_op(self, crate::box_blur::BoxBlur::new(radius))
    }
    fn brighten(self, by: f32) -> Result<Self, ImageErrors> {
        apply_op(self, Brighten::new(by))
    }
    fn color_matrix(self, matrix: [[f32; 5]; 4]) -> Result<Self, ImageErrors> {
        apply_op(self, crate::color_matrix::ColorMatrix::new(matrix))
    }
    fn flip(self, direction: FlipDirection) -> Result<Self, ImageErrors> {
        apply_op(self, crate::flip::Flip::new(direction))
    }
    #[cfg(feature = "cms")]
    fn color_transform(
        self, target_profile: crate::color_transform::ColorProfiles,
    ) -> Result<Self, ImageErrors> {
        apply_op(
            self,
            crate::color_transform::ColorTransform::new(target_profile),
        )
    }
    fn convolve(self, weights: Vec<f32>, scale: f32) -> Result<Self, ImageErrors> {
        apply_op(self, crate::convolve::Convolve::new(weights, scale))
    }
    fn contrast(self, contrast: f32) -> Result<Self, ImageErrors> {
        apply_op(self, Contrast::new(contrast))
    }
    fn crop(self, width: usize, height: usize, x: usize, y: usize) -> Result<Self, ImageErrors> {
        apply_op(self, crate::crop::Crop::new(width, height, x, y))
    }
    fn exposure(self, exposure: f32, black: f32) -> Result<Self, ImageErrors> {
        apply_op(self, crate::exposure::Exposure::new(exposure, black))
    }
    #[cfg(feature = "exmex")]
    fn fx<S: Into<String>>(self, expression: S) -> Result<Self, ImageErrors> {
        apply_op(self, crate::fx::Fx::new(expression))
    }
    fn gamma(self, value: f32) -> Result<Self, ImageErrors> {
        apply_op(self, crate::gamma::Gamma::new(value))
    }
    fn gaussian_blur(self, sigma: f32) -> Result<Self, ImageErrors> {
        apply_op(self, crate::gaussian_blur::GaussianBlur::new(sigma))
    }
    fn hsv_adjust(self, hue: f32, saturation: f32, lightness: f32) -> Result<Self, ImageErrors> {
        apply_op(
            self,
            crate::hsv_adjust::HsvAdjust::new(hue, saturation, lightness),
        )
    }
    fn transfer_curve(
        self, transfer_function: TransferFunction, conversion_type: ConversionType,
    ) -> Result<Self, ImageErrors> {
        apply_op(self, TransferCurve::new(transfer_function, conversion_type))
    }
    fn invert(self) -> Result<Self, ImageErrors> {
        apply_op(self, crate::invert::Invert)
    }
    fn median_blur(self, radius: usize) -> Result<Self, ImageErrors> {
        apply_op(self, crate::median::Median::new(radius))
    }
    fn mirror(self, mode: MirrorMode) -> Result<Self, ImageErrors> {
        apply_op(self, crate::mirror::Mirror::new(mode))
    }
    fn premultiply_alpha(self, to: AlphaState) -> Result<Self, ImageErrors> {
        apply_op(self, crate::premul_alpha::PremultiplyAlpha::new(to))
    }
    fn resize(self, dims: ResizeDimensions, method: ResizeMethod) -> Result<Self, ImageErrors> {
        apply_op(self, crate::resize::Resize::new(dims, method))
    }
    fn rotate(self, angle: f32, bg_color: f32) -> Result<Self, ImageErrors> {
        apply_op(
            self,
            crate::rotate::Rotate::new_with_bg_color(angle, bg_color),
        )
    }
    fn sharpen(self, sigma: f32, threshold: u16, percentage: u8) -> Result<Self, ImageErrors> {
        apply_op(
            self,
            crate::sharpen::Sharpen::new(sigma, threshold, percentage),
        )
    }
    fn stretch_contrast(self, lower: f32, upper: f32) -> Result<Self, ImageErrors> {
        apply_op(
            self,
            crate::stretch_contrast::StretchContrast::new(lower, upper),
        )
    }
    fn threshold(self, threshold: f32, method: ThresholdMethod) -> Result<Self, ImageErrors> {
        apply_op(self, crate::threshold::Threshold::new(threshold, method))
    }
    fn transpose(self) -> Result<Self, ImageErrors> {
        apply_op(self, crate::transpose::Transpose::new())
    }
}
