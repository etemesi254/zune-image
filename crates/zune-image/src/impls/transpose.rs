use zune_imageprocs::transpose::transpose;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
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

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();
        let out_dim = width * height;

        for channel in image.get_channels_mut(true)
        {
            let mut out_vec = vec![0; out_dim];

            transpose(channel, &mut out_vec, width, height);
            *channel = out_vec;
        }

        image.set_dimensions(height, width);

        Ok(())
    }
}
