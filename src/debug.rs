//! Debug tools - soft asserts, logging, visualization in 3D.
//!
//! Usage:
//! - When the these macros are used on the server,
//!   they tell clients what to print or draw (unlike `dbg` or `println`)
//!   to make it easy to debug server-side issues.
//! - Prefer `soft_assert` over `assert` in gamecode.
//! - Use `dbg_log*` instead of `dbg`.
//! - Use `dbg_text*` to print things that happen every frame.
//! - Use `dbg_line`, `dbg_arrow`, `dbg_cross`, `dbg_rot` to draw shapes in 3D space.
//! - If you're testing something that needs to be toggled at runtime,
//!   consider using `cvars.d_dbg*`.
//!
//! # Soft asserts
//!
//! Games shouldn't crash. It's better to have a gamelogic or rendering bug
//! than crash.
//!
//! There's a false dichotomy between fail-fast
//! (what most well-designed languages prefer and encourage nowadays)
//! and silently ignoring errors (what most old or poorly designed languages do).
//! Failing fast makes sense for most applications,
//! otherwise you risk corrupting user-data which is even worse than crashing.
//! Silently ignoring errors also often leads to security vulnerabilities.
//!
//! Consider a third option - logging the error and attempting to continue.
//!
//! A corrupted game state is generally better than no game state.
//! This should, of course, only be used in gamelogic code
//! which is not concerned with security, doesn't save to disk, etc.
//!
//! LATER soft_unwrap
//!
//! LATER Gamecode will be sandboxed using WASM.
//! LATER Offer a way for servers and clients to autoreport errors.
//!
//! LATER How does sending logs from sv to cl interact with cl vs sv framerates?
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
        {
            // Using a temporary variable to avoid triggering clippy::neg_cmp_op_on_partial_ord.
            // Can't use `#[allow(...)]` here because attributes on expressions are unstable.
            // (The `if` can become an expression depending on how the macro is used.)
            // NANs are handled correctly - any comparison with them returns `false`
            // which turns into `true` here and prints the message.
            let tmp = $cond;
            if !tmp {
                // LATER Proper logging
                // LATER client vs server
                dbg_logf!("soft assertion failed: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
            }
        }
    };
}

/// Print text into stdout. Uses `println!(..)`-style formatting.
#[macro_export]
macro_rules! dbg_logf {
    ( $( $t:tt )* ) => {
        {
            let name = $crate::debug::details::endpoint_name();
            print!("{} ", name);
            println!( $( $t )* );
        }
    };
}

/// Print variables into stdout formatted as `[file:line] var1: value1, var2: value2`.
#[macro_export]
macro_rules! dbg_logd {
    ( $( $e:expr ),* ) => {
        {
            let s = $crate::__format_pairs!( $( $e ),* );
            dbg_logf!("[{}:{}] {}", file!(), line!(), s);
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
            let name = $crate::debug::details::endpoint_name();
            let mut s = format!("{} ", name);
            s.push_str(&format!( $( $t )* ));
            $crate::debug::details::DEBUG_TEXTS.with(|texts| {
                texts.borrow_mut().push(s);
            });
        }
    };
}

/// Print variables onto the screen formatted as `[file:line] var1: value1, var2: value2`.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textd {
    ( $( $e:expr ),* ) => {
        {
            let s = $crate::__format_pairs!( $( $e ),* );
            dbg_textf!("[{}:{}] {}", file!(), line!(), s);
        }
    };
}

/// Draw a line from `begin` to `end` (in world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_line {
    ($begin:expr, $end:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_line($begin, $end, $time as f32, $color)
    };
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_line!($begin, $end, $time, $crate::debug::details::endpoint_color())
    };
    ($begin:expr, $end:expr) => {
        $crate::dbg_line!($begin, $end, 0.0)
    };
}

/// Draw an arrow from `begin` to `begin + dir` (in world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_arrow {
    ($begin:expr, $dir:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_arrow($begin, $dir, $time as f32, $color)
    };
    ($begin:expr, $dir:expr, $time:expr) => {
        $crate::dbg_arrow!($begin, $dir, $time, $crate::debug::details::endpoint_color())
    };
    ($begin:expr, $dir:expr) => {
        $crate::dbg_arrow!($begin, $dir, 0.0)
    };
}

/// Draw a cross at the given world coordinates.
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_cross {
    ($point:expr, $time:expr, $color:expr) => {
        $crate::debug::details::debug_cross($point, $time as f32, $color)
    };
    ($point:expr, $time:expr) => {
        $crate::dbg_cross!($point, $time, $crate::debug::details::endpoint_color())
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0)
    };
}

/// Draw RGB basis vectors at `point`, rotated by `rot`.
#[macro_export]
macro_rules! dbg_rot {
    ($point:expr, $rot:expr, $time:expr) => {
        $crate::debug::details::debug_rot($point, $rot, $time as f32)
    };
    ($point:expr, $rot:expr) => {
        $crate::dbg_rot!($point, $rot, 0.0)
    };
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
        #![allow(clippy::let_unit_value)] // We need to test that the macros eval to a ()

        // Identity function which counts how many times it's executed
        // to make sure macros only evaluate each input once.
        let mut execution_count = 0;
        let mut id = |x| {
            execution_count += 1;
            x
        };

        soft_assert!(2 + 2 == id(4));
        soft_assert!(2 + 2 == id(5));

        soft_assert!(2 + 2 == id(4), "custom message {}", 42);
        soft_assert!(2 + 2 == id(5), "custom message {}", 42);

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => soft_assert!(2 + 2 == id(4)),
            _ => soft_assert!(2 + 2 == id(5)),

            _ => soft_assert!(2 + 2 == id(4), "custom message {}", 42),
            _ => soft_assert!(2 + 2 == id(5), "custom message {}", 42),
        };
        assert_eq!(nothing, ());

        assert_eq!(execution_count, 4 + 1); // +1 because only one match arm runs
    }

    #[test]
    fn test_logging_compiles() {
        #![allow(clippy::let_unit_value)] // We need to test that the macros eval to a ()

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
        #![allow(clippy::let_unit_value)] // We need to test that the macros eval to a ()

        dbg_line!(v!(1 2 3), v!(4 5 6));
        dbg_line!(v!(1 2 3), v!(4 5 6), 5);
        dbg_line!(v!(1 2 3), v!(4 5 6), 5.0);
        dbg_line!(v!(1 2 3), v!(4 5 6), 5, BLUE);
        dbg_line!(v!(1 2 3), v!(4 5 6), 5.0, BLUE);

        dbg_arrow!(v!(1 2 3), v!(4 5 6));
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5);
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0);
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5, BLUE);
        dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0, BLUE);

        dbg_cross!(v!(1 2 3));
        dbg_cross!(v!(1 2 3), 5);
        dbg_cross!(v!(1 2 3), 5.0);
        dbg_cross!(v!(1 2 3), 5, BLUE);
        dbg_cross!(v!(1 2 3), 5.0, BLUE);

        let rot = UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3);
        dbg_rot!(v!(1 2 3), rot);
        dbg_rot!(v!(1 2 3), rot, 5.0);

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => dbg_line!(v!(1 2 3), v!(4 5 6)),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5.0),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5, BLUE),
            _ => dbg_line!(v!(1 2 3), v!(4 5 6), 5.0, BLUE),

            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6)),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5, BLUE),
            _ => dbg_arrow!(v!(1 2 3), v!(4 5 6), 5.0, BLUE),

            _ => dbg_cross!(v!(1 2 3)),
            _ => dbg_cross!(v!(1 2 3), 5),
            _ => dbg_cross!(v!(1 2 3), 5.0),
            _ => dbg_cross!(v!(1 2 3), 5, BLUE),
            _ => dbg_cross!(v!(1 2 3), 5.0, BLUE),

            _ => dbg_rot!(v!(1 2 3), rot),
            _ => dbg_rot!(v!(1 2 3), rot, 5.0),
        };
        assert_eq!(nothing, ());
    }
}
