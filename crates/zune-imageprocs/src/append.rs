use zune_core::bit_depth::BitType;
use zune_image::channel::Channel;
use zune_image::errors::ImageErrors;
use zune_image::frame::Frame;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};

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

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Any
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

        // Take all images out
        let imgs: Vec<Image> = std::mem::take(images);

        // Validate against the first image
        let first = &imgs[0];
        let depth = first.depth();
        let colorspace = first.colorspace();

        for img in &imgs[1..] {
            if img.depth() != depth {
                return Err(ImageErrors::GenericStr(
                    "Image depths do not match for append",
                ));
            }
            if img.colorspace() != colorspace {
                return Err(ImageErrors::GenericStr(
                    "Image colorspaces do not match for append",
                ));
            }
        }

        // Compute final dimensions
        let (new_width, new_height) = match self.direction {
            AppendDirection::Horizontal => {
                let width = imgs.iter().map(|i| i.dimensions().0).sum();
                let height = imgs.iter().map(|i| i.dimensions().1).max().unwrap();
                (width, height)
            }
            AppendDirection::Vertical => {
                let width = imgs.iter().map(|i| i.dimensions().0).max().unwrap();
                let height = imgs.iter().map(|i| i.dimensions().1).sum();
                (width, height)
            }
        };

        let bit_type = depth.bit_type();

        let mut new_frames = Vec::new();

        // Assume same frame count across images
        for frame_idx in 0..first.frames_ref().len() {
            let mut new_channels = Vec::new();

            let channels_len = first.frames_ref()[frame_idx]
                .channels_ref(colorspace, false)
                .len();

            for ch_idx in 0..channels_len {
                let new_dims = new_width * new_height;
                let mut out_channel = Channel::new_with_bit_type(new_dims, bit_type);

                match bit_type {
                    BitType::U8 => append_many::<u8>(
                        &imgs,
                        frame_idx,
                        ch_idx,
                        out_channel.reinterpret_as_mut()?,
                        new_width,
                        self.direction,
                    )?,
                    BitType::U16 => append_many::<u16>(
                        &imgs,
                        frame_idx,
                        ch_idx,
                        out_channel.reinterpret_as_mut()?,
                        new_width,
                        self.direction,
                    )?,
                    BitType::F32 => append_many::<f32>(
                        &imgs,
                        frame_idx,
                        ch_idx,
                        out_channel.reinterpret_as_mut()?,
                        new_width,
                        self.direction,
                    )?,
                    _ => {
                        return Err(ImageErrors::ImageOperationNotImplemented(
                            self.name(),
                            bit_type,
                        ))
                    }
                }

                new_channels.push(out_channel);
            }

            new_frames.push(Frame::new(new_channels));
        }

        let result = Image::new_frames(new_frames, depth, new_width, new_height, colorspace);
        images.push(result);

        Ok(())
    }
}


fn append_many<T: Copy+Default +'static>(
    imgs: &[Image],
    frame_idx: usize,
    ch_idx: usize,
    out: &mut [T],
    new_width: usize,
    direction: AppendDirection,
) -> Result<(), ImageErrors> {
    let mut offset_x = 0;
    let mut offset_y = 0;

    for img in imgs {
        let (w, h) = img.dimensions();

        let frame = &img.frames_ref()[frame_idx];
        let channel = &frame.channels_ref(img.colorspace(), false)[ch_idx];
        let data: &[T] = channel.reinterpret_as()?;

        match direction {
            AppendDirection::Horizontal => {
                for y in 0..h {
                    let out_row = &mut out[(y * new_width + offset_x)
                        ..(y * new_width + offset_x + w)];
                    let src_row = &data[(y * w)..((y + 1) * w)];
                    out_row.copy_from_slice(src_row);
                }
                offset_x += w;
            }
            AppendDirection::Vertical => {
                for y in 0..h {
                    let out_row = &mut out[((offset_y + y) * new_width)
                        ..((offset_y + y) * new_width + w)];
                    let src_row = &data[(y * w)..((y + 1) * w)];
                    out_row.copy_from_slice(src_row);
                }
                offset_y += h;
            }
        }
    }

    Ok(())
}