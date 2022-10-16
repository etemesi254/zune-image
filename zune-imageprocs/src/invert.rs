pub fn invert(in_matrix: &mut [u8])
{
    in_matrix.iter_mut().for_each(|x| *x = u8::MAX - *x);
}
