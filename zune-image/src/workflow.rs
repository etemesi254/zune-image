use std::time::Instant;

use log::Level::Info;
use log::{info, log_enabled, Level};

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::{DecoderTrait, EncoderTrait, OperationsTrait};

#[derive(Copy, Clone, Debug)]
enum WorkFlowState
{
    Initialized,
    Decode,
    Operations,
    Encode,
    Finished
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
            WorkFlowState::Finished => None
        }
    }
}

pub struct EncodeResult
{
    pub(crate) format: ImageFormat,
    pub(crate) data:   Vec<u8>
}

impl EncodeResult
{
    pub fn get_data(&self) -> &[u8]
    {
        &self.data
    }
    pub fn get_format(&self) -> ImageFormat
    {
        self.format
    }
}
/// Workflow, batch image processing
///
/// A workflow provides an idiomatic way to do batch image processing
/// it can load multiple images (by queing decoders) and batch apply an operation
/// to all the images and then encode images to a specified format.
///
pub struct WorkFlow<'a>
{
    state:         Option<WorkFlowState>,
    decode:        Option<Box<dyn DecoderTrait<'a> + 'a>>,
    image:         Vec<Image>,
    operations:    Vec<Box<dyn OperationsTrait>>,
    encode:        Vec<Box<dyn EncoderTrait + 'a>>,
    encode_result: Vec<EncodeResult>
}

impl<'a> WorkFlow<'a>
{
    /// Create a new workflow that encapsulates a
    #[allow(clippy::new_without_default)]
    pub fn new() -> WorkFlow<'a>
    {
        WorkFlow {
            image:         vec![],
            state:         Some(WorkFlowState::Initialized),
            decode:        None,
            operations:    vec![],
            encode:        vec![],
            encode_result: vec![]
        }
    }
    /// Add a single encoder for this image
    ///
    /// One can define multiple encoders for a single decoder
    /// the workflow will run all encoders in order of definition
    ///
    /// # Example
    /// ```no_run
    /// #[cfg(feature = "ppm")]
    /// {
    ///     use std::fs::File;
    ///     use std::io::BufWriter;
    ///     use zune_image::codecs::ppm::PPMEncoder;
    ///     use zune_image::workflow::WorkFlow;
    ///     let mut buf = BufWriter::new(File::open(".").unwrap());
    ///     let encoder = PPMEncoder::new();    
    ///     let x= WorkFlow::new().add_encoder(Box::new(encoder));
    /// }
    /// #[cfg(not(feature="ppm"))]
    /// {
    /// // nothing
    /// }
    /// ```
    pub fn add_encoder(&mut self, encoder: Box<dyn EncoderTrait + 'a>)
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
        self.image.push(image);
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
    /// ```no_run
    /// #
    /// use zune_image::impls::grayscale::RgbToGrayScale;
    /// use zune_image::impls::transpose::Transpose;
    /// use zune_image::workflow::WorkFlow;
    ///
    ///
    /// let image = WorkFlow::new()
    ///     .chain_operations(Box::new(RgbToGrayScale::new()))
    ///     .chain_operations(Box::new(Transpose::new()))    
    ///     .advance_to_end();
    /// ```
    pub fn chain_operations(&mut self, operations: Box<dyn OperationsTrait>) -> &mut WorkFlow<'a>
    {
        self.operations.push(operations);
        self
    }
    pub fn get_images(&self) -> &[Image]
    {
        self.image.as_ref()
    }
    /// Return all images in the workflow as mutable references
    pub fn get_image_mut(&mut self) -> &mut [Image]
    {
        self.image.as_mut()
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
    pub fn advance(&mut self) -> Result<(), ImageErrors>
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
                        if self.image.is_empty()
                        {
                            info!("Image already present, no need to decode");
                            // move to the next state
                            self.state = state.next();

                            return Ok(());
                        }
                        return Err(ImageErrors::NoImageForOperations);
                    }

                    let decode_op = self.decode.as_mut().unwrap();

                    let img = decode_op.decode()?;

                    self.image.push(img);

                    let stop = Instant::now();

                    self.state = state.next();

                    info!("Finished decoding in {} ms", (stop - start).as_millis());
                }
                WorkFlowState::Operations =>
                {
                    if self.image.is_empty()
                    {
                        return Err(ImageErrors::NoImageForOperations);
                    }

                    for image in self.image.iter_mut()
                    {
                        for operation in &self.operations
                        {
                            let operation_name = operation.get_name();

                            info!("Running {}", operation_name);

                            let start = Instant::now();

                            operation.execute(image)?;

                            let stop = Instant::now();

                            info!(
                                "Finished running `{operation_name}` in {} ms",
                                (stop - start).as_millis()
                            );
                        }
                        self.state = state.next();
                    }
                }
                WorkFlowState::Encode =>
                {
                    if self.image.is_empty()
                    {
                        return Err(ImageErrors::NoImageForOperations);
                    }
                    for image in self.image.iter()
                    {
                        for encoder in self.encode.iter_mut()
                        {
                            let encoder_name = encoder.get_name();

                            info!("Running {}", encoder_name);

                            let start = Instant::now();

                            let result = encoder.encode_to_result(image)?;
                            self.encode_result.push(result);
                            let stop = Instant::now();

                            info!(
                                "Finished running `{encoder_name}` in {} ms",
                                (stop - start).as_millis()
                            );
                            if log_enabled!(Level::Info)
                            {
                                eprintln!();
                            }
                        }
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
    pub fn advance_to_end(&mut self) -> Result<(), ImageErrors>
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
    pub fn get_results(&self) -> &[EncodeResult]
    {
        &self.encode_result
    }
}
