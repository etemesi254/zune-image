use zune_ppm::zune_core::bytestream::ZCursor;
use zune_ppm::zune_core::result::DecodingResult;
use zune_ppm::PPMDecoder;

#[test]
fn test_pfm_rows_are_flipped() {
    // PFM stores rows bottom to top
    for height in 1..=5 {
        for (header, components) in [("Pf", 1), ("PF", 3)] {
            let width = 2;
            let rows: Vec<Vec<f32>> = (0..height)
                .map(|y| {
                    (0..width * components)
                        .map(|x| (y * 10 + x) as f32)
                        .collect()
                })
                .collect();

            let mut data = format!("{header}\n{width} {height}\n-1.0\n").into_bytes();
            for row in rows.iter().rev() {
                for value in row {
                    data.extend_from_slice(&value.to_le_bytes());
                }
            }
            let pixels = match PPMDecoder::new(ZCursor::new(data)).decode().unwrap() {
                DecodingResult::F32(pixels) => pixels,
                _ => panic!("expected f32 pixels")
            };
            assert_eq!(pixels, rows.concat(), "{header} {width}x{height}");
        }
    }
}
