use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

use crate::traits::NumOps;
use crate::utils::{calculate_gravity, Gravity};

/// Composite method to use for composing
///
/// Not all composite methods are supported
#[derive(Copy, Clone, Debug)]
pub enum CompositeMethod {
    /// Put the source over the destination
    Over,
    /// Completely replace the background image with the overlay image.
    ///
    /// Colors and transparency are removed, leaving a blank image the same size as original
    /// over which it is applied to the source image
    Src,
    /// Does nothing compose
    Dst,
    /// Mask the background with shape
    DstIn
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum CompositeMethodType {
    ChannelBased,
    AlphaChannel
}
impl CompositeMethod {
    fn composite_type(self) -> CompositeMethodType {
        match self {
            CompositeMethod::Src | CompositeMethod::Dst | CompositeMethod::Over => {
                CompositeMethodType::ChannelBased
            }
            CompositeMethod::DstIn => CompositeMethodType::AlphaChannel
        }
    }
}

pub struct Composite<'a> {
    geometry:         Option<(usize, usize)>,
    src_image:        &'a Image,
    composite_method: CompositeMethod,
    gravity:          Option<Gravity>
}

impl<'a> Composite<'a> {
    /// Create a new filter that will copy an image to a specific location specified by `position`
    /// using  the composite method specified.
    ///
    /// # Arguments
    /// - image: The source image, this will be composited on top of the destination image
    /// - composite_method: The composite technique we are using to join two images together
    ///  - position: The absolute position to place the source image on top of the destination image
    ///   A tuple of (x,y) coordinate
    ///
    /// See also [Self::new_gravity] if you don't want to manually calculate coordinates
    pub fn new(
        image: &'a Image, composite_method: CompositeMethod, position: (usize, usize)
    ) -> Composite<'a> {
        Composite {
            geometry: Some(position),
            src_image: image,
            composite_method,
            gravity: None
        }
    }
    /// Create a new filter that will composite `image` with the dest image placing it in the location
    /// specified by gravity using the composite method specified.
    ///
    /// # Arguments
    /// - image: The source image, this will be composited on top of the destination image
    /// - composite_method: The composite technique we are using to join two images together
    /// - gravity: The location to place the image, useful for when you don't want to manually calculate the image coordinates yourself.
    ///
    ///
    /// See also [Self::new] if you want to use absolute coordinates
    pub fn new_gravity(
        image: &'a Image, composite_method: CompositeMethod, gravity: Gravity
    ) -> Composite<'a> {
        Composite {
            geometry: None,
            gravity: Some(gravity),
            src_image: image,
            composite_method
        }
    }
}

impl<'a> OperationsTrait for Composite<'a> {
    fn name(&self) -> &'static str {
        "Composite"
    }

    #[allow(clippy::too_many_lines)]
    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors> {
        let dims = if self.gravity.is_some() {
            calculate_gravity(self.src_image, image, self.gravity.unwrap())
        } else if self.geometry.is_some() {
            self.geometry.unwrap()
        } else {
            unreachable!()
        };
        let (src_width, _) = self.src_image.dimensions();
        let (dst_width, _) = image.dimensions();
        // confirm compatibility
        if image.depth() != self.src_image.depth() {
            return Err(ImageErrors::GenericStr(
                "Image depths do not match for composite"
            ));
        }

        if image.colorspace() != self.src_image.colorspace() {
            return Err(ImageErrors::GenericString(format!(
                "Image colorspace does not match for composite src image = {:?}, dst_image = {:?}",
                self.src_image.colorspace(),
                image.colorspace()
            )));
        }
        let b_type = image.depth().bit_type();

        match self.composite_method.composite_type() {
            CompositeMethodType::ChannelBased => {
                let colorspace = image.colorspace();
                if colorspace.has_alpha() {
                    for (src_frame, dst_frame) in
                        self.src_image.frames_ref().iter().zip(image.frames_mut())
                    {
                        let all_dst_channels = dst_frame.channels_vec();

                        let (src_color_channels, src_alpha_channel) =
                            src_frame.separate_color_and_alpha_ref(colorspace).unwrap();

                        for (src_chan, d_chan) in src_color_channels.iter().zip(all_dst_channels) {
                            match b_type {
                                BitType::U8 => composite_alpha::<u8>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method
                                ),
                                BitType::U16 => composite_alpha::<u16>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method
                                ),
                                BitType::F32 => composite_alpha::<f32>(
                                    src_chan.reinterpret_as()?,
                                    d_chan.reinterpret_as_mut()?,
                                    src_alpha_channel.reinterpret_as()?,
                                    dims.0,
                                    dims.1,
                                    src_width,
                                    dst_width,
                                    self.composite_method
                                ),
                                d => {
                                    return Err(ImageErrors::ImageOperationNotImplemented(
                                        self.name(),
                                        d
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    for (src_chan, d_chan) in self
                        .src_image
                        .channels_ref(false)
                        .iter()
                        .zip(image.channels_mut(false))
                    {
                        match b_type {
                            BitType::U8 => composite::<u8>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method
                            ),
                            BitType::U16 => composite::<u16>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method
                            ),
                            BitType::F32 => composite::<f32>(
                                src_chan.reinterpret_as()?,
                                d_chan.reinterpret_as_mut()?,
                                dims.0,
                                dims.1,
                                src_width,
                                dst_width,
                                self.composite_method
                            ),
                            d => {
                                return Err(ImageErrors::ImageOperationNotImplemented(
                                    self.name(),
                                    d
                                ));
                            }
                        }
                    }
                }
            }
            CompositeMethodType::AlphaChannel => {
                // we need
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }
}

#[allow(clippy::too_many_arguments)]
fn composite_alpha<T>(
    src: &[T], dest: &mut [T], src_alpha: &[T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize, method: CompositeMethod
) where
    T: Copy + NumOps<T>,
    f32: From<T>
{
    if let CompositeMethod::Over = method {
        composite_over_alpha(
            src, dest, src_alpha, start_x, start_y, width_src, width_dest
        );
    } else {
        unreachable!()
    }
}
fn composite<T: Copy + NumOps<T>>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize,
    method: CompositeMethod
) {
    match method {
        CompositeMethod::Over => composite_over(src, dest, start_x, start_y, width_src, width_dest),
        CompositeMethod::Src => composite_src(src, dest, start_x, start_y, width_src, width_dest),
        CompositeMethod::Dst => (),
        CompositeMethod::DstIn => {
            unreachable!("This should be called  for those that consider alpha")
        }
    }
}

fn composite_src<T: Copy + NumOps<T>>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize
) {
    // fill with max value, this whitens the output
    // or opaques the alpha channel
    dest.fill(T::max_val());
    composite_over(src, dest, start_x, start_y, width_src, width_dest);
}
fn composite_over<T: Copy>(
    src: &[T], dest: &mut [T], start_x: usize, start_y: usize, width_src: usize, width_dest: usize
) {
    //
    for (dst_width, src_width) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
    {
        if let Some(slice) = dst_width.get_mut(start_x..) {
            // get the minimum width to remove out of bounds panics
            let min_width = slice.len().min(src_width.len());
            // then copy
            slice[..min_width].copy_from_slice(&src_width[..min_width]);
        }
    }
}

fn composite_over_alpha<T>(
    src: &[T], dest: &mut [T], src_alpha: &[T], start_x: usize, start_y: usize, width_src: usize,
    width_dest: usize
) where
    T: Copy + NumOps<T>,
    f32: From<T>
{
    let max_v = 1.0 / f32::from(T::max_val());

    for ((dst_width, src_width), src_width_alpha) in dest
        .chunks_exact_mut(width_dest)
        .skip(start_y)
        .zip(src.chunks_exact(width_src))
        .zip(src_alpha.chunks_exact(width_src))
    {
        if let Some(slice) = dst_width.get_mut(start_x..) {
            // get the minimum width to remove out of bounds panics
            let min_width = slice.len().min(src_width.len());

            for ((src_p, src_alpha), dst) in src_width[..min_width]
                .iter()
                .zip(src_width_alpha[..min_width].iter())
                .zip(slice.iter_mut())
            {
                let src_normalized = (f32::from(*src_alpha) * max_v).clamp(0.0, 1.0);
                let dst_alpha = 1.0 - src_normalized;
                *dst = T::from_f32(
                    (src_normalized * f32::from(*src_p)) + (dst_alpha * f32::from(*dst))
                );
            }
        }
    }
}
// #[cfg(test)]
// mod tests {
//     use zune_core::colorspace::ColorSpace;
//     use zune_image::image::Image;
//     use zune_image::traits::OperationsTrait;
//
//     use crate::composite::{Composite, CompositeMethod};
//     use crate::utils::Gravity;
//
//     #[test]
//     fn test_composite_over() {
//         let mut src_image = Image::open("/run/media/caleb/Home/CITAM/logo - Copy.png").unwrap();
//         let mut dst_image = Image::open(
//             "/home/caleb/Pictures/backgrounds/wallpapers/backgrounds/Canazei Granite Ridges.jpg"
//         )
//         .unwrap();
//
//         src_image.convert_color(ColorSpace::RGBA).unwrap();
//         dst_image.convert_color(ColorSpace::RGBA).unwrap();
//
//         // PremultiplyAlpha::new(AlphaState::PreMultiplied)
//         //     .execute(&mut src_image)
//         //     .unwrap();
//
//         let composite = Composite::try_new(
//             &src_image,
//             CompositeMethod::Over,
//             None,
//             Some(Gravity::BottomRight)
//         )
//         .unwrap();
//         composite.execute(&mut dst_image).unwrap();
//         dst_image.save("./composite.jpg").unwrap();
//     }
// }
