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

    /// Saturating addition.
    ///
    /// Computes self + other, saturating at the relevant high
    /// boundary of the type.
    fn saturating_add(self, other: T) -> T;

    /// Returns `1` representation as type T
    fn one() -> T;
}

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
            fn from_usize(x: usize) -> $int
            {
                x as $int
            }
            #[inline(always)]
            fn saturating_add(self, other: $int) -> $int
            {
                self.saturating_add(other)
            }
        }
    };
}

numops_for_int!(u8);
numops_for_int!(u16);
numops_for_int!(i32);
