#[test]
fn test_transpose_sse_scalar_identical()
{
    use nanorand::Rng;

    use crate::transpose;

    let mut rng = nanorand::WyRand::new();

    let width: usize = 40;
    let height: usize = 24;

    let mut in_matrix: Vec<u8> = vec![0; width as usize * height as usize];
    rng.fill(&mut in_matrix);

    let mut sse_out = vec![90; width * height];
    let mut scalar_out = vec![34; width * height];
    unsafe {
        transpose::sse41::transpose_sse41(&in_matrix, &mut sse_out, width, height);
    }
    transpose::scalar::transpose_scalar(&in_matrix, &mut scalar_out, width, height);
    for (a, b) in scalar_out
        .chunks_exact(height)
        .zip(sse_out.chunks_exact(height))
    {
        assert_eq!(a, b);
    }
}
