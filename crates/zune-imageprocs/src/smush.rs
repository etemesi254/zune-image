use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

#[derive(Copy, Clone, Debug)]
pub enum SmushDirection {
    Horizontal,
    Vertical,
}
impl From<&str> for SmushDirection {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "v" | "vertical" => SmushDirection::Vertical,
            _ => SmushDirection::Horizontal, // Default to horizontal
        }
    }
}

/// Join multiple images into a single image with a defined offset.
///
/// Smush is an image sequence operator. It takes all images currently in the
/// processing stack, merges them into a single image, and replaces the
/// stack with the result.
///
/// Unlike a simple append, `Smush` allows for:
/// - **Gaps**: Positive offsets leave empty space between images.
/// - **Overlaps**: Negative offsets cause subsequent images to be drawn over previous ones.
///
/// # Layout
///
/// | Direction | Behavior |
/// | :--- | :--- |
/// | `Horizontal` | Images are placed side-by-side. The height of the result is the height of the tallest image. |
/// | `Vertical` | Images are placed top-to-bottom. The width of the result is the width of the widest image. |
///
/// # Examples
///
/// ```rust
///
/// use zune_imageprocs::smush::{Smush, SmushDirection};
///
/// // Create a horizontal strip with a 10-pixel gap between images
/// let op = Smush::new(SmushDirection::Horizontal, 10);
///
/// // Create a vertical collage where images overlap by 50 pixels
/// let overlap = Smush::new(SmushDirection::Vertical, -50);
/// ```
pub struct Smush {
    direction: SmushDirection,
    offset: i32,
}

impl Smush {
    /// Create a new Smush operation.
    ///
    /// # Arguments
    /// * `direction` - The axis along which to join images.
    /// * `offset` - Space between images (positive for gaps, negative for overlap).
    #[must_use]
    pub fn new(direction: SmushDirection, offset: i32) -> Self {
        Self { direction, offset }
    }
}
impl OperationsTrait for Smush {
    fn name(&self) -> &'static str {
        "Smush"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        // Typically Smush requires at least two images.
        // If this is called on a single image, it's a no-op or error.
        Err(ImageErrors::GenericStr(
            "Smush requires an image sequence (multiple images)",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    #[allow(clippy::cast_possible_wrap)]
    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.is_empty() {
            return Err(ImageErrors::GenericStr("No images provided to smush"));
        }
        if images.len() == 1 {
            return Ok(());
        }

        let first = &images[0];
        let colorspace = first.colorspace();
        let depth = first.depth();
        let bit_type = depth.bit_type();

        // 1. Calculate new dimensions
        let mut out_w = first.width();
        let mut out_h = first.height();

        for img in images.iter().skip(1) {
            match self.direction {
                SmushDirection::Horizontal => {
                    out_w = (out_w as i32 + img.width() as i32 + self.offset).max(0) as usize;
                    out_h = out_h.max(img.height());
                }
                SmushDirection::Vertical => {
                    out_h = (out_h as i32 + img.height() as i32 + self.offset).max(0) as usize;
                    out_w = out_w.max(img.width());
                }
            }
        }

        // 2. Initialize output image
        let mut out_img = match bit_type {
            BitType::U8 => Image::fill(0_u8, colorspace, out_w, out_h),
            BitType::U16 => Image::fill(0_u16, colorspace, out_w, out_h),
            BitType::F32 => Image::fill(0_f32, colorspace, out_w, out_h),
            _ => {
                return Err(ImageErrors::GenericStr("Invalid bit_type"));
            }
        };

        let mut curr_x = 0;
        let mut curr_y = 0;

        for img in images.iter() {
            let (img_w, img_h) = img.dimensions();

            for (ch_idx, src_channel) in img.frames_ref()[0]
                .channels_ref(colorspace, false)
                .iter()
                .enumerate()
            {
                let dest_channel =
                    &mut out_img.frames_mut()[0].channels_mut(colorspace, false)[ch_idx];

                match bit_type {
                    BitType::U8 => {
                        smush_plane_generic::<u8>(
                            src_channel.reinterpret_as()?,
                            dest_channel.reinterpret_as_mut()?,
                            curr_x,
                            curr_y,
                            img_w,
                            img_h,
                            out_w,
                        );
                    }
                    BitType::U16 => {
                        smush_plane_generic::<u16>(
                            src_channel.reinterpret_as()?,
                            dest_channel.reinterpret_as_mut()?,
                            curr_x,
                            curr_y,
                            img_w,
                            img_h,
                            out_w,
                        );
                    }
                    BitType::F32 => {
                        smush_plane_generic::<f32>(
                            src_channel.reinterpret_as()?,
                            dest_channel.reinterpret_as_mut()?,
                            curr_x,
                            curr_y,
                            img_w,
                            img_h,
                            out_w,
                        );
                    }
                    _ => return Err(ImageErrors::ImageOperationNotImplemented("Smush", bit_type)),
                }
            }

            match self.direction {
                SmushDirection::Horizontal => {
                    curr_x += (img_w as i32 + self.offset).max(0) as usize;
                }
                SmushDirection::Vertical => curr_y += (img_h as i32 + self.offset).max(0) as usize,
            }
        }

        images.clear();
        images.push(out_img);
        Ok(())
    }
}

/// Core logic for smushing two planar buffers together
fn smush_plane_generic<T: Copy + Default>(
    src: &[T], dest: &mut [T], x_off: usize, y_off: usize, src_w: usize, src_h: usize,
    dest_w: usize,
) {
    for row in 0..src_h {
        let src_start = row * src_w;
        let src_end = src_start + src_w;

        let dest_row = y_off + row;
        let dest_start = dest_row * dest_w + x_off;
        let dest_end = dest_start + src_w;

        // Bounds check to ensure we don't panic on negative offsets or mismatched sizes
        if dest_end <= dest.len() {
            dest[dest_start..dest_end].copy_from_slice(&src[src_start..src_end]);
        }
    }
}
