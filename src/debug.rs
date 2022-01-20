//! debug tools for logging and visualizing what is going on.
//!
//! LATER How does this interact with client vs server framerate?

#![allow(dead_code)]

pub(crate) mod details;

#[macro_export]
macro_rules! soft_assert {
    ($cond:expr $(,)?) => {
        soft_assert!($cond, stringify!($cond));
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            // LATER Proper logging
            // LATER client vs server
            println!("soft assertion failed: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
        }
    };
}

/// Draw a line from `begin` to `end` (both world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_line {
    ($begin:expr, $end:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_line($begin, $end, $time, $color);
    };
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_line!($begin, $end, $time, Color::RED);
    };
    ($begin:expr, $end:expr) => {
        $crate::dbg_line!($begin, $end, 0.0);
    };
}

/// Draw an arrow from `begin` to `end` (both world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_arrow {
    ($begin:expr, $end:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_arrow($begin, $end, $time, $color);
    };
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_arrow!($begin, $end, $time, Color::RED);
    };
    ($begin:expr, $end:expr) => {
        $crate::dbg_arrow!($begin, $end, 0.0);
    };
}

/// Draw a cross at the given world coordinates.
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_cross {
    ($point:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_cross($point, $time, $color);
    };
    ($point:expr, $time:expr) => {
        $crate::dbg_cross!($point, $time, Color::RED);
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0);
    };
}
