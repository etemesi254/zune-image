#[derive(Debug, Copy, Clone)]
pub enum DeflateState
{
    Initialized,
    NewBlock,
    Continue,
}
