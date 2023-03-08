//! Special filters
//!
//!
//! These filter functions can skip some bytes in
//! output in anticipation that a pass will fill it in the future.
//! E.g for paletted images, we skip 3 or 4 bytes(depending if the image has a
//! tRNS chunk) which will be filled by the palette pass and the tRNS pass.
//!
//!
//! These functions are slightly slower than their counterparts since
//! they do more book-keeping
//!
use core::cell::Cell;

use crate::filters::paeth;

pub fn handle_none_special(raw: &[u8], current: &mut [u8], components: usize, out_components: usize)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);
    if out_components == 0
    {
        return;
    }

    if components == 0
    {
        return;
    }

    for (current_chunk, raw_chunk) in current
        .chunks_mut(out_components)
        .zip(raw.chunks_exact(components))
    {
        for (raw_byte, current_byte) in raw_chunk.iter().zip(current_chunk)
        {
            *current_byte = *raw_byte;
        }
    }
}

pub fn handle_up_special(
    raw: &[u8], previous_row: &[u8], current: &mut [u8], components: usize, out_components: usize
)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);
    if out_components == 0
    {
        return;
    }

    if components == 0
    {
        return;
    }

    for ((current_chunk, raw_chunk), previous_chunk) in current
        .chunks_mut(out_components)
        .zip(raw.chunks_exact(components))
        .zip(previous_row.chunks_exact(out_components))
    {
        for ((raw_byte, current_byte), previous_byte) in
            raw_chunk.iter().zip(current_chunk).zip(previous_chunk)
        {
            *current_byte = raw_byte.wrapping_add(*previous_byte);
        }
    }
}

pub fn handle_avg_special(
    raw: &[u8], prev_row: &[u8], current: &mut [u8], components: usize, out_components: usize
)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);

    if raw.len() < components || current.len() < components
    {
        return;
    }
    // handle leftmost bytes explicitly
    for i in 0..components
    {
        current[i] = raw[i].wrapping_add(prev_row[i] >> 1);
    }

    let diff = out_components.saturating_sub(components);
    let current_windows = Cell::from_mut(current).as_slice_of_cells();

    let mut previous = 0;

    for ((current_chunk, raw_chunk), prev_chunk) in current_windows[out_components..]
        .chunks(out_components)
        .zip(raw[components..].chunks_exact(components))
        .zip(prev_row[out_components..].chunks_exact(out_components))
    {
        for ((raw_byte, current_byte), prev_row) in
            raw_chunk.iter().zip(current_chunk).zip(prev_chunk)
        {
            let a = u16::from(current_windows[previous].get());
            let b = u16::from(*prev_row);

            let c = (((a + b) >> 1) & 0xFF) as u8;
            current_byte.set(raw_byte.wrapping_add(c));
            previous += 1;
        }
        previous += diff;
    }
}

pub fn handle_avg_special_first(
    raw: &[u8], current: &mut [u8], components: usize, out_components: usize
)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);

    if raw.len() < components || current.len() < components
    {
        return;
    }
    // handle leftmost byte explicitly
    current[..components].copy_from_slice(&raw[..components]);

    let diff = out_components.saturating_sub(components);
    let current_windows = Cell::from_mut(current).as_slice_of_cells();

    let mut previous = 0;

    for (current_chunk, raw_chunk) in current_windows[out_components..]
        .chunks(out_components)
        .zip(raw[components..].chunks_exact(components))
    {
        for (raw_byte, current_byte) in raw_chunk.iter().zip(current_chunk)
        {
            let avg = current_windows[previous].get() >> 1;
            current_byte.set(raw_byte.wrapping_add(avg));
            previous += 1;
        }
        previous += diff;
    }
}

pub fn handle_sub_special(raw: &[u8], current: &mut [u8], components: usize, out_components: usize)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);

    if raw.len() < components || current.len() < components
    {
        return;
    }
    // handle leftmost byte explicitly
    current[..components].copy_from_slice(&raw[..components]);

    let diff = out_components.saturating_sub(components);
    let current_windows = Cell::from_mut(current).as_slice_of_cells();

    let mut previous = 0;

    for (current_chunk, raw_chunk) in current_windows[out_components..]
        .chunks(out_components)
        .zip(raw[components..].chunks_exact(components))
    {
        for (raw_byte, current_byte) in raw_chunk.iter().zip(current_chunk)
        {
            let a = current_windows[previous].get();
            current_byte.set(raw_byte.wrapping_add(a));
            previous += 1;
        }
        previous += diff;
    }
}

pub fn handle_paeth_special_first(
    raw: &[u8], current: &mut [u8], components: usize, out_components: usize
)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);

    if raw.len() < components || current.len() < components
    {
        return;
    }
    // handle leftmost byte explicitly
    current[..components].copy_from_slice(&raw[..components]);

    let diff = out_components.saturating_sub(components);
    let current_windows = Cell::from_mut(current).as_slice_of_cells();

    let mut previous = 0;

    for (current_chunk, raw_chunk) in current_windows[out_components..]
        .chunks(out_components)
        .zip(raw[components..].chunks_exact(components))
    {
        for (raw_byte, current_byte) in raw_chunk.iter().zip(current_chunk)
        {
            let p = paeth(current_windows[previous].get(), 0, 0);
            current_byte.set(raw_byte.wrapping_add(p));
            previous += 1;
        }
        previous += diff;
    }
}

pub fn handle_paeth_special(
    raw: &[u8], prev_row: &[u8], current: &mut [u8], components: usize, out_components: usize
)
{
    assert!(components <= out_components);
    assert_eq!(raw.len() / components, current.len() / out_components);

    if raw.len() < components || current.len() < components
    {
        return;
    }
    // handle leftmost byte explicitly
    for i in 0..components
    {
        current[i] = raw[i].wrapping_add(paeth(0, prev_row[i], 0));
    }

    let diff = out_components.saturating_sub(components);
    let current_windows = Cell::from_mut(current).as_slice_of_cells();

    let mut previous = 0;
    let mut up_previous = out_components;

    for (current_chunk, raw_chunk) in current_windows[out_components..]
        .chunks(out_components)
        .zip(raw[components..].chunks_exact(components))
    {
        for (raw_byte, current_byte) in raw_chunk.iter().zip(current_chunk)
        {
            let a = current_windows[previous].get();
            let b = prev_row[up_previous];
            let c = prev_row[up_previous - out_components];
            let p = paeth(a, b, c);

            current_byte.set(raw_byte.wrapping_add(p));
            up_previous += 1;
            previous += 1;
        }
        up_previous += diff;
        previous += diff;
    }
}
