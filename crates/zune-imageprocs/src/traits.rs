pub trait NumOps<T>
{
    /// Return the maximum value possible for this
    /// type
    fn max_val() -> T;
    /// Return the minimum value possible for this
    /// type
    fn min_val() -> T;

    fn from_u32(x: u32) -> T;

    fn from_f64(x: f64) -> T;

    fn from_f32(x: f32) -> T;

    fn from_usize(x: usize) -> T;

    fn saturating_add(self, other: T) -> T;

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
