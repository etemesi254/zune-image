use log::Level::Info;
use log::{info, log_enabled};
use zune_jpeg::errors::DecodeErrors;

use crate::errors::ImgDecodeErrors;
use crate::traits::{DecoderTrait, EncoderTrait, NewImg, OperationsTrait};
#[derive(Copy, Clone, Debug)]
enum WorkFlowState
{
    Initialized,
    Decode,
    Operations,
    Encode,
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
            WorkFlowState::Encode => None,
        }
    }
}
pub struct WorkFlow<'a>
{
    buf:        Option<&'a [u8]>,
    state:      Option<WorkFlowState>,
    decode:     Box<dyn DecoderTrait>,
    image:      Option<NewImg>,
    operations: Vec<Box<dyn OperationsTrait>>,
    //encode:     Box<dyn EncoderTrait>,
}

impl<'a> WorkFlow<'a>
{
    /// Create a new workflow that encapsulates a
    pub fn new(
        buf: Option<&[u8]>,
        decode: Box<dyn DecoderTrait>,
        operations: Vec<Box<dyn OperationsTrait>>,
        //encode: Box<dyn EncoderTrait>,
    ) -> WorkFlow
    {
        WorkFlow {
            buf,
            image: None,
            state: Some(WorkFlowState::Initialized),
            decode,
            operations,
            //  encode,
        }
    }
    pub fn advance(&mut self) -> Result<(), ImgDecodeErrors>
    {
        if let Some(state) = self.state
        {
            if log_enabled!(Info)
            {
                println!();
            }
            info!("Current state: {:?}\n", state);
            match state
            {
                WorkFlowState::Decode =>
                {
                    self.decode.decode_buffer(self.buf.unwrap())?;
                    self.state = state.next();
                }
                WorkFlowState::Operations =>
                {
                    for operation in &self.operations
                    {
                        info!("Running {}", operation.name());
                    }

                    self.state = state.next();
                }
                _ =>
                {
                    self.state = state.next();
                }
            }
        }
        Ok(())
    }
    pub fn advance_to_end(&mut self) -> Result<(), ImgDecodeErrors>
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
