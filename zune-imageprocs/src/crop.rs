pub fn crop(
    in_image: &[u16], in_width: usize, out_image: &mut [u16], out_width: usize, out_height: usize,
    x: usize, y: usize,
)
{
    // We can take cropping as a view into a sub-image
    // which means
    //
    //     width ──────────────────────────────►
    // │ ┌─────────────────────────────────────┐
    // │ │                                     │
    // │ │                                     │
    // │ │   (x,y)     out width               │
    // │ │     ┌────────────────────┐          │
    // │ │   o │                    │          │
    // │ │   u │                    │          │
    // │ │   t │                    │          │
    // │ │     │  CROPPED IMAGE     │          │
    // │ │   h │                    │          │
    // │ │   e │                    │          │
    // │ │   i │                    │          │
    // │ │   g │                    │          │
    // │ │   h └────────────────────┘          │
    // │ │   t                                 │
    // │ │                                     │
    // ▼ │                                     │
    //   └─────────────────────────────────────┘
    //
    // So a crop is essentially a weird memory copy starting from
    // (x,y) like a small sub copy !!!
    // That's what we essentialy implement here

    // Specific implementation
    // So because we need to skip from 0--y we can use iter.skip(y) to point at y.
    //
    // Since every iterator is moving a single line per height, we only iterate per
    // out_height number of times, so we can achieve this with a `take` iterators.
    // Rust iterators are fun!!

    if in_width == 0 || out_width == 0
    {
        // these generate panic paths for chunks_exact so just eliminate them
        return;
    }

    for (single_in_width, single_out_width) in in_image
        .chunks_exact(in_width)
        .skip(y)
        .take(out_height)
        .zip(out_image.chunks_exact_mut(out_width))
    {
        single_out_width.copy_from_slice(&single_in_width[x..x + out_width]);
    }
}
