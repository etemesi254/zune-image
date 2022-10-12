pub fn flip(in_out_image: &mut [u8])
{
    // this is a fast enough operation but I had some extra time
    //
    // So we use the SIMD within register technique here, i.e we
    // chunk output into 8 values, which fit into a 64 bit register
    // then we do a byte-swap during inner loop
    //
    // The byte-swap does the flip for us, meaning we swap 8 operations per memory read
    //
    // There is no explicit bswap we emulate those with read_be and write_le operations
    // This also extends nicely to auto-vectorization hence no need for sse
    //
    // We can further increase the speed by halving the in array, since we are just moving pixels from one point to another
    //  and this operation is symmetric along a diagonal (the bottom left becomes the top right, bottom right
    // becomes the top left), a flip becomes a byte-swap and write.

    // split into half
    let length = in_out_image.len() / 2;

    let (in_img1, in_img2) = in_out_image.split_at_mut(length);

    // chunk each half into u8's
    let in_img1_chunks = in_img1.chunks_exact_mut(8);
    let in_img2_chunks = in_img2.rchunks_exact_mut(8);

    in_img1_chunks
        .zip(in_img2_chunks)
        .for_each(|(top, bottom)| {
            let top_u64 = u64::from_be_bytes(top.try_into().unwrap());
            let bottom_u64 = u64::from_be_bytes(bottom.try_into().unwrap());

            bottom.copy_from_slice(&top_u64.to_le_bytes());
            top.copy_from_slice(&bottom_u64.to_le_bytes());
        });

    in_img1
        .chunks_exact_mut(8)
        .into_remainder()
        .iter_mut()
        .zip(
            in_img2
                .rchunks_exact_mut(8)
                .into_remainder()
                .iter_mut()
                .rev(),
        )
        .for_each(|(in_dim, out_dim)| {
            std::mem::swap(in_dim, out_dim);
        })
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    use crate::flip::flip;

    #[bench]
    fn flip_scalar(b: &mut test::Bencher)
    {
        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0; dimensions];

        b.iter(|| {
            flip(&mut c1);
        })
    }
}
