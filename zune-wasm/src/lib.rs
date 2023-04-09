use std::ops::{Deref, DerefMut};

use log::{debug, error, info};
use wasm_bindgen::prelude::*;
use zune_core::bit_depth::BitDepth;
// use zune_core::colorspace::ColorSpace;
use zune_image::codecs::ImageFormat;
use zune_image::image::Image;
use zune_image::impls::brighten::Brighten;
use zune_image::impls::contrast::Contrast;
use zune_image::impls::depth::Depth;
use zune_image::impls::gamma::Gamma;
use zune_image::impls::grayscale::RgbToGrayScale;
use zune_image::impls::invert::Invert;
use zune_image::impls::statistics::{StatisticOperations, StatisticsOps};
use zune_image::impls::stretch_contrast::StretchContrast;
use zune_image::impls::threshold::{Threshold, ThresholdMethod};
use zune_image::traits::OperationsTrait;

use crate::enums::{WasmColorspace, WasmImageDecodeFormats};
use crate::utils::set_panic_hook;

mod enums;
mod utils;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet()
{
    //alert("Hello, zune-wasm!");
}

#[wasm_bindgen(start)]
pub fn setup()
{
    wasm_logger::init(wasm_logger::Config::default());
    set_panic_hook();
    print_initial_stats();
}

fn print_initial_stats()
{
    info!("Zune-wasm is live");
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    {
        debug!("Running with SIMD 128 bit support");
    }
    #[cfg(not(all(target_arch = "wasm32", target_feature = "simd128")))]
    {
        debug!("No SIMD 128 bit support :( ");
    }
}

//
// #[wasm_bindgen]
// pub struct WasmImageMetadata
// {
//     width:      usize,
//     height:     usize,
//     depth:      BitDepth,
//     colorspace: ColorSpace
// }
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmImage
{
    image: Image
}

impl Deref for WasmImage
{
    type Target = Image;

    fn deref(&self) -> &Self::Target
    {
        &self.image
    }
}

impl DerefMut for WasmImage
{
    fn deref_mut(&mut self) -> &mut Self::Target
    {
        &mut self.image
    }
}

#[wasm_bindgen]
impl WasmImage
{
    pub fn width(&self) -> usize
    {
        let (width, _) = self.image.get_dimensions();
        width
    }

    pub fn height(&self) -> usize
    {
        let (_, height) = self.image.get_dimensions();
        height
    }

    /// Flatten an image to RGBA layout without considering the colorspace
    ///
    /// # Behaviour
    /// For Luma, it duplicates channel to grayscale
    ///
    pub fn flatten_rgba(&mut self, out_pixel: &mut [u8])
    {
        self.image.flatten_rgba_frames_u8(vec![out_pixel])
    }

    /// Apply a contrast operation to the image
    pub fn stretch_contrast(&mut self, lower: u16, upper: u16)
    {
        let ops = StretchContrast::new(lower, upper);
        self.execute_ops(&ops);
    }

    fn execute_ops(&mut self, ops: &dyn OperationsTrait)
    {
        match ops.execute(&mut self.image)
        {
            Ok(()) =>
            {
                info!("Successfully executed {}", ops.get_name());
            }
            Err(e) =>
            {
                error!("Executing {} failed because of {:?}", ops.get_name(), e);
            }
        }
    }

    /// Apply a brighten operation to the image
    pub fn brighten(&mut self, value: f32)
    {
        let ops = Brighten::new(value);

        self.execute_ops(&ops);
    }
    /// Apply a contrast operation to the image
    pub fn contrast(&mut self, contrast: f32)
    {
        let ops = Contrast::new(contrast);
        self.execute_ops(&ops);
    }

    /// Adjust an image's gama value
    pub fn gamma(&mut self, gamma: f32)
    {
        let ops = Gamma::new(gamma);
        self.execute_ops(&ops);
    }

    /// Invert an image's pixels
    pub fn invert(&mut self)
    {
        let ops = Invert::new();
        self.execute_ops(&ops);
    }

    /// Binarize an image
    pub fn threshold(&mut self, threshold: u16)
    {
        let ops = Threshold::new(threshold, ThresholdMethod::Binary);
        self.execute_ops(&ops);
    }

    /// Convert from RGB to grayscale
    pub fn grayscale(&mut self)
    {
        let ops = RgbToGrayScale::new();
        self.execute_ops(&ops);
    }

    /// Carry out a mean filter on the image
    ///
    /// Execution speed depends on array radius and image size
    pub fn mean_filter(&mut self, radius: usize)
    {
        let ops = StatisticsOps::new(radius, StatisticOperations::Mean);
        self.execute_ops(&ops);
    }

    /// Return the image's colorspace
    pub fn colorspace(&mut self) -> WasmColorspace
    {
        WasmColorspace::from_colorspace(self.image.get_colorspace())
    }
}

/// Decode an image returning the pixels if the image is decodable
/// or none otherwise
#[wasm_bindgen]
pub fn decode(bytes: &[u8]) -> Option<WasmImage>
{
    if let Some(format) = ImageFormat::guess_format(bytes)
    {
        if let Ok(mut decoder) = format.get_decoder(bytes)
        {
            let mut image = decoder.decode().unwrap();

            // WASM works with 8 bit images, so convert this to an 8 biy image
            Depth::new(BitDepth::Eight).execute(&mut image).unwrap();

            return Some(WasmImage { image });
        }
        else
        {
            error!(
                "Could not decode {:?}",
                format.get_decoder(bytes).err().unwrap()
            )
        }
    }
    None
}

/// Guess the image format returning an enum if we know the format
///
/// or None otherwise
#[wasm_bindgen]
pub fn guess_format(bytes: &[u8]) -> Option<WasmImageDecodeFormats>
{
    if let Some(format) = ImageFormat::guess_format(bytes)
    {
        return Some(WasmImageDecodeFormats::from_formats(format));
    }
    None
}
