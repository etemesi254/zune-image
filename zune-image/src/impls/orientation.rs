use zune_core::bit_depth::BitType;

use crate::errors::ImageErrors;
use crate::image::Image;
use crate::impls::flip::Flip;
use crate::impls::flop::Flop;
use crate::traits::OperationsTrait;

pub enum OrientationType
{
    DoNothing = 1,
    FlipHorizontally = 2,
    Rotate180 = 3,
    FlipVertically = 4
}

pub struct AutoOrient
{
    orientation_type: OrientationType
}

impl OperationsTrait for AutoOrient
{
    fn get_name(&self) -> &'static str
    {
        "Auto orient"
    }

    fn execute_impl(&self, image: &mut Image) -> Result<(), ImageErrors>
    {
        match self.orientation_type
        {
            OrientationType::DoNothing =>
            {}
            OrientationType::FlipHorizontally =>
            {
                Flop::new().execute(image)?;
            }
            OrientationType::Rotate180 =>
            {}
            OrientationType::FlipVertically =>
            {
                Flip::new().execute(image)?;
            }
        }
        // check if we have exif orientation metadata and transform it
        // to be this orientation
        #[cfg(feature = "metadata")]
        {
            use exif::{Tag, Value};

            if let Some(data) = &mut image.metadata.exif
            {
                for field in data
                {
                    // set orientation to do nothing
                    if field.tag == Tag::Orientation
                    {
                        field.value = Value::Byte(vec![1]);
                    }
                }
            }
        }
        Ok(())
    }

    fn supported_types(&self) -> &'static [BitType]
    {
        &[BitType::U16, BitType::U8]
    }
}
