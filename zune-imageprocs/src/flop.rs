pub fn flop(in_out_image: &mut [u8], width: usize)
{
    assert_eq!(
        in_out_image.len() % width,
        0,
        "Width does not evenly divide image"
    );

    for width_chunks in in_out_image.chunks_exact_mut(width)
    {
        let (left_to_right, right_to_left) = width_chunks.split_at_mut(width / 2);

        // iterate and swap
        left_to_right
            .iter_mut()
            .zip(right_to_left.iter_mut().rev())
            .for_each(|(ltr, rtl)| {
                std::mem::swap(ltr, rtl);
            });
    }
}
