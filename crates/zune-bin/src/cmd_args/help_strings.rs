pub static BRIGHTEN_HELP: &str = "Brighten or darken an image

Range is between -255 to 255. -255 gives a black image, 
255 gives a fully white image";

pub static TRANSPOSE_HELP: &str = "Transpose an image

The transposition of an image is performed by swapping the 
X and Y indices of its array representation.

This mirrors the image along the image top-left to bottom-right diagonal";

pub static COLORSPACE_HELP: &str = "Set alternative image colorspace

E.g this can be set to decode JPEG RGB images to RGBA colorspace.
by adding an extra alpha channel set to opaque.";
