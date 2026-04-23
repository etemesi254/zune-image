use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::frame::Frame;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Direction to append the images
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum AppendDirection {
    /// Stitch images side-by-side (Left to Right)
    Horizontal,
    /// Stitch images top-to-bottom
    Vertical,
}

/// Append filter
///
/// Takes the last two images on the stack and stitches them together.
pub struct Append {
    direction: AppendDirection,
}

impl Append {
    #[must_use]
    pub fn new(direction: AppendDirection) -> Self {
        Append { direction }
    }
}

impl OperationsTrait for Append {
    fn name(&self) -> &'static str {
        "Append"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Append requires multiple images; it must be called via execute_multiple",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        if images.len() < 2 {
            return Err(ImageErrors::GenericStr(
                "Append requires at least two images in the pipeline",
            ));
        }

        // 1. POP the stack
        // The last image loaded is the "Right" or "Bottom" image
        let img2 = images.pop().unwrap();

        // The previously loaded image is the "Left" or "Top" image
        // Notice we also pop this one, because we are going to create a brand
        // new canvas and push that back instead!
        let img1 = images.pop().unwrap();

        // 2. Validate compatibility
        if img1.depth() != img2.depth() {
            return Err(ImageErrors::GenericStr(
                "Image depths do not match for append",
            ));
        }
        if img1.colorspace() != img2.colorspace() {
            return Err(ImageErrors::GenericStr(
                "Image colorspaces do not match for append",
            ));
        }

        let depth = img1.depth();

        // 3. Calculate new canvas dimensions
        let (w1, h1) = img1.dimensions();
        let (w2, h2) = img2.dimensions();

        let (new_width, new_height) = match self.direction {
            AppendDirection::Horizontal => (w1 + w2, h1.max(h2)),
            AppendDirection::Vertical => (w1.max(w2), h1 + h2),
        };
        let bit_type = depth.bit_type();
        let colorspace = img1.colorspace();

        let mut new_frames = Vec::new();
        for (frame1, frame2) in img1.frames_ref().iter().zip(img2.frames_ref().iter()) {
            assert_eq!(
                frame1.channels_ref(colorspace, false).len(),
                frame2.channels_ref(colorspace, false).len()
            );
            let new_dims = new_width.wrapping_mul(new_height);
            // now dealing with the channels
            let mut new_channels = Vec::new();
            for (c1, c2) in frame1
                .channels_ref(colorspace, false)
                .iter()
                .zip(frame2.channels_ref(colorspace, false))
            {
                let mut im_channel = Channel::new_with_bit_type(new_dims, bit_type);

                match bit_type {
                    BitType::U8 => append_channel_data::<u8>(
                        c1.reinterpret_as()?,
                        c2.reinterpret_as()?,
                        im_channel.reinterpret_as_mut()?,
                        w1,
                        h1,
                        w2,
                        h2,
                        new_width,
                        self.direction,
                    ),
                    BitType::U16 => append_channel_data::<u16>(
                        c1.reinterpret_as()?,
                        c2.reinterpret_as()?,
                        im_channel.reinterpret_as_mut()?,
                        w1,
                        h1,
                        w2,
                        h2,
                        new_width,
                        self.direction,
                    ),
                    BitType::F32 => append_channel_data::<f32>(
                        c1.reinterpret_as()?,
                        c2.reinterpret_as()?,
                        im_channel.reinterpret_as_mut()?,
                        w1,
                        h1,
                        w2,
                        h2,
                        new_width,
                        self.direction,
                    ),
                    _ => {
                        return Err(ImageErrors::ImageOperationNotImplemented(
                            self.name(),
                            bit_type,
                        ))
                    }
                }
                new_channels.push(im_channel);
            }
            let frame = Frame::new(new_channels);
            new_frames.push(frame);
        }
        let img = Image::new_frames(new_frames, depth, new_width, new_height, colorspace);

        images.push(img);

        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn append_channel_data<T: Copy>(
    c1: &[T], c2: &[T], out: &mut [T], w1: usize, h1: usize, w2: usize, h2: usize,
    new_width: usize, direction: AppendDirection,
) {
    match direction {
        AppendDirection::Horizontal => {
            // Copy Image 1 (Left)
            for y in 0..h1 {
                let out_row = &mut out[(y * new_width)..(y * new_width + w1)];
                let c1_row = &c1[(y * w1)..((y + 1) * w1)];
                out_row.copy_from_slice(c1_row);
            }
            // Copy Image 2 (Right)
            for y in 0..h2 {
                let out_row = &mut out[(y * new_width + w1)..(y * new_width + w1 + w2)];
                let c2_row = &c2[(y * w2)..((y + 1) * w2)];
                out_row.copy_from_slice(c2_row);
            }
        }
        AppendDirection::Vertical => {
            // Copy Image 1 (Top)
            for y in 0..h1 {
                let out_row = &mut out[(y * new_width)..(y * new_width + w1)];
                let c1_row = &c1[(y * w1)..((y + 1) * w1)];
                out_row.copy_from_slice(c1_row);
            }
            // Copy Image 2 (Bottom)
            // Offset the Y axis by Image 1's height (h1)
            for y in 0..h2 {
                let out_row = &mut out[((h1 + y) * new_width)..((h1 + y) * new_width + w2)];
                let c2_row = &c2[(y * w2)..((y + 1) * w2)];
                out_row.copy_from_slice(c2_row);
            }
        }
    }
}
