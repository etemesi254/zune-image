/*
 * Copyright (c) 2023.
 *
 * This software is free software;
 *
 * You can redistribute it or modify it under terms of the MIT, Apache License or Zlib license
 */

// Re-export macros under nicer names
pub use crate::{
    __debug as debug, __error as error, __info as info, __log_enabled as log_enabled,
    __trace as trace, __warn as warn
};

#[repr(usize)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace
}

//
// log_enabled (unchanged)
//
#[doc(hidden)]
#[macro_export]
macro_rules! __log_enabled {
    ($lvl:expr) => {{
        let _ = $lvl;
        false
    }};
}

//
// ERROR
//
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! __error {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __error {
    ($($arg:tt)+) => {};
}

//
// WARN
//
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! __warn {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __warn {
    ($($arg:tt)+) => {};
}

//
// INFO
//
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! __info {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __info {
    ($($arg:tt)+) => {};
}

//
// DEBUG
//
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! __debug {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __debug {
    ($($arg:tt)+) => {};
}

//
// TRACE
//
#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! __trace {
    ($($arg:tt)+) => {};
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __trace {
    ($($arg:tt)+) => {};
}
