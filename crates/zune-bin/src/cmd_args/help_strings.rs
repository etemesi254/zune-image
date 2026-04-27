pub const AFTER_HELP: &str = "";

pub const BRIGHTEN_HELP: &str = "Brighten or darken an image.

Range is between -255 to 255. 
-255 gives a black image, 255 gives a fully white image.

USAGE EXAMPLES:
  --brighten -32   (Darkens the image slightly)
  --brighten 100   (Significantly brightens the image)";

pub const TRANSPOSE_HELP: &str = "Transpose an image.

The transposition of an image is performed by swapping the 
X and Y indices of its array representation.
This mirrors the image along the image top-left to bottom-right diagonal.";

pub const COLORSPACE_HELP: &str = "Set alternative image colorspace.

E.g. this can be set to decode JPEG RGB images to RGBA colorspace 
by adding an extra alpha channel set to opaque.";

pub const THRESHOLD_HELP: &str = "Replace pixels in an image depending on intensity of the pixel.

FORMAT:
  <threshold>:<mode>

MODES:
  binary         => max if src(x,y) > thresh, 0 otherwise
  binary_inv     => 0 if src(x,y) > thresh, max otherwise
  thresh_trunc   => thresh if src(x,y) > thresh, src(x,y) otherwise
  thresh_to_zero => src(x,y) if src(x,y) > thresh, 0 otherwise

USAGE EXAMPLES:
  --threshold 128:binary     (Standard black/white thresholding)
  --threshold 200:binary_inv (Inverted thresholding for bright spots)";

pub const CROP_HELP: &str = "Crop an image.

Cropping an image removes the outer unwanted layers from an image allowing focus on a subject.
Origin is defined from the top left of the image.

FORMAT:
  <width> <height> <x> <y>

PARAMETERS:
  * width:  How wide the new image should be
  * height: How tall the new image should be
  * x:      How many pixels horizontally from the origin cropping should start
  * y:      How many pixels vertically from the origin cropping should start

USAGE EXAMPLES:
  --crop 100 100 30 32 
  (Creates a 100x100 pixel image starting at coordinate X:30, Y:32 of the original)";

pub const BOX_BLUR_HELP: &str = "Apply a box blur to an image.

A box blur is simply an average of pixels across a length (defined by radius).
The greater the radius, the greater the blur effect. A radius of 1 doesn't do anything.
Speed is independent of radius.

USAGE EXAMPLES:
  --box-blur 5    (Standard box blur)
  --box-blur 50   (Heavy, wide blur)";

pub const GAUSSIAN_BLUR_HELP: &str = "Apply a gaussian blur to an image.

Sigma is a measure of how much to blur by. The higher the sigma the more pronounced the blur.
The implementation approximates a true 2D Gaussian kernel using very fast 1D box blurs.

USAGE EXAMPLES:
  --blur 1.5      (Subtle, natural-looking blur)
  --blur 5.0      (Strong blur)";

pub const RESIZE_HELP: &str = "Resize an image using ImageMagick-style geometry.

This command supports standard exact dimensions, percentages, and complex aspect-ratio bounding boxes using trailing modifier characters.

SUPPORTED FORMATS & MODIFIERS:
  * WxH       (Fit Within): Fits the image entirely inside the WxH box while preserving the original aspect ratio. (Default)
  * WxH^      (Fill):       Scales the image to completely cover the WxH box, preserving the aspect ratio.
  * WxH!      (Exact):      Forces the image to exactly WxH, ignoring the original aspect ratio (may distort).
  * WxH>      (Shrink):     Resizes the image to fit within WxH, but ONLY if the image is currently larger than the target.
  * WxH<      (Enlarge):    Resizes the image to fit within WxH, but ONLY if the image is currently smaller than the target.
  * W         (Width Only): E.g., '800'. Scales the width to 800 and automatically calculates the height to keep the aspect ratio.
  * xH        (Height Only): E.g., 'x600'. Scales the height to 600 and automatically calculates the width.
  * Area@     (Pixel Area): E.g., '400000@'. Scales the image so its total pixel count (W * H) equals the target area.
  * P%        (Uniform %):  E.g., '50%'. Scales both the width and height by the given percentage.
  * W%xH%     (Indep. %):   E.g., '50%x75%'. Scales the width and height by separate percentages.

NOTE ON SHELL USAGE:
If you use modifiers like '>', '<', or '^', you should wrap the argument in quotes to prevent your shell from interpreting them as redirects or pipes.

USAGE EXAMPLES:
  --resize 1920x1080         (Fits inside a 1080p box)
  --resize \"1920x1080^\"      (Covers a 1080p box)
  --resize \"1920x1080!\"      (Forces exactly 1920x1080)
  --resize \"800x600>\"        (Shrinks large images, leaves small ones alone)
  --resize 800               (Sets width to 800, scales height automatically)
  --resize x600              (Sets height to 600, scales width automatically)
  --resize 50%               (Shrinks image to half its size)";

pub const BILATERAL_FILTER_HELP: &str =
    "Applies a bilateral filter to the image, which reduces noise while preserving edges.

FORMAT:
  <d>,<sigma_color>,<sigma_space>

PARAMETERS:
  * d (integer):
      Diameter of each pixel neighborhood used during filtering.
      If <= 0, it is automatically computed from sigma_space.
  * sigma_color (float):
      Filter sigma in the color space. A larger value mixes farther colors
      within the pixel neighborhood, resulting in larger areas of semi-equal color.
  * sigma_space (float):
      Filter sigma in the coordinate space. A larger value means farther pixels
      will influence each other as long as their colors are close enough.

USAGE EXAMPLES:
  --bilateral 9,75.0,75.0      (Standard smoothing)
  --bilateral -1,50.0,50.0     (Compute diameter automatically from space)";

pub const GRAYSCALE_HELP: &str = "Change image type from RGB to grayscale. This removes all color information and leaves only luminance.";

pub const FLIP_HELP: &str = "Flip an image on the vertical axis (top-to-bottom).";

pub const FLOP_HELP: &str = "Flop an image on the horizontal axis (left-to-right).";

pub const V_FLIP_HELP: &str = "Flip an image on the vertical axis (alias for flip).";

pub const MIRROR_HELP: &str = "Mirror the image in a specific direction.

VALID OPTIONS: 
  north, south, east, west

USAGE EXAMPLES:
  --mirror east";

pub const INVERT_HELP: &str = "Invert the colors of the image. For example, a black pixel becomes white, and a white pixel becomes black.";

pub const GAMMA_HELP: &str = "Perform a gamma correction on the image.

Values > 1.0 will lighten the midtones, while values < 1.0 will darken them.

USAGE EXAMPLES:
  --gamma 2.2     (Standard sRGB gamma correction)
  --gamma 0.8     (Darken midtones)";

pub const STRETCH_CONTRAST_HELP: &str = "Linearly stretch the contrast of an image.

Expects a lower and upper bound to stretch between,
effectively redistributing the pixel intensities to fill the available dynamic range.

USAGE EXAMPLES:
  --stretch-contrast 10.0 245.0";

pub const CONTRAST_HELP: &str = "Adjust the contrast of the image.

Positive values increase contrast (making brights brighter and darks darker),
while negative values decrease it (washing out the image).

USAGE EXAMPLES:
  --contrast 20.0   (Increase contrast)
  --contrast -15.0  (Decrease contrast)";

pub const RESIZE_METHOD_HELP: &str = "The algorithm to use when resizing the image.

VALID OPTIONS:
  Lanczos3, Lanczos2, Bicubic, CatmullRom, Mitchell, BSpline, Hermite, Sinc, Bilinear.

Defaults to Bicubic if not specified.";

pub const DEPTH_HELP: &str = "Change the bit depth of the image (e.g., 8 or 16).

USAGE EXAMPLES:
  --depth 8       (Standard 8-bit per channel)
  --depth 16      (High-color 16-bit per channel)";

pub const AUTO_ORIENT_HELP: &str = "Automatically orient the image based on its EXIF Orientation tag.
This will automatically rotate or flip the image so it displays correctly based on how the camera was held.";

pub const EXPOSURE_HELP: &str = "Adjust the exposure of the image.

Simulates adjusting the f-stop/EV on a camera. Values are capped between -3.0 and 3.0.
A value of 1.0 doubles the brightness, while -1.0 halves it.

USAGE EXAMPLES:
  --exposure 1.5    (Significantly increase exposure)
  --exposure -0.5   (Slightly darken an overexposed image)";

pub const HUEROTATE_HELP: &str = "Apply a hue rotation to the image.

Accepts degrees between 0.0 and 360.0. This shifts all colors in the image around the color wheel.

USAGE EXAMPLES:
  --huerotate 90.0  (Shifts reds to greens, greens to blues, etc.)
  --huerotate 180.0 (Inverts the hues completely)";

pub const SATURATE_HELP: &str = "Adjust the color saturation of the image.

Positive values increase saturation (making colors more vibrant),
while negative values decrease it (pushing the image closer to grayscale).

USAGE EXAMPLES:
  --saturate 2.0    (Double the saturation)
  --saturate -0.5   (Reduce saturation by 50%)";

pub const LIGHTNESS_HELP: &str = "Adjust the overall lightness of the image.

Unlike exposure (which acts multiplicatively) or brighten (which adds raw pixel values),
lightness adjusts the L-channel in an HSL/LAB context. Accepts both positive and negative float values.

USAGE EXAMPLES:
  --lightness 10.0  (Lighten the image)
  --lightness -20.0 (Darken the image deeply)";

pub const ROTATE_HELP: &str = "Rotate the image clockwise by a specific angle in degrees.

NOTE ON AFFINE TRANSFORMATIONS:
For angles that are not exact right angles (e.g., 90, 180, 270),
the rotation is performed using an affine transformation.
This means the resulting image bounding box will grow to accommodate the tilted image,
 and the empty triangular areas at the corners will be filled with a background padding color.

USAGE EXAMPLES:
  --rotate 90     (Perfect right-angle rotation, keeps exact bounds)
  --rotate 45     (Affine rotation, introduces padding)
  --rotate -15    (Rotates counter-clockwise by 15 degrees)";

pub const QUALITY_HELP: &str =
    "Set the encoding quality for output formats that support it (e.g., JPEG, WebP).
Accepts an integer from 0 to 100, where 100 is maximum quality and lowest compression.

USAGE EXAMPLES:
  --quality 85";

pub const ENCODE_THREADS_HELP: &str =
    "Set the number of threads to use during image encoding to speed up the process.";

pub const EFFORT_HELP: &str =
    "Set the encoding effort/speed tradeoff for formats that support it (like WebP or AVIF).
Higher values result in smaller file sizes but take longer to encode.";

pub const PROGRESSIVE_HELP: &str = "Encode the image using progressive encoding.
This allows the image to load in successive waves of increasing quality over the web.";

pub const STRIP_HELP: &str =
    "Strip all metadata (EXIF, ICC profiles, etc.) from the image before saving.
Useful for reducing file size and removing sensitive information.";

pub const UNSHARPEN_HELP: &str = "Apply an unsharp mask to sharpen the image.

This algorithm works by comparing the image to a blurred version of itself.
If the brightness difference between the two exceeds the threshold,
the contrast at that edge is increased.

FORMAT:
  <sigma> <threshold> <percentage>

PARAMETERS:
  * sigma (float):
      The radius of the underlying Gaussian blur. Larger values affect wider edges.
  * threshold (integer):
      The minimum brightness difference (e.g., 0-255) needed before a pixel is sharpened.
  * percentage (integer):
      The strength multiplier for the sharpening effect.

USAGE EXAMPLES:
  --unsharpen 1.0 10 100    (Standard edge sharpening)
  --unsharpen 2.5 0 150     (Aggressive sharpening on wider edges)";

pub const STATISTIC_HELP: &str =
    "Replace each pixel with a specified statistic from its surrounding neighborhood.

FORMAT:
  <radius> <statistic>

VALID STATISTICS:
  median, mean, min, max, mode

USAGE EXAMPLES:
  --statistic 3 median   (Applies a 3px radius median filter, good for noise)
  --statistic 5 max      (Applies a 5px radius maximum filter, dilates bright areas)";

pub const MEAN_BLUR_HELP: &str = "Perform a mean blur on the image.
Requires a radius. Each pixel is replaced by the average of its neighbors.";

pub const SOBEL_HELP: &str = "Apply a 3x3 Sobel operator to find and highlight edges in the image.";

pub const SCHARR_HELP: &str = "Apply a 3x3 Scharr operator for edge detection.
Similar to Sobel but more sensitive to subtle gradients.";

pub const CONVOLVE_HELP: &str = "Apply a custom 2D NxN convolution matrix to the image.

N must be 3, 5, or 7. You must provide exactly N*N values sequentially.

USAGE EXAMPLES:
  --convolve 0 -1 0 -1 5 -1 0 -1 0 
  (Applies a standard 3x3 sharpening matrix)
  
  --convolve -1 -1 -1 -1 8 -1 -1 -1 -1
  (Applies a 3x3 edge-detection matrix)";

pub const MEDIAN_BLUR_HELP: &str = "Perform a median blur on the image.

Replaces each pixel with the median value of its neighbors within the specified radius.
Excellent for removing salt-and-pepper noise without destroying sharp edges.

USAGE EXAMPLES:
  --median-blur 3";

pub const COLOR_TRANSFORM_HELP: &str =
    "Parse the ICC profile of the image and perform a color space transformation.

VALID OPTIONS: 
  rgb, adobe-rgb, display-p3, bt-2020

USAGE EXAMPLES:
  --color-transform display-p3";

pub const AFFINE_TRANSFORM_HELP: &str = "Apply a 2D affine transformation to the image.

An affine transformation is a linear mapping method that preserves points,
 straight lines, and planes. It requires 6 parameters representing a 3x3 matrix.

FORMAT:
  <a> <b> <c> <d> <tx> <ty>

MATRIX REPRESENTATION:
  | a  b  tx |
  | c  d  ty |
  | 0  0  1  |

* a & d control scaling.
* b & c control shearing/rotation.
* tx & ty control X and Y translation.

USAGE EXAMPLES:
  --affine-transform 1.0 0.0 0.0 1.0 50.0 100.0  
  (Translates the image 50px right and 100px down)
  
  --affine-transform 2.0 0.0 0.0 2.0 0.0 0.0     
  (Scales the image by 2x in both directions)";

pub const COMPOSITE_HELP: &str = r#"Composite the last two loaded images together.

This operation uses a stack-based architecture. It requires at least two 
input images (-i) to be loaded into the pipeline. 

STACK MECHANICS:
When --composite is called, it removes the last loaded image from the 
pipeline stack to use as the "Source" (overlay). It then composites this 
onto the previously loaded image, which acts as the "Destination" (background).

AVAILABLE METHODS:
  Over    - Standard alpha blending. Places the source over the destination.
  Src     - Replaces the destination completely with the source image.
  Dst     - Does nothing (leaves the destination image unchanged).
  DstIn   - Masks the background using the source's alpha channel.

EXAMPLES:
1. Basic overlay (defaults to top-left corner 0,0):
   zune -i background.png -i logo.png --composite Over -o final.png

2. Offset the overlay using --geometry (x,y):
   zune -i bg.png -i logo.png --composite Over --geometry 150,50 -o final.png

Note: Both images must have the same bit depth and colorspace."#;

pub const BLEND_HELP: &str = "Blend the last two loaded images together.

This operation uses a stack-based architecture. It requires at least two 
input images (-i) to be loaded into the pipeline. 

STACK MECHANICS:
It removes the last loaded image from the stack (Source) and blends it onto 
the previously loaded image (Destination) based on the provided alpha.

ALPHA:
A float between 0.0 and 1.0. 
  - 0.0 keeps only the destination image.
  - 0.5 mixes them equally.
  - 1.0 keeps only the source image.

EXAMPLE:
  zune -i background.png -i overlay.png --blend 0.5 -o output.png";

pub const APPEND_HELP: &str = r#"Append (stitch) the last N loaded images together.

This removes the last n images from the stack, combines them into a
single larger image, and pushes the new image back onto the stack.

OPTIONS:
  horizontal - Stitches side-by-side (First image on Left, N on Right)
  vertical   - Stitches top-to-bottom (First image on Top, N on Bottom)

EXAMPLE:
  zune -i left.png -i right.png --append horizontal -o wide_output.png"#;

pub const SSIM_HELP: &str = r#"Calculate the Structural Similarity Index Measure (SSIM).

This compares the last two images loaded onto the stack and prints their
similarity score (MSSIM). A score of 1.0 means the images are perfectly identical.

STACK MECHANICS:
This operation is NON-DESTRUCTIVE. It peeks at the top two images but leaves them on the stack.

EXAMPLE:
  zune -i reference.png -i compressed.jpg --ssim
"#;

pub const HALD_CLUT: &str = r#"Apply a Hald-CLUT color grade to an image.

This operation uses a stack-based architecture. It requires exactly two 
input images (-i) to be loaded into the pipeline. 

STACK MECHANICS:
1. Load your target image (the photo you want to color grade).
2. Load your Hald-CLUT image (the 3D color lookup table, usually a 512x512 PNG).
3. Call --hald-clut. 

The CLUT image is popped from the stack, its color mappings are applied to 
the target image, and the target image remains on the stack for saving.

EXAMPLE:
  zune -i raw_photo.jpg -i cinematic_clut.png --hald-clut -o graded_photo.jpg"#;

pub const AVERAGE_HELP: &str = r#"Average all currently loaded images into a single image.

This operation consumes the ENTIRE stack. It calculates the mean value 
for every pixel across all loaded images to generate a single output image. 
This is highly effective for reducing high ISO noise in low-light photography.

STACK MECHANICS:
Pops all N images from the stack. Pushes 1 averaged image back.

EXAMPLE (Averaging 3 noisy photos):
  zune -i photo1.jpg -i photo2.jpg -i photo3.jpg --average -o clean_photo.jpg"#;
