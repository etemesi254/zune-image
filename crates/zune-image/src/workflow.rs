use std::time::Instant;

use log::Level::Info;
use log::{info, log_enabled};

use crate::errors::ImgErrors;
use crate::image::{Image, ImageChannels};
use crate::traits::{DecoderTrait, EncoderTrait, OperationsTrait};

#[derive(Copy, Clone, Debug)]
enum WorkFlowState
{
    Initialized,
    Decode,
    Operations,
    Encode,
    Finished,
}
impl WorkFlowState
{
    pub fn next(self) -> Option<Self>
    {
        match self
        {
            WorkFlowState::Initialized => Some(WorkFlowState::Decode),
            WorkFlowState::Decode => Some(WorkFlowState::Operations),
            WorkFlowState::Operations => Some(WorkFlowState::Encode),
            WorkFlowState::Encode => Some(WorkFlowState::Finished),
            WorkFlowState::Finished => None,
        }
    }
}
pub struct WorkFlow<'a>
{
    state:      Option<WorkFlowState>,
    decode:     Option<Box<dyn DecoderTrait<'a> + 'a>>,
    image:      Option<Image>,
    operations: Vec<Box<dyn OperationsTrait>>,
    encode:     Vec<Box<dyn EncoderTrait>>,
}

impl<'a> WorkFlow<'a>
{
    /// Create a new workflow that encapsulates a
    #[allow(clippy::new_without_default)]
    pub fn new() -> WorkFlow<'a>
    {
        WorkFlow {
            image:      None,
            state:      Some(WorkFlowState::Initialized),
            decode:     None,
            operations: vec![],
            encode:     vec![],
        }
    }
    /// Add a single encoder for this image
    ///
    /// One can define multiple encoders for a single decoder
    /// the workflow will run all encoders in order of definition
    ///
    /// # Example
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::BufWriter;
    /// use zune_image::codecs::ppm::SPPMEncoder;
    /// let buf = BufWriter::new(File::open(".").unwrap());
    /// let encoder = SPPMEncoder::new(buf);
    /// let decoder = zune_jpeg::JpegDecoder::new(&[0xff,0xd8]);
    /// ```
    pub fn add_encoder(&mut self, encoder: Box<dyn EncoderTrait>)
    {
        self.encode.push(encoder);
    }
    /// Add a single decoder for this image
    pub fn add_decoder(&mut self, decoder: Box<dyn DecoderTrait<'a> + 'a>)
    {
        self.decode = Some(decoder);
    }

    pub fn add_operation(&mut self, operations: Box<dyn OperationsTrait>)
    {
        self.operations.push(operations);
    }
    /// Add an image to this chain.
    pub fn chain_image(&mut self, image: Image)
    {
        self.image = Some(image);
    }

    pub fn chain_encoder(&mut self, encoder: Box<dyn EncoderTrait>) -> &mut WorkFlow<'a>
    {
        self.encode.push(encoder);
        self
    }
    pub fn chain_decoder(&mut self, decoder: Box<dyn DecoderTrait<'a> + 'a>) -> &mut WorkFlow<'a>
    {
        self.decode = Some(decoder);
        self
    }
    /// Add a new operation to the workflow.
    ///
    /// This is used as a way to chain multiple operations in a builder
    /// pattern style
    ///
    /// # Example
    /// - This operation will decode a jpeg image pointed by buf,
    /// which is added to the workflow via add_buffer, then
    /// 1. It de-interleaves the image channels, separating them into
    /// separate RGB channels
    /// 2. Convert RGB data to grayscale
    /// 3. Transpose the image channels   
    /// ```
    /// use zune_image::codecs::ppm::SPPMEncoder;
    /// use zune_image::impls::deinterleave::DeInterleaveChannels;
    /// use zune_image::impls::grayscale::RgbToGrayScale;
    /// use zune_image::impls::transpose::Transpose;
    /// use zune_image::workflow::WorkFlow;
    ///
    ///
    /// use zune_jpeg::JpegDecoder;
    ///
    /// let buf = [0xff,0xd8];
    ///
    /// let decoder = JpegDecoder::new(&buf);
    /// let image = WorkFlow::new()
    ///     .add_buffer(&buf)
    ///     .chain_decoder(Box::new(decoder))
    ///     .chain_operations(Box::new(DeInterleaveChannels::new()))
    ///     .chain_operations(Box::new(RgbToGrayScale::new()))
    ///     .chain_operations(Box::new(Transpose::new()))
    ///     .advance_to_end();
    /// ```
    pub fn chain_operations(&mut self, operations: Box<dyn OperationsTrait>) -> &mut WorkFlow<'a>
    {
        self.operations.push(operations);
        self
    }
    pub fn get_image(&self) -> Option<&Image>
    {
        self.image.as_ref()
    }
    /// Advance the workflow one state forward
    ///
    /// The workflow advance is as follows
    ///
    /// 1. Decode
    /// 2. One or more operations [ all ran at once]
    /// 3. One or more encodes [all ran at once]
    /// 4. Finish
    ///
    /// Calling `Workflow::advance()` will run one of this operation
    pub fn advance(&mut self) -> Result<(), ImgErrors>
    {
        if let Some(state) = self.state
        {
            if log_enabled!(Info)
            {
                println!();
                info!("Current state: {:?}\n", state);
            }

            match state
            {
                WorkFlowState::Decode =>
                {
                    let start = Instant::now();
                    // do the actual decode
                    if self.decode.is_none()
                    {
                        // we have an image, no need to decode a new one
                        if self.image.is_some()
                        {
                            info!("Image already present, no need to decode");
                            // move to the next state
                            self.state = state.next();

                            return Ok(());
                        }
                        return Err(ImgErrors::NoImageForOperations);
                    }

                    let decode_op = self.decode.as_mut().unwrap();

                    let decode_buf = decode_op.decode_buffer()?;
                    let colorspace = decode_op.get_out_colorspace();

                    let pixels = {
                        // One channel we don't need to deinterleave
                        if colorspace.num_components() == 1
                        {
                            ImageChannels::OneChannel(decode_buf)
                        }
                        else
                        {
                            ImageChannels::Interleaved(decode_buf)
                        }
                    };

                    let (width, height) = decode_op.get_dimensions().unwrap();

                    let mut image = Image::new();

                    image.set_dimensions(width, height);
                    image.set_image_channel(pixels);
                    image.set_colorspace(colorspace);

                    self.image = Some(image);

                    let stop = Instant::now();

                    self.state = state.next();

                    info!("Finished decoding in {} ms", (stop - start).as_millis());
                }
                WorkFlowState::Operations =>
                {
                    if let Some(image) = &mut self.image
                    {
                        for operation in &self.operations
                        {
                            let operation_name = operation.get_name();

                            info!("Running {}", operation_name);

                            let start = Instant::now();

                            operation.execute_simple(image)?;

                            let stop = Instant::now();

                            info!(
                                "Finished running `{operation_name}` in {} ms",
                                (stop - start).as_millis()
                            );
                        }
                        self.state = state.next();
                    }
                    else
                    {
                        return Err(ImgErrors::NoImageForOperations);
                    }
                }
                WorkFlowState::Encode =>
                {
                    if let Some(image) = self.image.as_ref()
                    {
                        for encoder in self.encode.iter_mut()
                        {
                            let encoder_name = encoder.get_name();

                            info!("Running {}", encoder_name);

                            let start = Instant::now();

                            encoder.encode_to_file(image)?;

                            let stop = Instant::now();

                            info!(
                                "Finished running `{encoder_name}` in {} ms",
                                (stop - start).as_millis()
                            );
                        }
                    }
                    else
                    {
                        return Err(ImgErrors::NoImageForOperations);
                    }
                    self.state = state.next();
                }
                WorkFlowState::Finished =>
                {
                    info!("Finished operations for this workflow");

                    self.state = state.next();
                    return Ok(());
                }
                _ =>
                {
                    self.state = state.next();
                }
            }
        }
        Ok(())
    }
    /// Advance the operations in this workflow up until
    /// we finish.
    ///
    /// This will run a decoder, all operations and all encoders
    /// for this particular workflow
    pub fn advance_to_end(&mut self) -> Result<(), ImgErrors>
    {
        if self.state.is_some()
        {
            while self.state.is_some()
            {
                self.advance()?;
            }
        }
        Ok(())
    }
}
