pub static AFTER_HELP: &str = "";

pub static BRIGHTEN_HELP: &str = "Brighten or darken an image

Range is between -255 to 255. -255 gives a black image, 
255 gives a fully white image

Example: zune -i [img] -o [img] --brighten=-32 // darken the image";

pub static TRANSPOSE_HELP: &str = "Transpose an image

The transposition of an image is performed by swapping the 
X and Y indices of its array representation.

This mirrors the image along the image top-left to bottom-right diagonal";

pub static COLORSPACE_HELP: &str = "Set alternative image colorspace

E.g this can be set to decode JPEG RGB images to RGBA colorspace.
by adding an extra alpha channel set to opaque.";

pub static THRESHOLD_HELP: &str = "Replace pixels in an image depending on intensity of the pixel

Threshold methods supported supported are
\tbinary => max if src(x,y) > thresh 0 otherwise
\tbinary_inv => 0 if src(x,y) > thresh max otherwise
\tthresh_trunc => thresh if src(x,y) > thresh src(x,y) otherwise
\tthresh_to_zero => src(x,y) if src(x,y) > thresh 0 otherwise

See https://en.wikipedia.org/wiki/Thresholding_(image_processing)

Example: zune -i [img] -o [img] --threshold='32:binary'";

pub static CROP_HELP: &str = "Crop an image 


Cropping an image removes the outer unwanted layers from an image allowing focus on a subject

Format for this command is as follows
\tout_w: Out width, how wide the new image should be
\tout_h: Out height, how tall the new image should be
\tx: How many pixels horizontally from the origin should the cropping start from
\ty: How many pixels vertically from the origin should the cropping start from.

Origin is defined from the top left of the image.

Example: zune -i [img] -o [img] --crop='100:100:30:32' 

Creates a 100 by 100 pixel image with the pixel (0,0) being from (30,32) of the original image
";

pub static BOX_BLUR_HELP: &str = "Apply a box blur to an image

A box blur is simply an average of pixels across a length(defined by radius)

The greater the radius, the greater the blur effect,  a radius of 1 doesn't
do anything.

Speed is independent of radius";

pub static GAUSSIAN_BLUR_HELP: &str = "Apply a gaussian blur to an image

sigma is a measure of how much to blur by. The higher the sigma the more
pronounced the blur.

The implementation does not produce a true gaussian blur which involves convolving
a 2D kernel over the image as that is really slow, but we approximate it using very
fast 1D box blurs.
";
