//! debug tools for logging and visualizing what is going on.
//!
//! LATER How does this interact with client vs server framerate?
//! LATER Add usage examples

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

/// Print text into stdout. Uses `println!(..)`-style formatting.
#[macro_export]
macro_rules! dbg_logf {
    ( $( $t:tt )* ) => {
        $crate::debug::details::DEBUG_ENDPOINT.with(|endpoint|{
            print!("{} ", endpoint.borrow());
        });
        println!( $( $t )* );
    };
}

/// Print variables into stdout formatted as `var1: value1, var2: value2`.
#[macro_export]
macro_rules! dbg_logd {
    ( $( $e:expr ),* ) => {
        let s = $crate::__format_pairs!( $( $e ),* );
        dbg_logf!("{}", s);
    };
}

/// Print text onto the screen. Uses `println!(..)`-style formatting.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textf {
    ( ) => {
        dbg_textf!("");
    };
    ( $( $t:tt )* ) => {
        let s = format!( $( $t )* );
        $crate::debug::details::DEBUG_TEXTS.with(|texts| {
            texts.borrow_mut().push(s)
        });
    };
}

/// Print variables onto the screen formatted as `var1: value1, var2: value2`.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textd {
    ( $( $e:expr ),* ) => {
        let s = $crate::__format_pairs!( $( $e ),* );
        $crate::debug::details::DEBUG_TEXTS.with(|texts| {
            texts.borrow_mut().push(s)
        });
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
        $crate::dbg_line!($begin, $end, $time, rg3d::core::color::Color::RED);
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
        $crate::dbg_arrow!($begin, $end, $time, rg3d::core::color::Color::RED);
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
        $crate::dbg_cross!($point, $time, rg3d::core::color::Color::RED);
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0);
    };
}

#[cfg(test)]
mod tests {
    // Don't import anything else here to test the macros properly use full paths.
    use crate::prelude::*;

    // LATER Test these do what they should, not just that they compile.

    #[test]
    fn test_soft_assert() {
        // Neither should crash
        soft_assert!(2 + 2 == 4);
        soft_assert!(2 + 2 == 5);
    }

    #[test]
    fn test_logging_compiles() {
        let x = 5;
        let y = 6;

        dbg_logf!();
        dbg_logf!("abcd");
        dbg_logf!("x: {}, y: {y}, 7: {}", x, 7);

        dbg_logd!();
        dbg_logd!(x);
        dbg_logd!(x, y, 7);

        dbg_textf!();
        dbg_textf!("abcd");
        dbg_textf!("x: {}, y: {y}, 7: {}", x, 7);

        dbg_textd!();
        dbg_textd!(x);
        dbg_textd!(x, y, 7);
    }

    #[test]
    fn test_drawing_compiles_no_import() {
        dbg_line!(v!(1 2 3), v!(4 5 6));
        dbg_line!(v!(1 2 3), v!(4 5 6), 5.0);
        dbg_line!(v!(1 2 3), v!(4 5 6), 5.0, Color::BLUE);

        dbg_arrow!(v!(1 2 3), v!(4 5 6));
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0);
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0, Color::BLUE);

        dbg_cross!(v!(1 2 3));
        dbg_cross!(v!(1 2 3), 5.0);
        dbg_cross!(v!(1 2 3), 5.0, Color::BLUE);
    }
}
