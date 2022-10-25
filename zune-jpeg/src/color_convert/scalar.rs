use std::cmp::{max, min};
use std::convert::TryInto;

/// Limit values to 0 and 255
#[inline]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, dead_code)]
fn clamp(a: i16) -> u8
{
    min(max(a, 0), 255) as u8
}

/// YCbCr to RGBA color conversion

pub fn ycbcr_to_rgba_16_scalar(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize,
)
{
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 64 elements for Rust to see we won't go out of bounds.
    let opt: &mut [u8; 64] = output_position
        .get_mut(0..64)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();
    let mut p = 0;
    for (y, (cb, cr)) in y.iter().zip(cb.iter().zip(cr.iter()))
    {
        let cr = cr - 128;

        let cb = cb - 128;

        let r = y + ((45_i16.wrapping_mul(cr)) >> 5);

        let g = y - ((11_i16.wrapping_mul(cb) + 23_i16.wrapping_mul(cr)) >> 5);

        let b = y + ((113_i16.wrapping_mul(cb)) >> 6);

        opt[p] = clamp(r);

        opt[p + 1] = clamp(g);

        opt[p + 2] = clamp(b);

        opt[p + 3] = 255;

        p += 4;
    }
    *pos += 64;
}

pub fn ycbcr_to_rgb_16_scalar(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize,
)
{
    let mut p = 0;
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 48 elements
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    for (y, (cb, cr)) in y.iter().zip(cb.iter().zip(cr.iter()))
    {
        let cr = cr - 128;

        let cb = cb - 128;

        let r = y + ((45_i16.wrapping_mul(cr)) >> 5);

        let g = y - ((11_i16.wrapping_mul(cb) + 23_i16.wrapping_mul(cr)) >> 5);

        let b = y + ((113_i16.wrapping_mul(cb)) >> 6);

        opt[p] = clamp(r);

        opt[p + 1] = clamp(g);

        opt[p + 2] = clamp(b);

        p += 3;
    }

    // Increment pos
    *pos += 48;
}

pub fn ycbcr_to_grayscale(y: &[i16], width: usize, output: &mut [u8])
{
    let width_chunk = ((width + 7) >> 3) * 8;

    for (y_in, out) in y
        .chunks_exact(width_chunk)
        .zip(output.chunks_exact_mut(width))
    {
        for (y, out) in y_in.iter().zip(out.iter_mut())
        {
            *out = *y as u8;
        }
    }
}

/// Convert YCbCr to YCbCr
///
/// Basically all we do is remove fill bytes (if there) in the edges
pub fn ycbcr_to_ycbcr(
    y: &[i16; 16], cb: &[i16; 16], cr: &[i16; 16], output: &mut [u8], pos: &mut usize,
)
{
    // let mut p = 0;
    let (_, output_position) = output.split_at_mut(*pos);

    // Convert into a slice with 48 elements
    let opt: &mut [u8; 48] = output_position
        .get_mut(0..48)
        .expect("Slice to small cannot write")
        .try_into()
        .unwrap();

    for (((y, cb), cr), out) in y
        .iter()
        .zip(cb.iter())
        .zip(cr.iter())
        .zip(opt.chunks_exact_mut(3))
    {
        out[0] = *y as u8;
        out[1] = *cb as u8;
        out[2] = *cr as u8;
    }
}
