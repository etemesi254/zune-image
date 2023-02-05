use zune_core::bit_depth::BitType;
use zune_imageprocs::resize::resize;
pub use zune_imageprocs::resize::ResizeMethod;

use crate::channel::Channel;
use crate::errors::ImgOperationsErrors;
use crate::image::Image;
use crate::traits::OperationsTrait;

#[derive(Copy, Clone)]
pub struct Resize
{
    new_width:  usize,
    new_height: usize,
    method:     ResizeMethod
}

impl Resize
{
    pub fn new(new_width: usize, new_height: usize, method: ResizeMethod) -> Resize
    {
        Resize {
            new_height,
            new_width,
            method
        }
    }
}

impl OperationsTrait for Resize
{
    fn get_name(&self) -> &'static str
    {
        "Resize"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImgOperationsErrors>
    {
        let (old_w, old_h) = image.get_dimensions();
        let depth = image.get_depth().bit_type();

        let new_length = self.new_width * self.new_height * image.get_depth().size_of();

        match depth
        {
            BitType::Eight =>
            {
                for old_channel in image.get_channels_mut(false)
                {
                    let mut new_channel = Channel::new_with_length(new_length);

                    resize::<u8>(
                        old_channel.reinterpret_as().unwrap(),
                        new_channel.reinterpret_as_mut().unwrap(),
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    );
                    *old_channel = new_channel;
                }
            }
            BitType::Sixteen =>
            {
                for old_channel in image.get_channels_mut(true)
                {
                    let mut new_channel = Channel::new_with_length(new_length);

                    resize::<u16>(
                        old_channel.reinterpret_as().unwrap(),
                        new_channel.reinterpret_as_mut().unwrap(),
                        self.method,
                        old_w,
                        old_h,
                        self.new_width,
                        self.new_height
                    );
                    *old_channel = new_channel;
                }
            }
            _ => todo!()
        }
        image.set_dimensions(self.new_width, self.new_height);

        Ok(())
    }
    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::Eight, BitType::Sixteen]
    }
}
