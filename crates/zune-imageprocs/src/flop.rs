/// Flop an image
///
///```text
///old image     new image
///┌─────────┐   ┌──────────┐
///│a b c d e│   │e d b c a │
///│f g h i j│   │j i h g f │
///└─────────┘   └──────────┘
///```
///
pub fn flop<T: Copy>(in_out_image: &mut [T], width: usize)
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
        for (ltr, rtl) in left_to_right.iter_mut().zip(right_to_left.iter_mut().rev())
        {
            std::mem::swap(ltr, rtl);
        }
    }
}

#[cfg(all(feature = "benchmarks"))]
#[cfg(test)]
mod benchmarks
{
    extern crate test;

    #[bench]
    fn flop_scalar(b: &mut test::Bencher)
    {
        use crate::flop::flop;

        let width = 800;
        let height = 800;
        let dimensions = width * height;

        let mut c1 = vec![0; dimensions];

        b.iter(|| {
            flop(&mut c1, width);
        });
    }
}
