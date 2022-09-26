use crate::colorspace::ImageColorspace;

pub trait DecoderTrait
{
    fn decode_file(&mut self, file: &str) -> Result<Vec<u8>, crate::errors::ImgDecodeErrors>;

    fn decode_buffer(&mut self, buffer: &[u8]) -> Result<Vec<u8>, crate::errors::ImgDecodeErrors>;

    fn get_dimensions(&self) -> Option<(usize, usize)>;

    fn get_out_colorspace(&self) -> ImageColorspace;
}

impl DecoderTrait for zune_jpeg::Decoder
{
    fn decode_file(&mut self, file: &str) -> Result<Vec<u8>, crate::errors::ImgDecodeErrors>
    {
        self.decode_file(file).map_err(|x| x.into())
    }

    fn decode_buffer(&mut self, buffer: &[u8]) -> Result<Vec<u8>, crate::errors::ImgDecodeErrors>
    {
        self.decode_buffer(buffer).map_err(|x| x.into())
    }

    fn get_dimensions(&self) -> Option<(usize, usize)>
    {
        let width = usize::from(self.width());
        let height = usize::from(self.height());

        Some((width, height))
    }

    fn get_out_colorspace(&self) -> ImageColorspace
    {
        self.get_output_colorspace().into()
    }
}
#[derive(Debug, Default)]
pub struct Img {}

#[derive(Debug, Default)]
pub struct NewImg {}
pub trait OperationsTrait
{
    fn name(&self) -> &'static str;
}

pub trait EncoderTrait {}

//type DeInterleaveFuncSignature = fn(&[u8], (&mut [u8], &mut [u8], &mut [u8]));

pub struct DeInterleave
{
    //operation: DeInterleaveFuncSignature,
    from: Img,
    to:   NewImg,
}
impl DeInterleave
{
    pub fn new() -> DeInterleave
    {
        DeInterleave {
            //operation: zune_imageprocs::deinterleave::de_interleave_3_channels,
            from: Img::default(),
            to:   NewImg::default(),
        }
    }
}
impl OperationsTrait for DeInterleave
{
    fn name(&self) -> &'static str
    {
        return "de-interleave";
    }
}

impl OperationsTrait for NewImg
{
    fn name(&self) -> &'static str
    {
        return "new image";
    }
}
