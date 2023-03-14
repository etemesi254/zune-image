//! Various operations useful for generic image processing.
//!
//!
//! NB: Everything with an implementation should have `#[inline(always)]` attribute. as
//! this massively speeds up a lot of vectorizable implementations

/// Various number traits useful for generic image
/// processing.
pub trait NumOps<T>
{
    /// Return the maximum value possible for this
    /// type
    fn max_val() -> T;
    /// Return the minimum value possible for this
    /// type
    fn min_val() -> T;

    /// Convert a u8 to type T
    /// using an `as` cast
    fn from_u8(x: u8) -> T;
    /// Convert a u32 to type T
    /// using an `as` cast
    fn from_u32(x: u32) -> T;

    /// Convert an f64 to type T
    /// using an `as` cast
    fn from_f64(x: f64) -> T;

    /// Convert an f32 to type T using an
    /// `as` cast
    fn from_f32(x: f32) -> T;

    /// Convert a usize to type T
    /// using an `as` cast
    fn from_usize(x: usize) -> T;

    /// Convert an i32 to type T
    /// using an `as` cast
    fn from_i32(x: i32) -> T;

    /// Saturating addition.
    ///
    /// Computes self + other, saturating at the relevant high
    /// boundary of the type.
    fn saturating_add(self, other: T) -> T;

    /// Saturating subtraction.
    ///
    /// Computes self - other, saturating at the relevant high
    /// boundary of the type.
    fn saturating_sub(self, other: T) -> T;

    /// Returns `1` representation as type T
    fn one() -> T;

    /// Return this as number casted
    /// to usize
    fn to_usize(self) -> usize;

    /// Return this number casted to
    /// f64
    fn to_f64(self) -> f64;
}

/// A trait implemented only for floats
///
/// Can be used to bind operations which require floats to work
pub trait ZFloat {}

impl ZFloat for f32 {}

impl ZFloat for f64 {}

macro_rules! numops_for_int {
    ($int:tt) => {
        impl NumOps<$int> for $int
        {
            #[inline(always)]
            fn max_val() -> $int
            {
                $int::MAX
            }
            #[inline(always)]
            fn min_val() -> $int
            {
                $int::MIN
            }
            #[inline(always)]
            fn from_u32(x: u32) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn from_f64(x: f64) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn from_f32(x: f32) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn one() -> $int
            {
                1 as $int
            }
            #[inline(always)]
            fn from_u8(x: u8) -> $int
            {
                x as $int
            }

            #[inline(always)]
            fn from_usize(x: usize) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn from_i32(x: i32) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn saturating_add(self, other: $int) -> $int
            {
                self.saturating_add(other)
            }
            #[inline(always)]
            fn saturating_sub(self, other: $int) -> $int
            {
                self.saturating_sub(other)
            }
            #[inline(always)]
            fn to_usize(self) -> usize
            {
                self as usize
            }
            #[inline(always)]
            fn to_f64(self) -> f64
            {
                self as f64
            }
        }
    };
}

numops_for_int!(u8);
numops_for_int!(u16);
numops_for_int!(i32);
