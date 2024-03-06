/// Different GIF disposal methods
#[derive(Copy, Clone, PartialEq)]
pub(crate) enum DisposalMethod {
    None = 0,
    InPlace = 1,
    Background = 2,
    Restore = 3
}

impl DisposalMethod {
    pub fn from_flags(value: u8) -> DisposalMethod {
        match value {
            1 => DisposalMethod::InPlace,
            2 => DisposalMethod::Background,
            3 => DisposalMethod::Restore,
            _ => DisposalMethod::None
        }
    }
}
