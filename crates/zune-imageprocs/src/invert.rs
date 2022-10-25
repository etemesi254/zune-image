///Invert a pixel
///
/// The formula for inverting a 8 bit pixel
///  is `pixel[x,y] = 255-pixel[x,y]`
pub fn invert(in_image: &mut [u8])
{
    in_image.iter_mut().for_each(|x| *x = u8::MAX - *x);
}
