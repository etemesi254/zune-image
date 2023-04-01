use crate::decoder::PLTEEntry;
use crate::enums::PngColor;

pub(crate) fn expand_palette(input: &[u8], out: &mut [u8], palette: &[PLTEEntry], components: usize)
{
    if components == 0
    {
        return;
    }

    if components == 3
    {
        for (in_px, px) in input.iter().zip(out.chunks_exact_mut(3))
        {
            let entry = palette[usize::from(*in_px) & 255];

            px[0] = entry.red;
            px[1] = entry.green;
            px[2] = entry.blue;
        }
    }
    else if components == 4
    {
        for (in_px, px) in input.iter().zip(out.chunks_exact_mut(4))
        {
            let entry = palette[usize::from(*in_px) & 255];

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
    input: &[u8], out: &mut [u8], color: PngColor, trns_bytes: [u16; 4], depth: u8
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

                for (in_chunk, chunk) in input.chunks_exact(2).zip(out.chunks_exact_mut(4))
                {
                    chunk[..2].copy_from_slice(in_chunk);

                    if trns_byte != &in_chunk[0..2]
                    {
                        chunk[2] = 255;
                        chunk[3] = 255;
                    }
                    else
                    {
                        chunk[2] = 0;
                        chunk[3] = 0;
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
                    else
                    {
                        chunk[6] = 0;
                        chunk[7] = 0;
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

                for (in_byte, chunk) in input.iter().zip(out.chunks_exact_mut(2))
                {
                    chunk[0] = *in_byte;
                    chunk[1] = u8::from(*in_byte != trns_byte) * 255;
                }
            }
            PngColor::RGB =>
            {
                let depth_mask = (1_u16 << depth) - 1;

                let scale = DEPTH_SCALE_TABLE[usize::from(depth)];

                let r = (trns_bytes[0] & 255 & depth_mask) as u8 * scale;
                let g = (trns_bytes[1] & 255 & depth_mask) as u8 * scale;
                let b = (trns_bytes[2] & 255 & depth_mask) as u8 * scale;

                let r_matrix = [r, g, b];

                for (in_chunk, chunk) in input.chunks_exact(3).zip(out.chunks_exact_mut(4))
                {
                    let mask = &in_chunk[0..3] != &r_matrix;

                    chunk[0..3].copy_from_slice(in_chunk);
                    chunk[3] = 255 * u8::from(mask);
                }
            }
            _ => unreachable!()
        }
    }
}

/// Expand bits to bytes expand images with less than 8 bpp
pub(crate) fn expand_bits_to_byte(
    width: usize, depth: usize, mut in_offset: usize, out_n: usize, plte_present: bool,
    input: &[u8], out: &mut [u8]
)
{
    const DEPTH_SCALE_TABLE: [u8; 9] = [0, 0xff, 0x55, 0, 0x11, 0, 0, 0, 0x01];

    let mut current = 0;

    let mut scale = DEPTH_SCALE_TABLE[depth];

    // for pLTE chunks with lower bit depths
    // do not scale values just expand.
    // The palette pass will expand values to the right pixels.
    if plte_present
    {
        scale = 1;
    }

    let mut k = width * out_n;

    if depth == 1
    {
        while k >= 8
        {
            let cur: &mut [u8; 8] = out
                .get_mut(current..current + 8)
                .unwrap()
                .try_into()
                .unwrap();

            let in_val = input[in_offset];

            cur[0] = scale * ((in_val >> 7) & 0x01);
            cur[1] = scale * ((in_val >> 6) & 0x01);
            cur[2] = scale * ((in_val >> 5) & 0x01);
            cur[3] = scale * ((in_val >> 4) & 0x01);
            cur[4] = scale * ((in_val >> 3) & 0x01);
            cur[5] = scale * ((in_val >> 2) & 0x01);
            cur[6] = scale * ((in_val >> 1) & 0x01);
            cur[7] = scale * ((in_val) & 0x01);

            in_offset += 1;
            current += 8;

            k -= 8;
        }
        if k > 0
        {
            let in_val = input[in_offset];

            for p in 0..k
            {
                let shift = (7_usize).wrapping_sub(p);
                out[current] = scale * ((in_val >> shift) & 0x01);
                current += 1;
            }
        }
    }
    else if depth == 2
    {
        while k >= 4
        {
            let cur: &mut [u8; 4] = out
                .get_mut(current..current + 4)
                .unwrap()
                .try_into()
                .unwrap();

            let in_val = input[in_offset];

            cur[0] = scale * ((in_val >> 6) & 0x03);
            cur[1] = scale * ((in_val >> 4) & 0x03);
            cur[2] = scale * ((in_val >> 2) & 0x03);
            cur[3] = scale * ((in_val) & 0x03);

            k -= 4;

            in_offset += 1;
            current += 4;
        }
        if k > 0
        {
            let in_val = input[in_offset];

            for p in 0..k
            {
                let shift = (6_usize).wrapping_sub(p * 2);
                out[current] = scale * ((in_val >> shift) & 0x03);
                current += 1;
            }
        }
    }
    else if depth == 4
    {
        while k >= 2
        {
            let cur: &mut [u8; 2] = out
                .get_mut(current..current + 2)
                .unwrap()
                .try_into()
                .unwrap();
            let in_val = input[in_offset];

            cur[0] = scale * ((in_val >> 4) & 0x0f);
            cur[1] = scale * ((in_val) & 0x0f);

            k -= 2;

            in_offset += 1;
            current += 2;
        }

        if k > 0
        {
            let in_val = input[in_offset];

            // leftovers
            for p in 0..k
            {
                let shift = (4_usize).wrapping_sub(p * 4);
                out[current] = scale * ((in_val >> shift) & 0x0f);
                current += 1;
            }
        }
    }
}
