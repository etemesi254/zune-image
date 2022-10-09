pub fn transpose_scalar(in_matrix: &[u8], out_matrix: &mut [u8], width: usize, height: usize)
{
    let dimensions = width * height;
    assert_eq!(
        in_matrix.len(),
        dimensions,
        "In matrix dimensions do not match width and height"
    );

    assert_eq!(
        out_matrix.len(),
        dimensions,
        "Out matrix dimensions do not match width and height"
    );

    for i in 0..height
    {
        for j in 0..width
        {
            out_matrix[(j * height) + i] = in_matrix[(i * width) + j]
        }
    }
}
