use std::cell::RefCell;
use zune_core::bit_depth::BitType;
use zune_image::errors::ImageErrors;
use zune_image::image::Image;
use zune_image::traits::{OperationColorValues, OperationsTrait};
use zune_imageprocs::composite::{Composite, CompositeMethod};

/// A custom operation that isolates the overlay image from the rest of the pipeline
/// until it is time to composite.
pub struct WasmOverlayOp {
    overlay: RefCell<Option<Image>>,
    method: CompositeMethod,
    x: usize,
    y: usize,
}

impl WasmOverlayOp {
    pub fn new(overlay: Image, method: CompositeMethod, x: usize, y: usize) -> Self {
        Self {
            overlay: RefCell::new(Some(overlay)),
            method,
            x,
            y,
        }
    }
}

impl OperationsTrait for WasmOverlayOp {
    fn name(&self) -> &'static str {
        "WasmOverlayComposite"
    }

    fn execute_impl(&self, _image: &mut Image) -> Result<(), ImageErrors> {
        Err(ImageErrors::GenericStr("WasmOverlayOp requires multiple images"))
    }

    fn supported_types(&self) -> &'static [BitType] {
        &[BitType::U8, BitType::U16, BitType::F32]
    }

    fn operation_color_values(&self) -> OperationColorValues {
        OperationColorValues::Linear
    }

    fn execute_multiple(&self, images: &mut Vec<Image>) -> Result<(), ImageErrors> {
        // 1. Take the overlay out of the RefCell (consumes it safely)
        let overlay_img = self.overlay
            .borrow_mut()
            .take()
            .ok_or(ImageErrors::GenericStr("Overlay image was already consumed"))?;

        // 2. Push it onto the stack just in time for the native Composite operation
        images.push(overlay_img);

        // 3. Defer to the native Zune Composite operation to handle the stack logic
        let comp = Composite::new(self.method, (self.x, self.y));
        comp.execute_multiple(images)
    }
}