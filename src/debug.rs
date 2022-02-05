//! Debug tools for logging and visualizing what is going on.
//!
//! LATER How does this interact with client vs server framerate?
//! LATER Add usage examples

// Implementation note: the macros should be usable
// in expression position, e.g. in match statements - see tests.
// This means they shouldn't end with semicolons
// or should be wrapped with an extra pair of curly braces.
// They should evaluate to `()`.

#![allow(dead_code)]

pub(crate) mod details;

/// Same as `assert!` but only prints a message without crashing.
#[macro_export]
macro_rules! soft_assert {
    // The matchers are the same as in stdlib's assert.
    // The rest is an approximation of the same message format.
    ($cond:expr $(,)?) => {
        soft_assert!($cond, stringify!($cond))
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            // LATER Proper logging
            // LATER client vs server
            dbg_logf!("soft assertion failed: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
        }
    };
}

/// Print text into stdout. Uses `println!(..)`-style formatting.
#[macro_export]
macro_rules! dbg_logf {
    ( $( $t:tt )* ) => {
        {
            $crate::debug::details::DEBUG_ENDPOINT.with(|endpoint|{
                print!("{} ", endpoint.borrow().name);
            });
            println!( $( $t )* );
        }
    };
}

/// Print variables into stdout formatted as `var1: value1, var2: value2`.
#[macro_export]
macro_rules! dbg_logd {
    ( $( $e:expr ),* ) => {
        {
            let s = $crate::__format_pairs!( $( $e ),* );
            dbg_logf!("{}", s);
        }
    };
}

/// Print text onto the screen. Uses `println!(..)`-style formatting.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textf {
    ( ) => {
        dbg_textf!("")
    };
    ( $( $t:tt )* ) => {
        {
            let mut s = String::new();
            $crate::debug::details::DEBUG_ENDPOINT.with(|endpoint|{
                s.push_str(&format!("{} ", endpoint.borrow().name));
            });
            s.push_str(&format!( $( $t )* ));
            $crate::debug::details::DEBUG_TEXTS.with(|texts| {
                texts.borrow_mut().push(s);
            });
        }
    };
}

/// Print variables onto the screen formatted as `var1: value1, var2: value2`.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textd {
    ( $( $e:expr ),* ) => {
        {
            let s = $crate::__format_pairs!( $( $e ),* );
            dbg_textf!("{}", s);
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
        $crate::debug::details::debug_line($begin, $end, $time, $color)
    };
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_line!($begin, $end, $time, $crate::debug::details::default_color())
    };
    ($begin:expr, $end:expr) => {
        $crate::dbg_line!($begin, $end, 0.0)
    };
}

/// Draw an arrow from `begin` to `end` (both world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_arrow {
    ($begin:expr, $end:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_arrow($begin, $end, $time, $color)
    };
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_arrow!($begin, $end, $time, $crate::debug::details::default_color())
    };
    ($begin:expr, $end:expr) => {
        $crate::dbg_arrow!($begin, $end, 0.0)
    };
}

/// Draw a cross at the given world coordinates.
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_cross {
    ($point:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_cross($point, $time, $color)
    };
    ($point:expr, $time:expr) => {
        $crate::dbg_cross!($point, $time, $crate::debug::details::default_color())
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0)
    };
}

/// These only exist for convenience when debugging,
/// normal code should use `Color::YOUR_COLOR_HERE` in full.
/// Also this doesn't follow any standard color naming scheme
/// and can change at any time.
pub(crate) mod colors {
    use fyrox::core::color::Color;

    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const WHITE: Color = Color::WHITE;
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const BLACK: Color = Color::BLACK;
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const RED: Color = Color::RED;
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const GREEN: Color = Color::GREEN;
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const BLUE: Color = Color::BLUE;
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    ///
    /// A blue you can actually see
    pub(crate) const BLUE2: Color = Color::opaque(0, 100, 255);
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const YELLOW: Color = Color::opaque(255, 255, 0);
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const MAGENTA: Color = Color::opaque(255, 0, 255);
    /// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
    pub(crate) const CYAN: Color = Color::opaque(0, 255, 255);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unit_cmp)] // https://github.com/rust-lang/rust-clippy/issues/4661

    // Don't import anything else here to test the macros properly use full paths.
    use crate::prelude::*;

    // LATER Test these do what they should, not just that they compile.
    //          At least check the globals.

    #[test]
    fn test_soft_assert() {
        // Neither should crash
        soft_assert!(2 + 2 == 4);
        soft_assert!(2 + 2 == 5);

        soft_assert!(2 + 2 == 4, "custom message {}", 42);
        soft_assert!(2 + 2 == 5, "custom message {}", 42);

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => soft_assert!(false),
            _ => soft_assert!(false, "custom message {}", 42),
        };
        assert_eq!(nothing, ());
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

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => dbg_logf!(),
            _ => dbg_logf!("abcd"),
            _ => dbg_logf!("x: {}, y: {y}, 7: {}", x, 7),

            _ => dbg_logd!(),
            _ => dbg_logd!(x),
            _ => dbg_logd!(x, y, 7),

            _ => dbg_textf!(),
            _ => dbg_textf!("abcd"),
            _ => dbg_textf!("x: {}, y: {y}, 7: {}", x, 7),

            _ => dbg_textd!(),
            _ => dbg_textd!(x),
            _ => dbg_textd!(x, y, 7),
        };
        assert_eq!(nothing, ());
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

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => dbg_line!(v!(1 2 3), v!(4 5 6)),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5.0),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5.0, Color::BLUE),

            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6)),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0, Color::BLUE),

            _ => dbg_cross!(v!(1 2 3)),
            _ => dbg_cross!(v!(1 2 3), 5.0),
            _ => dbg_cross!(v!(1 2 3), 5.0, Color::BLUE),
        };
        assert_eq!(nothing, ());
    }
}
