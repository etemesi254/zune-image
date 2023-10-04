#[repr(usize)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_enabled {
    ($lvl:expr) => {{
        let _ = $lvl;
        false
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! __error {
    ($($arg:tt)+) => (
        #[cfg(feature = "std")]
        {
            panic!($($arg)+);
        }
    )
}

#[doc(hidden)]
#[macro_export]
macro_rules! __warn {
    ($($arg:tt)+) => (
        #[cfg(feature = "std")]
        {
            panic!($($arg)+);
        }
    )
}

#[doc(hidden)]
#[macro_export]
macro_rules! __info {
    ($($arg:tt)+) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __debug {
    ($($arg:tt)+) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __trace {
    ($($arg:tt)+) => {};
}

// #[macro_export] is required to make macros works across crates
// but it always put the macro in the crate root.
// #[doc(hidden)] + "pub use" is a workaround to namespace a macro.
pub use crate::__error as error;
pub use crate::__warn as warn;
pub use crate::__info as info;
pub use crate::__debug as debug;
pub use crate::__trace as trace;
pub use crate::__log_enabled as log_enabled;
