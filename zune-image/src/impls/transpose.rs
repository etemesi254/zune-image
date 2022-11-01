use zune_imageprocs::transpose::transpose;

use crate::errors::ImgOperationsErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::OperationsTrait;

/// Transpose an image
///
/// This mirrors the image along the image top left to bottom-right
/// diagonal
///
/// Done by swapping X and Y indices of the array representation
#[derive(Default)]
pub struct Transpose;

impl Transpose
{
    pub fn new() -> Transpose
    {
        Transpose::default()
    }
}
impl OperationsTrait for Transpose
{
    fn get_name(&self) -> &'static str
    {
        "Transpose"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        let out_dim = width * height;

        match image.get_channel_mut()
        {
            ImageChannels::OneChannel(data) =>
            {
                let mut out_vec = vec![0; out_dim];
                transpose(data, &mut out_vec, width, height);
                *data = out_vec;
            }
            ImageChannels::TwoChannels(input) =>
            {
                for data in input
                {
                    let mut out_vec = vec![0; out_dim];
                    transpose(data, &mut out_vec, width, height);
                    *data = out_vec;
                }
            }
            ImageChannels::ThreeChannels(input) =>
            {
                for data in input
                {
                    let mut out_vec = vec![0; out_dim];
                    transpose(data, &mut out_vec, width, height);
                    *data = out_vec;
                }
            }
            ImageChannels::FourChannels(input) =>
            {
                for data in input
                {
                    let mut out_vec = vec![0; out_dim];
                    transpose(data, &mut out_vec, width, height);
                    *data = out_vec;
                }
            }
            ImageChannels::Interleaved(_) =>
            {
                return Err(ImgOperationsErrors::Generic(
                    "Cannot transpose Interleaved data, run de-interleaved operation before this",
                ))
            }
            ImageChannels::Uninitialized =>
            {
                return Err(ImgOperationsErrors::Generic(
                    "Cannot transpose uninitialized pixels",
                ))
            }
        }
        image.set_dimensions(height, width);
        Ok(())
    }
}
