use std::fmt::{Debug, Formatter};

pub enum PngErrors
{
    BadSignature,
    GenericStatic(&'static str),
    Generic(String),

}
impl Debug for PngErrors
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            Self::BadSignature => writeln!(f, "Bad PNG signature, not a png"),
            Self::GenericStatic(val) => writeln!(f, "{:?}", val),
            Self::Generic(val) => writeln!(f, "{:?}", val),

        }
    }
}
impl From<&'static str> for PngErrors
{
    fn from(val: &'static str) -> Self
    {
        Self::GenericStatic(val)
    }
}

impl From<String> for PngErrors
{
    fn from(val: String) -> Self
    {
        Self::Generic(val)
    }
}