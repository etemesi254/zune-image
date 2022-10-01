use zune_core::colorspace::ColorSpace;

use crate::traits::DecoderTrait;

impl DecoderTrait for zune_jpeg::JpegDecoder
{
    fn decode_file(&mut self, file: &str) -> Result<Vec<u8>, crate::errors::ImgErrors>
    {
        self.decode_file(file).map_err(|x| x.into())
    }

    fn decode_buffer(&mut self, buffer: &[u8]) -> Result<Vec<u8>, crate::errors::ImgErrors>
    {
        self.decode_buffer(buffer).map_err(|x| x.into())
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        let width = usize::from(self.width());
        let height = usize::from(self.height());

        Some((width, height))
    }

    fn get_out_colorspace(&self) -> ColorSpace
    {
        self.get_output_colorspace()
    }

    fn get_name(&self) -> &'static str
    {
        "Jpeg decoder"
    }
}
