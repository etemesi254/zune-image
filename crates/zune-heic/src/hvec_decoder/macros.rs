macro_rules! debug_more {
    // conditional version
    ($cond:expr => $($arg:tt)*) => {
        if DEBUG_MORE && $cond {
            println!(
                "[{}:{}] {}",
                file!(),
                line!(),
                format_args!($($arg)*)
            );
        }
    };

    // normal version
    ($($arg:tt)*) => {
        if DEBUG_MORE {
            println!(
                "[{}:{}] {}",
                file!(),
                line!(),
                format_args!($($arg)*)
            );
        }
    };
}
