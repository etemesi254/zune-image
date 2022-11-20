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
        }
    };
}

numops_for_int!(u8);
numops_for_int!(u16);
numops_for_int!(i32);
