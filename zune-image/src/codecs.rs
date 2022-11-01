pub mod jpeg;
pub mod png;
pub mod ppm;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Codecs
{
    Jpeg,
    Png,
}
pub fn guess_format(bytes: &[u8]) -> Option<Codecs>
{
    if let Some(magic) = bytes.get(0..2)
    {
        if magic == (0xffd8_u16).to_be_bytes()
        {
            // jpeg bits
            return Some(Codecs::Jpeg);
        }
    }
    if let Some(magic) = bytes.get(0..8)
    {
        if magic == [137, 80, 78, 71, 13, 10, 26, 10]
        // png signature
        {
            return Some(Codecs::Png);
        }
    }
    None
}
