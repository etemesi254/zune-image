use crate::decoder::PLTEEntry;
use crate::enums::PngColor;

pub(crate) fn expand_palette(out: &mut [u8], palette: &[PLTEEntry; 256], components: usize)
{
    if components == 0
    {
        return;
    }

    if components == 3
    {
        for px in out.chunks_exact_mut(3)
        {
            // the & 255 may be removed as the compiler can see u8 can never be
            // above 255, but for safety
            let entry = palette[usize::from(px[0]) & 255];

            px[0] = entry.red;
            px[1] = entry.green;
            px[2] = entry.blue;
        }
    }
    else if components == 4
    {
        for px in out.chunks_exact_mut(4)
        {
            let entry = palette[usize::from(px[0]) & 255];

            px[0] = entry.red;
            px[1] = entry.green;
            px[2] = entry.blue;
            px[3] = entry.alpha;
        }
    }
}

/// Expand an image filling the tRNS chunks
///
/// # Arguments
///
/// * `out`:  The output we are to expand
/// * `color`: Input color space
/// * `trns_bytes`:  The tRNS bytes present for the images
/// * `depth`:  The depth of the image
///
pub fn expand_trns<const SIXTEEN_BITS: bool>(
    out: &mut [u8], color: PngColor, trns_bytes: [u16; 4], depth: u8
)
{
    const DEPTH_SCALE_TABLE: [u8; 9] = [0, 0xff, 0x55, 0, 0x11, 0, 0, 0, 0x01];

    // for images whose color types are not paletted
    // presence of a tRNS chunk indicates that the image
    // has transparency.
    //
    // When the pixel specified  in the tRNS chunk is encountered in the resulting stream,
    // it is to be treated as fully transparent.
    // We indicate that by replacing the pixel with pixel+alpha and setting alpha to be zero;
    // to indicate fully transparent.
    if SIXTEEN_BITS
    {
        match color
        {
            PngColor::Luma =>
            {
                let trns_byte = trns_bytes[0].to_ne_bytes();

                for chunk in out.chunks_exact_mut(4)
                {
                    if trns_byte != &chunk[0..2]
                    {
                        chunk[2] = 255;
                        chunk[3] = 255;
                    }
                }
            }
            PngColor::RGB =>
            {
                let r = trns_bytes[0].to_ne_bytes();
                let g = trns_bytes[1].to_ne_bytes();
                let b = trns_bytes[2].to_ne_bytes();

                // copy all trns chunks into one big vector
                let mut all: [u8; 6] = [0; 6];

                all[0..2].copy_from_slice(&r);
                all[2..4].copy_from_slice(&g);
                all[4..6].copy_from_slice(&b);

                for chunk in out.chunks_exact_mut(8)
                {
                    // the read does not match the bytes
                    // so set it to opaque
                    if all != &chunk[0..6]
                    {
                        chunk[6] = 255;
                        chunk[7] = 255;
                    }
                }
            }
            _ => unreachable!()
        }
    }
    else
    {
        match color
        {
            PngColor::Luma =>
            {
                let scale = DEPTH_SCALE_TABLE[usize::from(depth)];

                let depth_mask = (1_u16 << depth) - 1;
                // BUG: This overflowing is indicative of a wrong tRNS value
                let trns_byte = (((trns_bytes[0]) & 255 & depth_mask) as u8) * scale;

                for chunk in out.chunks_exact_mut(2)
                {
                    chunk[1] = u8::from(chunk[0] != trns_byte) * 255;
                }
            }
            PngColor::RGB =>
            {
                let r = (trns_bytes[0] & 255) as u8;
                let g = (trns_bytes[1] & 255) as u8;
                let b = (trns_bytes[2] & 255) as u8;

                let r_matrix = [r, g, b];

                for chunk in out.chunks_exact_mut(4)
                {
                    let mask = &chunk[0..3] != &r_matrix;

                    chunk[3] = 255 * u8::from(mask);
                }
            }
            _ => unreachable!()
        }
    }
}

#[test]
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[cfg(target_feature = "ssse3")]
fn test_convert_be_to_ne()
{
    let mut a: [u8; 64] = std::array::from_fn(|x| x as u8);
    let mut b = a;
    convert_be_to_ne_scalar(&mut a);
    unsafe {
        convert_be_to_ne_sse4(&mut b);
    }
    assert_eq!(a, b);
}
