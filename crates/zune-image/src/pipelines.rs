/*
 * Copyright (c) 2023.
 *
 * This software is free software; You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */
//! Pipelines, Batch image processing support
//!
#![allow(unused_variables)]
use std::time::Instant;

use zune_core::log::Level::Trace;
use zune_core::log::{log_enabled, trace};

use crate::codecs::ImageFormat;
use crate::errors::ImageErrors;
use crate::image::Image;
use crate::traits::{IntoImage, OperationsTrait};

#[derive(Copy, Clone, Debug)]
enum PipelineState {
    /// Initial state, the struct has been defined
    Initialized,
    /// The pipeline is ready to carry out image decoding of various supported formats
    Decode,
    /// The pipeline is ready to carry out image processing routines
    Operations,
    /// The pipeline is ready to carry out image encoding
    Encode,
    /// The pipeline is done.
    Finished
}

impl PipelineState {
    pub fn next(self) -> Option<Self> {
        match self {
            PipelineState::Initialized => Some(PipelineState::Decode),
            PipelineState::Decode => Some(PipelineState::Operations),
            PipelineState::Operations => Some(PipelineState::Encode),
            PipelineState::Encode => Some(PipelineState::Finished),
            PipelineState::Finished => None
        }
    }
}

/// A struct holding the result of an encode operation
///
/// It contains the image format the data is in
pub struct EncodeResult {
    pub(crate) format: ImageFormat,
    pub(crate) data:   Vec<u8>
}

impl EncodeResult {
    /// Return the raw data of the encoded format
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// Return the format for which the data
    /// of this encode result is stored in
    pub fn format(&self) -> ImageFormat {
        self.format
    }
}

/// Pipeline, batch image processing
///
/// A pipeline provides an idiomatic way to do batch image processing
/// it can load multiple images (by queueing decoders) and batch apply an operation
/// to all the images and then encode images to multiple specified format.
///
/// A pipeline accepts anything that implements [IntoImage](crate::traits::IntoImage) and
/// it has to own the image for the duration of it's lifetime, but can return references to it
/// via  [`images`](crate::pipelines::Pipeline::images) and
///  [`images_mut`](crate::pipelines::Pipeline::images_mut)
pub struct Pipeline<T: IntoImage> {
    state:      Option<PipelineState>,
    decode:     Option<T>,
    image:      Vec<Image>,
    operations: Vec<Box<dyn OperationsTrait>>
}

impl<T> Pipeline<T>
where
    T: IntoImage
{
    /// Create a new pipeline that can be used for default
    #[allow(clippy::new_without_default)]
    pub fn new() -> Pipeline<T> {
        Pipeline {
            image:      vec![],
            state:      Some(PipelineState::Initialized),
            decode:     None,
            operations: vec![]
        }
    }

    /// Add an image to this chain.
    pub fn chain_image(&mut self, image: Image) {
        self.image.push(image);
    }

    /// Override the decoder present in the pipeline with a different
    /// decoder.
    ///
    /// There can only be one decoder in a pipeline, so the last decoder
    /// is the one that will be considered.
    pub fn chain_decoder(&mut self, decoder: T) -> &mut Pipeline<T> {
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
    /// 3. Change the depth to be float32 (f32 in range 0..2)
    /// ```no_run
    /// #
    /// use zune_image::core_filters::colorspace::ColorspaceConv;
    /// use zune_image::image::Image;
    ///
    /// use zune_image::core_filters::depth::Depth;
    /// use zune_image::pipelines::Pipeline;
    ///
    ///
    /// let image = Pipeline::<Image>::new()
    ///     .chain_operations(Box::new(ColorspaceConv::new(zune_core::colorspace::ColorSpace::Luma)))
    ///     .chain_operations(Box::new(Depth::new(zune_core::bit_depth::BitDepth::Float32)))   
    ///     .advance_to_end();
    /// ```
    pub fn chain_operations(&mut self, operations: Box<dyn OperationsTrait>) -> &mut Pipeline<T> {
        self.operations.push(operations);
        self
    }
    pub fn images(&self) -> &[Image] {
        self.image.as_ref()
    }
    /// Return all images in the workflow as mutable references
    pub fn images_mut(&mut self) -> &mut [Image] {
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
    pub fn advance(&mut self) -> Result<(), ImageErrors> {
        if let Some(state) = self.state {
            match state {
                PipelineState::Decode => {
                    let start = Instant::now();
                    // do the actual decode
                    if self.decode.is_none() {
                        // we have an image, no need to decode a new one
                        if self.image.is_empty() {
                            trace!("Image already present, no need to decode");
                            // move to the next state
                            self.state = state.next();

                            return Ok(());
                        }
                        return Err(ImageErrors::NoImageForOperations);
                    }

                    if log_enabled!(Trace) {
                        println!();
                        trace!("Current state: {:?}\n", state);
                    }

                    let decode_op = self.decode.take().unwrap();

                    let img = decode_op.into_image()?;

                    self.image.push(img);

                    let stop = Instant::now();

                    self.state = state.next();

                    trace!("Finished decoding in {} ms", (stop - start).as_millis());
                }
                PipelineState::Operations => {
                    if self.image.is_empty() {
                        return Err(ImageErrors::NoImageForOperations);
                    }

                    if log_enabled!(Trace) && !self.operations.is_empty() {
                        println!();
                        trace!("Current state: {:?}\n", state);
                    }

                    for image in self.image.iter_mut() {
                        for operation in &self.operations {
                            let operation_name = operation.name();

                            trace!("Running {}", operation_name);

                            let start = Instant::now();

                            operation.execute(image)?;

                            let stop = Instant::now();

                            trace!(
                                "Finished running `{operation_name}` in {} ms",
                                (stop - start).as_millis()
                            );
                        }
                        self.state = state.next();
                    }
                }
                PipelineState::Finished => {
                    trace!("Finished operations for this workflow");

                    self.state = state.next();
                    return Ok(());
                }
                _ => {
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
    pub fn advance_to_end(&mut self) -> Result<(), ImageErrors> {
        if self.state.is_some() {
            while self.state.is_some() {
                self.advance()?;
            }
        }
        Ok(())
    }
}
