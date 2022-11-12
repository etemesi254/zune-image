use log::trace;
use zune_imageprocs::box_blur::box_blur;

use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

/// Perform a box blur
///
/// Radius is a measure of how many
/// pixels to include in the box blur.
///
/// The greater the radius, the more pronounced the box blur
#[derive(Default)]
pub struct BoxBlur
{
    radius: usize,
}

impl BoxBlur
{
    pub fn new(radius: usize) -> BoxBlur
    {
        BoxBlur { radius }
    }
}

impl OperationsTrait for BoxBlur
{
    fn get_name(&self) -> &'static str
    {
        "Box blur"
    }

    fn _execute_simple(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (width, height) = image.get_dimensions();

        let channels = image.get_channels_mut(false);

        #[cfg(feature = "threads")]
        {
            trace!("Running box blur in multithreaded mode");
            std::thread::scope(|s| {
                // blur each channel on a separate thread
                for channel in channels
                {
                    s.spawn(|| {
                        let mut out_dim = vec![0; width * height];
                        box_blur(channel, &mut out_dim, width, height, self.radius);
                    });
                }
            });
        }
        #[cfg(not(feature = "threads"))]
        {
            trace!("Running box blur in single threaded mode");

            let mut out_dim = vec![0; width * height];

            for channel in channels
            {
                box_blur(channel, &mut out_dim, width, height, self.radius);
            }
        }

        Ok(())
    }
}
