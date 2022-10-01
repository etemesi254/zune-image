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
    buf:        Option<&'a [u8]>,
    state:      Option<WorkFlowState>,
    decode:     Option<Box<dyn DecoderTrait>>,
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
            buf:        None,
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
    /// let decoder = zune_jpeg::JpegDecoder::new();
    /// ```
    pub fn add_encoder(&mut self, encoder: Box<dyn EncoderTrait>)
    {
        self.encode.push(encoder);
    }
    /// Add a single decoder for this image
    pub fn add_decoder(&mut self, decoder: Box<dyn DecoderTrait>)
    {
        self.decode = Some(decoder);
    }
    pub fn add_buffer(&mut self, buffer: &'a [u8])
    {
        self.buf = Some(buffer);
    }
    pub fn add_operation(&mut self, operations: Box<dyn OperationsTrait>)
    {
        self.operations.push(operations);
    }

    pub fn chain_encoder(&mut self, encoder: Box<dyn EncoderTrait>) -> &mut WorkFlow<'a>
    {
        self.encode.push(encoder);
        self
    }
    pub fn chain_decoder(&mut self, decoder: Box<dyn DecoderTrait>) -> &mut WorkFlow<'a>
    {
        self.decode = Some(decoder);
        self
    }
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
                        return Err(ImgErrors::NoImageForOperations);
                    }
                    if self.buf.is_none()
                    {
                        return Err(ImgErrors::NoImageBuffer);
                    }

                    let decode_op = self.decode.as_mut().unwrap();

                    let pixels =
                        ImageChannels::Interleaved(decode_op.decode_buffer(self.buf.unwrap())?);
                    let colorspace = decode_op.get_out_colorspace();
                    let (width, height) = decode_op.get_dimensions().unwrap();

                    let mut image = Image::new();

                    image.set_dimensions(width, height);
                    image.set_image_channel(pixels);
                    image.set_colorspace(colorspace);

                    self.image = Some(image);

                    let stop = Instant::now();
                    info!("Finished decoding in {} ms", (stop - start).as_millis());

                    self.state = state.next();
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
                    let image = self.image.as_ref().unwrap();

                    for encoder in self.encode.iter_mut()
                    {
                        let encoder_name = encoder.get_name();

                        info!("Running {} encoder", encoder_name);

                        let start = Instant::now();

                        encoder.encode_to_file(image)?;

                        let stop = Instant::now();

                        info!(
                            "Finished running `{encoder_name}` in {} ms",
                            (stop - start).as_millis()
                        );
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
