use crate::image::MAX_CHANNELS;

#[derive(Clone, Debug, Copy, PartialEq)]
pub struct Color {
    colors: [f32; MAX_CHANNELS],
}

impl Color {
    pub fn black() -> Self {
        Color {
            colors: [0.0, 0.0, 0.0, 1.0],
        }
    }
    pub fn white() -> Self {
        Color {
            colors: [1.0, 1.0, 1.0, 1.0],
        }
    }
}
