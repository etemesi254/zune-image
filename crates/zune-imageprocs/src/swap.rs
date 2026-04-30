use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::OperationsTrait;

/// Swap images in the stack
///
/// This uses [core::slice::swap] so panics if the indices are out of bounds
///
pub struct Swap {
    a: usize,
    b: usize,
}
impl Swap {
    #[must_use] 
    pub fn new(a: usize, b: usize) -> Self {
        Self { a, b }
    }
}

impl OperationsTrait for Swap {
    fn name(&self) -> &'static str {
        "Swap"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr(
            "Swap requires the full stack; call via execute_multiple",
        ))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::F32, BitType::U8, BitType::U16]
    }
    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        images.swap(self.a, self.b);
        Ok(())
    }
}
