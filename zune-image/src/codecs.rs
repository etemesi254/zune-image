pub mod jpeg;
pub mod ppm;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Codecs
{
    Jpeg,
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
    None
}
