pub trait NumOps<T>
{
    fn max_val() -> T;
    fn min_val() -> T;
}

macro_rules! numops_for_int {
    ($int:tt) => {
        impl NumOps<$int> for $int
        {
            fn max_val() -> $int
            {
                $int::MAX
            }

            fn min_val() -> $int
            {
                $int::MIN
            }
        }
    };
}

numops_for_int!(u8);
numops_for_int!(u16);

numops_for_int!(i32);
