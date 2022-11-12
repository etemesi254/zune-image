/// Convert a u16 to a u8 clamping it at `max` value
///
/// # Arguments
/// - source: Source array.
/// - dest: Destination array
/// - max: Maximum value to clamp values
pub fn to_u8(source: &[u16], dest: &mut [u8], max: u16)
{
    for (src, dst) in source.iter().zip(dest.iter_mut())
    {
        *dst = *src.max(&max) as u8
    }
}

pub fn to_u16(source: &[u8], dest: &mut [u16])
{
    for (src, dst) in source.iter().zip(dest.iter_mut())
    {
        *dst = u16::from(*src);
    }
}
