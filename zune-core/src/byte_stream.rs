#[cfg(feature = "byte_stream")]
static ERROR_MSG: &str = "No more bytes";

/// An encapsulation of a byestream
///
/// The lifetime parameter is from the stream which we
pub struct ZByteStream<'a>
{
    /// Data stream
    stream:   &'a [u8],
    position: usize,
}
enum Mode
{
    // Big endian
    BE,
    // Little Endian
    LE,
}

impl<'a> ZByteStream<'a>
{
    pub fn new(buf: &'a [u8]) -> ZByteStream<'a>
    {
        ZByteStream {
            stream:   buf,
            position: 0,
        }
    }
    /// Skip `n` bytes ahead of the stream.
    pub fn skip(&mut self, bytes: usize)
    {
        // Can this overflow ??
        self.position = self.position.wrapping_add(bytes);
    }

    pub const fn get_bytes_left(&self) -> usize
    {
        // Must be saturating to prevent underflow
        self.stream.len().saturating_sub(self.position)
    }
    pub const fn len(&self) -> usize
    {
        self.stream.len()
    }
    pub const fn get_position(&self) -> usize
    {
        self.position
    }
}

macro_rules! get_single_type {
    ($name:tt,$name2:tt,$name3:tt,$name4:tt,$name5:tt,$name6:tt,$int_type:tt) => {
        impl<'a> ZByteStream<'a>
        {
            #[inline(always)]
            fn $name(&mut self, mode: Mode) -> $int_type
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => $int_type::from_le_bytes(space),
                            Mode::BE => $int_type::from_be_bytes(space),
                        }
                    }
                    None => 0,
                }
            }

            #[inline(always)]
            fn $name2(&mut self, mode: Mode) -> Result<$int_type, &'static str>
            {
                const SIZE_OF_VAL: usize = core::mem::size_of::<$int_type>();

                let mut space = [0; SIZE_OF_VAL];

                match self.stream.get(self.position..self.position + SIZE_OF_VAL)
                {
                    Some(position) =>
                    {
                        space.copy_from_slice(position);
                        self.position += SIZE_OF_VAL;

                        match mode
                        {
                            Mode::LE => Ok($int_type::from_le_bytes(space)),
                            Mode::BE => Ok($int_type::from_be_bytes(space)),
                        }
                    }
                    None => Err(ERROR_MSG),
                }
            }

            pub fn $name3(&mut self) -> Result<$int_type, &'static str>
            {
                self.$name2(Mode::BE)
            }

            pub fn $name4(&mut self) -> Result<$int_type, &'static str>
            {
                self.$name2(Mode::LE)
            }
            pub fn $name5(&mut self) -> $int_type
            {
                self.$name(Mode::BE)
            }

            pub fn $name6(&mut self) -> $int_type
            {
                self.$name(Mode::LE)
            }
        }
    };
}
get_single_type!(
    get_u8_inner_or_default,
    get_u8_inner_or_die,
    get_u8_be_err,
    get_u8_le_err,
    get_u8_be,
    get_u8_le,
    u8
);
get_single_type!(
    get_u16_inner_or_default,
    get_u16_inner_or_die,
    get_u16_be_err,
    get_u16_le_err,
    get_u16_be,
    get_u16_le,
    u16
);
get_single_type!(
    get_u32_inner_or_default,
    get_u32_inner_or_die,
    get_u32_be_err,
    get_u32_le_err,
    get_u32_be,
    get_u32_le,
    u32
);
get_single_type!(
    get_u64_inner_or_default,
    get_u64_inner_or_die,
    get_u64_be_err,
    get_u64_le_err,
    get_u64_be,
    get_u64_le,
    u64
);
