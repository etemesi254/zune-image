/// A lightweight debug printing macro controlled by `DEBUG_MORE`.
///
/// This macro behaves like [`println!`] but only emits output when
/// `DEBUG_MORE` is `true`. It also prepends the file and line number
/// to each message for easier tracing.
///
/// # Features
///
/// - Accepts full `println!`-style formatting
/// - Optional conditional printing per call
/// - Includes `file!()` and `line!()` automatically
/// - Zero-cost when `DEBUG_MORE` is `false` (optimized out in release if const)
///
/// # Examples
///
/// Basic usage:
///
/// ```rust
/// debug_more!("value = {}", 42);
/// ```
///
/// With multiple arguments:
///
/// ```rust
/// let x = 10;
/// let y = 20;
/// debug_more!("x = {}, y = {}", x, y);
/// ```
///
/// Multiline formatting:
///
/// ```rust
/// debug_more!(
///     "coords: x={}, y={}, z={}",
///     1,
///     2,
///     3
/// );
/// ```
///
/// Named formatting:
///
/// ```rust
/// let size = 64;
/// debug_more!("block size: {size}");
/// ```
///
/// Conditional logging:
///
/// ```rust
/// let is_visible = false;
/// debug_more!(is_visible => "this will not print");
/// ```
///
/// Conditional with values:
///
/// ```rust
/// let x = 5;
/// debug_more!(x > 0 => "x is positive: {}", x);
/// ```
///
/// # Output Format
///
/// ```text
/// [src/main.rs:10] value = 42
/// ```
///
/// # Notes
///
/// - Requires a `DEBUG_MORE: bool` in scope
/// - Prefer `const DEBUG_MORE: bool = false;` for zero-cost in release builds
///
#[macro_export]
macro_rules! debug_more {
    ($cond:expr => $($arg:tt)*) => {
        if DEBUG_MORE && $cond {
            println!(
                "{}",
                //file!(),
                //line!(),
                format_args!($($arg)*)
            );
        }
    };
    ($($arg:tt)*) => {
        if DEBUG_MORE {
            println!(
                "{}",
                // file!(),
                // line!(),
                format_args!($($arg)*)
            );
        }
    };
}