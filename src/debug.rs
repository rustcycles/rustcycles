//! Debug tools - soft asserts, logging, visualizations in screen and world space.
//!
//! # Usage
//!
//! - When the these macros are used on the server,
//!   they tell clients what to print or draw (unlike `dbg` or `println`)
//!   to make it easy to debug server-side issues.
//! - Prefer `soft_assert` over `assert` in gamecode.
//! - Use `dbg_log*` instead of `dbg` / `println`.
//! - Use `dbg_text*` to print things that happen every frame on screen.
//! - Use `dbg_line`, `dbg_arrow`, `dbg_cross`, `dbg_rot` to draw shapes in 3D space.
//! - If you're testing something that needs to be toggled at runtime,
//!   consider using `cvars.dbg*`.
//!
//! Note there can be multiple different framerates: e.g. server bookkeeping,
//! client/server gamelogic, client bookkeeping, rendering.
//! These debug tools are mainly meant for use in gamelogic - some get cleared each gamelogic step.
//! This might result in missing or duplicated messages when the framerates are no the same
//! (especially when the game is paused and nothing is cleared).
//! LATER For each message, save which frame type it came from,
//!     only clear it at the start of the same frame type.
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
//! LATER Offer a way for servers and clients to autoreport errors.
//! LATER How does sending logs from sv to cl interact with cl vs sv framerates?
//! LATER Add usage examples

// Implementation note: the macros should be usable
// in expression position, e.g. in match statements - see tests.
// This means they shouldn't end with semicolons
// or should be wrapped with an extra pair of curly braces.
// They should evaluate to `()`.

// This file is shared between RecWars and RustCycles
// to keep their debug APIs the same
// and as an experiment to see how much code is shareable
// between significantly different multiplayer games.

#![allow(dead_code)]

pub mod details;

use std::cell::{Cell, RefCell};

use crate::{
    debug::details::{DebugShape, WorldText},
    prelude::*,
};

/// Print text into stdout. Uses `println!(..)`-style formatting.
#[macro_export]
macro_rules! dbg_logf {
    () => {
        dbg_logf!("")
    };
    ($($t:tt)*) => {{
        let msg = format!($($t)*);
        $crate::__println!("{} {:.04} {}", $crate::debug::endpoint_name(), $crate::debug::game_time(), msg);
    }};
}

/// Print variables into stdout formatted as `[file:line] var1: value1, var2: value2`.
#[macro_export]
macro_rules! dbg_logd {
    ($($e:expr),*) => {{
        let s = $crate::__format_pairs!($($e),*);
        dbg_logf!("[{}:{}] {}", file!(), line!(), s);
    }};
}

/// Print text onto the screen. Uses `println!(..)`-style formatting.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textf {
    () => {
        dbg_textf!("")
    };
    ($($t:tt)*) => {{
        let msg = format!($($t)*);
        let text = format!("{} {}", $crate::debug::endpoint_name(), msg);
        $crate::debug::DEBUG_TEXTS.with(|texts| {
            texts.borrow_mut().push(text);
        });
    }};
}

/// Print variables onto the screen formatted as `[file:line] var1: value1, var2: value2`.
///
/// Useful for printing debug info each frame.
#[macro_export]
macro_rules! dbg_textd {
    ($($e:expr),*) => {{
        let s = $crate::__format_pairs!($($e),*);
        dbg_textf!("[{}:{}] {}", file!(), line!(), s);
    }};
}

/// Print text onto the screen at the given world coordinates.
///
/// Useful for printing debug info next to game entities each frame.
#[macro_export]
macro_rules! dbg_world_textf {
    ($pos:expr, $($t:tt)*) => {{
        let msg = format!($($t)*);
        let text = $crate::debug::details::WorldText::new($pos, msg);
        $crate::debug::DEBUG_TEXTS_WORLD.with(|texts| {
            texts.borrow_mut().push(text);
        });
    }};
}

/// Print variables onto the screen at the given world coordinates formatted as `var1: value1, var2: value2`.
///
/// Useful for printing debug info next to game entities each frame.
#[macro_export]
macro_rules! dbg_world_textd {
    ($pos:expr,$($e:expr),*) => {{
        let s = $crate::__format_pairs!($($e),*);
        dbg_world_textf!($pos, "[{}:{}] {}", file!(), line!(), s);
    }};
}

/// Private helper to print the name and value of each given variable.
/// Not meant to be used directly.
#[macro_export]
macro_rules! __format_pairs {
    () => {
        format!("")
    };
    ($e:expr) => {
        // We use {:?} instead of {} here because it's more likely to stay on one line.
        // E.g. nalgebra vectors get printed as columns when using {}.
        format!("{}: {:.6?}", stringify!($e), $e)
    };
    ($e:expr, $($rest:expr),+) => {
        format!(
            "{}, {}",
            $crate::__format_pairs!($e),
            $crate::__format_pairs!($($rest),+)
        )
    };
}

/// Draw a line from `begin` to `end` (in world coordinates).
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_line {
    ($begin:expr, $end:expr, $time:expr, $color:expr) => {{
        #[allow(trivial_numeric_casts)]
        $crate::debug::details::debug_line($begin, $end, $time as fl, $color);
    }};
    ($begin:expr, $end:expr, $time:expr) => {
        $crate::dbg_line!($begin, $end, $time, $crate::debug::endpoint_color())
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
    ($begin:expr, $dir:expr, $time:expr, $color:expr) => {{
        #[allow(trivial_numeric_casts)]
        $crate::debug::details::debug_arrow($begin, $dir, $time as fl, $color);
    }};
    ($begin:expr, $dir:expr, $time:expr) => {
        $crate::dbg_arrow!($begin, $dir, $time, $crate::debug::endpoint_color())
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
    ($point:expr, $time:expr, $color:expr) => {{
        #[allow(trivial_numeric_casts)]
        $crate::debug::details::debug_cross($point, $time as fl, $color);
    }};
    ($point:expr, $time:expr) => {
        $crate::dbg_cross!($point, $time, $crate::debug::endpoint_color())
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0)
    };
}

/// Draw RGB basis vectors at `point`, rotated by `rot`.
#[macro_export]
macro_rules! dbg_rot {
    ($point:expr, $rot:expr, $time:expr, $scale:expr) => {{
        #[allow(trivial_numeric_casts)]
        $crate::debug::details::debug_rot($point, $rot, $time as fl, $scale as fl);
    }};
    ($point:expr, $rot:expr, $time:expr) => {
        $crate::dbg_rot!($point, $rot, 0.0, 1.0)
    };
    ($point:expr, $rot:expr) => {
        $crate::dbg_rot!($point, $rot, 0.0)
    };
}

/// Same as `assert!` but at compile time.
#[macro_export]
macro_rules! static_assert {
    ($($arg:tt)+) => {
        const _: () = assert!($($arg)+);
    };
}
// There is no corresponding static_assert_{eq,ne} because assert_{eq,ne} call a non-const function.

/// Same as `assert!` but only prints a message without crashing.
#[macro_export]
macro_rules! soft_assert {
    // The matchers are the same as in stdlib's assert.
    ($cond:expr $(,)?) => {
        soft_assert!($cond, stringify!($cond))
    };
    ($cond:expr, $($arg:tt)+) => {
        // Using a match to extend lifetimes, see soft_assert_eq for more details.
        match (&$cond) {
            cond_val => {
                if !*cond_val {
                    dbg_logf!("[ERROR]: soft_assert failed: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
                }
            }
        }
    };
}

/// Same as `assert_eq!` but only prints a message without crashing.
#[macro_export]
macro_rules! soft_assert_eq {
    // The matchers are the same as in stdlib's assert.
    // The message format is to fit on one line.
    // LATER Is one line necessary?
    //  As long as each is prefixes with endpoint and timestamp, it should be ok.
    //  Consider changing format to match stdlib 1.73.0:
    //  https://blog.rust-lang.org/2023/10/05/Rust-1.73.0.html#cleaner-panic-messages
    ($left:expr, $right:expr $(,)?) => {
        soft_assert_eq!($left, $right, "`{} == {}`", stringify!($left), stringify!($right))
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        // This is based on the impl of `assert_eq!` in stdlib.
        // https://stackoverflow.com/questions/48732263/why-is-rusts-assert-eq-implemented-using-a-match/54855986#54855986
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    dbg_logf!("[ERROR]: soft_assert_eq failed: {}, left: {:?}, right {:?}, {}:{}:{}",
                        format!($($arg)+), &*left_val, &*right_val, file!(), line!(), column!()
                    )
                }
            }
        }
    };
}

/// Same as `assert_ne!` but only prints a message without crashing.
#[macro_export]
macro_rules! soft_assert_ne {
    // The matchers are the same as in stdlib's assert.
    // The message format is similar but not identical, we want to fit on one line.
    // Examples from regular asserts:
    // assert_ne!(1, 1);
    // assert_ne!(1, 1, "test");
    // thread 'main' panicked at 'assertion failed: `(left != right)`
    //   left: `1`,
    //  right: `1`', src/main.rs:6:5
    // thread 'main' panicked at 'assertion failed: `(left != right)`
    //   left: `1`,
    //  right: `1`: test', src/main.rs:7:5
    ($left:expr, $right:expr $(,)?) => {
        soft_assert_ne!($left, $right, "`{} != {}`", stringify!($left), stringify!($right))
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val != *right_val) {
                    dbg_logf!("[ERROR]: soft_assert_ne failed: {}, left: {:?}, right {:?}, {}:{}:{}",
                        format!($($arg)+), &*left_val, &*right_val, file!(), line!(), column!()
                    )
                }
            }
        }
    };
}

/// Simiar to `unreachable!` but only prints a message and returns without crashing.
///
/// Control flow must not go past this macro but it must not panic either,
/// so the solution is to return from the function.
/// The value returned is `Default::default()`
/// so it'll only work in functions that return a type implementing `Default`
/// (this includes functions returning `()`).
#[macro_export]
macro_rules! soft_unreachable {
    () => {
        {
            dbg_logf!("[ERROR]: soft_unreachable {}:{}:{}", file!(), line!(), column!());
            return Default::default();
        }
    };
    ($($arg:tt)+) => {
        {
            dbg_logf!("[ERROR]: soft_unreachable: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
            return Default::default();
        }
    };
}

pub trait SoftUnwrap {
    type Inner;

    #[track_caller]
    fn soft_unwrap(self) -> Self::Inner;
}

impl<T: Default> SoftUnwrap for Option<T> {
    type Inner = T;

    fn soft_unwrap(self) -> Self::Inner {
        match self {
            Some(x) => x,
            None => {
                let loc = std::panic::Location::caller();
                dbg_logf!(
                    "[ERROR]: soft_unwrap failed: Option::None, {}:{}:{}",
                    loc.file(),
                    loc.line(),
                    loc.column()
                );
                Default::default()
            }
        }
    }
}

impl<T: Default, E: Debug> SoftUnwrap for Result<T, E> {
    type Inner = T;

    fn soft_unwrap(self) -> Self::Inner {
        match self {
            Ok(x) => x,
            Err(e) => {
                let loc = std::panic::Location::caller();
                dbg_logf!(
                    "[ERROR]: soft_unwrap failed: Result::Err({:?}), {}:{}:{}",
                    e,
                    loc.file(),
                    loc.line(),
                    loc.column()
                );
                Default::default()
            }
        }
    }
}

// LATER soft accessors for Vecs and generational arenas

/// Extension trait for debugging iterators.
///
/// Note that if multiple debug methods are used in a single chain,
/// the order in which they print their results will be reversed.
pub trait DbgIterator: Iterator + Sized {
    // This might not be useful very often,
    // I just wanted to play with iterators instead of writing my game for a bit.
    // But hey, at least I am not writing the 51st Rust game engine.

    /// Count how many times an iterator returned `Some` and dbg_log it.
    ///
    /// # Examples
    /// ```rust
    /// for x in [1, 2, 3].iter().dbg_count_log("element count") {}
    /// ```
    fn dbg_count_log(self, msg: impl AsRef<str>) -> DbgCounter<Self, Box<dyn FnMut(usize, bool)>> {
        let msg = msg.as_ref().to_owned();
        let f = move |cnt, finished| {
            if finished {
                dbg_logf!("{}: {}", msg, cnt);
            } else {
                dbg_logf!("{}: {} (not finished)", msg, cnt);
            }
        };
        DbgCounter {
            iterator: self,
            f: Box::new(f),
            cnt: 0,
            finished: false,
        }
    }

    /// Count how many times an iterator returned `Some` and dbg_text it.
    ///
    /// # Examples
    /// ```rust
    /// for x in [1, 2, 3].iter().dbg_count_text("element count") {}
    /// ```
    fn dbg_count_text(self, msg: impl AsRef<str>) -> DbgCounter<Self, Box<dyn FnMut(usize, bool)>> {
        let msg = msg.as_ref().to_owned();
        let f = move |cnt, finished| {
            if finished {
                dbg_textf!("{}: {}", msg, cnt);
            } else {
                dbg_textf!("{}: {} (not finished)", msg, cnt);
            }
        };
        DbgCounter {
            iterator: self,
            f: Box::new(f),
            cnt: 0,
            finished: false,
        }
    }

    /// Count how many times an iterator returned `Some` and call `f` with the result.
    fn dbg_count<F>(self, f: F) -> DbgCounter<Self, F>
    where
        F: FnMut(usize, bool),
    {
        DbgCounter {
            iterator: self,
            f,
            cnt: 0,
            finished: false,
        }
    }
}

impl<I> DbgIterator for I where I: Iterator {}

pub struct DbgCounter<I, F>
where
    I: Iterator,
    F: FnMut(usize, bool),
{
    iterator: I,
    f: F,
    cnt: usize,
    finished: bool,
}

impl<I, F> Iterator for DbgCounter<I, F>
where
    I: Iterator,
    F: FnMut(usize, bool),
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iterator.next();
        if next.is_some() {
            self.cnt += 1;
        } else {
            self.finished = true;
        }
        next
    }
}

// We could call the function in next() when the iterator returns None
// but that would only work for iterators that are consumed to the end.
// Instead, we call it when dropping so it works for all iterators.
// The downside is that destructors run inside out
// so the last debug function in a chain will print its result first.
impl<I, F> Drop for DbgCounter<I, F>
where
    I: Iterator,
    F: FnMut(usize, bool),
{
    fn drop(&mut self) {
        (self.f)(self.cnt, self.finished);
    }
}

//
//
// =======================================================================
// The public debugging API is above, shared implementation details below.
// =======================================================================
//
//

// LATER(multithreading) Make debug tools work correctly from all threads.
thread_local! {
    // The default value here should be overwritten as soon as it's decided
    // whether the thread is a client or a server. If you see it in stdout/stderr,
    // something is wrong - it's very early in startup or somebody spawned
    // more threads without setting this.
    static DEBUG_ENDPOINT: RefCell<DebugEndpoint> = RefCell::new(DebugEndpoint{
        name: "??cl/sv",
        default_color: WHITE,
    });

    static DEBUG_GAME_TIME: Cell<fl> = const { Cell::new(-1.0) };

    pub static DEBUG_TEXTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    pub static DEBUG_TEXTS_WORLD: RefCell<Vec<WorldText>> = RefCell::new(Vec::new());
    pub static DEBUG_SHAPES: RefCell<Vec<DebugShape>> = RefCell::new(Vec::new());
}

#[derive(Debug, Clone)]
pub struct DebugEndpoint {
    pub name: &'static str,
    pub default_color: Color,
}

pub fn set_endpoint(name: &'static str) {
    DEBUG_ENDPOINT.with(|endpoint| {
        let mut endpoint = endpoint.borrow_mut();
        endpoint.name = name;
        endpoint.default_color = endpoint_to_color(name);
    });
}

fn endpoint_to_color(name: &'static str) -> Color {
    match name {
        "sv" | "losv" => GREEN,
        "cl" | "locl" => RED,
        "lo" => CYAN,
        _ => WHITE,
    }
}

pub fn endpoint_name() -> &'static str {
    DEBUG_ENDPOINT.with(|endpoint| endpoint.borrow().name)
}

pub fn endpoint_color() -> Color {
    DEBUG_ENDPOINT.with(|endpoint| endpoint.borrow().default_color)
}

pub fn set_game_time(time: fl) {
    DEBUG_GAME_TIME.with(|game_time| game_time.set(time));
}

pub fn game_time() -> fl {
    DEBUG_GAME_TIME.with(|game_time| game_time.get())
}

pub fn clear_expired() {
    DEBUG_TEXTS.with(|texts| texts.borrow_mut().clear());
    DEBUG_TEXTS_WORLD.with(|texts| texts.borrow_mut().clear());
    DEBUG_SHAPES.with(|shapes| shapes.borrow_mut().retain(|shape| shape.time > 0.0));
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unit_cmp)] // https://github.com/rust-lang/rust-clippy/issues/4661

    // Don't import anything else here to test the macros properly use full paths.
    use crate::{
        debug::details::{V1, V2},
        prelude::*,
        r1,
    };

    // LATER Test these do what they should, not just that they compile.
    //  At least check the globals. Or better yet, test them by running the game and comparing output.

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

        dbg_world_textf!(V1, "abcd");
        dbg_world_textf!(V1, "x: {}, y: {y}, 7: {}", x, 7);

        dbg_world_textd!(V1, x);
        dbg_world_textd!(V1, x, y, 7);

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

            _ => dbg_world_textf!(V1, "abcd"),
            _ => dbg_world_textf!(V1, "x: {}, y: {y}, 7: {}", x, 7),

            _ => dbg_world_textd!(V1, x),
            _ => dbg_world_textd!(V1, x, y, 7),
        };
        assert_eq!(nothing, ());
    }

    #[test]
    fn test_drawing_compiles() {
        #![allow(clippy::let_unit_value)] // We need to test that the macros eval to a ()

        dbg_line!(V1, V2);
        dbg_line!(V1, V2, 5);
        dbg_line!(V1, V2, 5.0);
        dbg_line!(V1, V2, 5, BLUE);
        dbg_line!(V1, V2, 5.0, BLUE);

        dbg_arrow!(V1, V2);
        dbg_arrow!(V1, V2, 5);
        dbg_arrow!(V1, V2, 5.0);
        dbg_arrow!(V1, V2, 5, BLUE);
        dbg_arrow!(V1, V2, 5.0, BLUE);

        dbg_cross!(V1);
        dbg_cross!(V1, 5);
        dbg_cross!(V1, 5.0);
        dbg_cross!(V1, 5, BLUE);
        dbg_cross!(V1, 5.0, BLUE);

        dbg_rot!(V1, r1!());
        dbg_rot!(V1, r1!(), 5);
        dbg_rot!(V1, r1!(), 5.0);
        dbg_rot!(V1, r1!(), 5, 2);
        dbg_rot!(V1, r1!(), 5, 2.0);
        dbg_rot!(V1, r1!(), 5.0, 2);
        dbg_rot!(V1, r1!(), 5.0, 2.0);

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => dbg_line!(V1, V2),
            _ => dbg_line!(V1, V2, 5),
            _ => dbg_line!(V1, V2, 5.0),
            _ => dbg_line!(V1, V2, 5, BLUE),
            _ => dbg_line!(V1, V2, 5.0, BLUE),

            _ => dbg_arrow!(V1, V2),
            _ => dbg_arrow!(V1, V2, 5),
            _ => dbg_arrow!(V1, V2, 5.0),
            _ => dbg_arrow!(V1, V2, 5, BLUE),
            _ => dbg_arrow!(V1, V2, 5.0, BLUE),

            _ => dbg_cross!(V1),
            _ => dbg_cross!(V1, 5),
            _ => dbg_cross!(V1, 5.0),
            _ => dbg_cross!(V1, 5, BLUE),
            _ => dbg_cross!(V1, 5.0, BLUE),

            _ => dbg_rot!(V1, r1!()),
            _ => dbg_rot!(V1, r1!(), 5.0),
        };
        assert_eq!(nothing, ());
    }

    #[test]
    fn test_static_asserts() {
        static_assert!(2 + 2 == 4);
    }

    #[test]
    fn test_soft_asserts() {
        #![allow(clippy::let_unit_value)] // We need to test that the macros eval to a ()
        #![allow(clippy::nonminimal_bool)]
        #![allow(clippy::redundant_clone)]

        // Identity function which counts how many times it's executed
        // to make sure macros only evaluate each input once.
        let mut execution_count = 0;
        let mut id_cnt = |x| {
            execution_count += 1;
            x
        };

        soft_assert!(2 + 2 == id_cnt(4));
        soft_assert!(2 + 2 == id_cnt(5));
        soft_assert_eq!(2 + 2, id_cnt(4));
        soft_assert_eq!(2 + 2, id_cnt(5));
        soft_assert_ne!(2 + 2, id_cnt(4));
        soft_assert_ne!(2 + 2, id_cnt(5));

        soft_assert!(2 + 2 == id_cnt(4), "custom message {}", 42);
        soft_assert!(2 + 2 == id_cnt(5), "custom message {}", 42);
        soft_assert_eq!(2 + 2, id_cnt(4), "custom message {}", 42);
        soft_assert_eq!(2 + 2, id_cnt(5), "custom message {}", 42);
        soft_assert_ne!(2 + 2, id_cnt(4), "custom message {}", 42);
        soft_assert_ne!(2 + 2, id_cnt(5), "custom message {}", 42);

        // Test the macros in expression position
        #[allow(unreachable_patterns)]
        let nothing = match 0 {
            _ => soft_assert!(2 + 2 == id_cnt(4)),
            _ => soft_assert!(2 + 2 == id_cnt(5)),
            _ => soft_assert_eq!(2 + 2, id_cnt(4)),
            _ => soft_assert_eq!(2 + 2, id_cnt(5)),
            _ => soft_assert_ne!(2 + 2, id_cnt(4)),
            _ => soft_assert_ne!(2 + 2, id_cnt(5)),

            _ => soft_assert!(2 + 2 == id_cnt(4), "custom message {}", 42),
            _ => soft_assert!(2 + 2 == id_cnt(5), "custom message {}", 42),
            _ => soft_assert_eq!(2 + 2, id_cnt(4), "custom message {}", 42),
            _ => soft_assert_eq!(2 + 2, id_cnt(5), "custom message {}", 42),
            _ => soft_assert_ne!(2 + 2, id_cnt(4), "custom message {}", 42),
            _ => soft_assert_ne!(2 + 2, id_cnt(5), "custom message {}", 42),
        };
        assert_eq!(nothing, ());
        assert_eq!(execution_count, 6 + 6 + 1); // +1 because only one match arm runs

        // Unlike std's assert, we accept &bool. This is originally an accident but why not.
        soft_assert!(&true);
        soft_assert!(&false);

        // Test everything lives long enough when using references to temporaries.
        // This is slightly more complicated than it looks because rust appears to extend lifetimes in some cases.
        // For example:
        //
        // let b1 = id(&(true && true));
        // dbg!(b1);
        //
        // let b2 = &(true && true);
        // dbg!(b2);
        //
        // dbg!(id(&(true && true)));
        //
        // let i1 = id(&(1 + 1));
        // dbg!(i1);
        //
        // let idk = id(&id(&(1 + 1)));
        // dbg!(idk);
        //
        // Only b1 and idk fail to compile and only if they're actually used.
        // Trait based binary ops behae differently than logical && and ||.
        // I don't fully understand these rules but the tests below are written so they fail
        // if the macros don't extend lifetimes properly.
        fn id<T>(t: T) -> T {
            t
        }
        soft_assert!(id(&(true && true)));
        soft_assert!(id(&(false && false)));
        soft_assert_eq!(id(&id(2 + 2)), &4);
        soft_assert_eq!(id(&id(2 + 2)), &5);
        soft_assert_ne!(id(&id(2 + 2)), &4);
        soft_assert_ne!(id(&id(2 + 2)), &5);

        let vals = vec![1, 2, 3, 4].into_iter();
        soft_assert_eq!(vals.clone().collect::<Vec<_>>().as_slice(), [1, 2, 3, 4]);
        soft_assert_ne!(vals.clone().collect::<Vec<_>>().as_slice(), [1, 2, 3, 4]);
    }

    #[test]
    fn test_soft_unreachable() {
        (|| soft_unreachable!())();
        (|| soft_unreachable!("custom message {}", 42))();

        fn int1(option: Option<i32>) -> i32 {
            let Some(x) = option else { soft_unreachable!() };
            x
        }
        int1(None);
        fn int2(option: Option<i32>) -> i32 {
            let Some(x) = option else {
                soft_unreachable!("custom message {}", 42)
            };
            x
        }
        int2(None);

        fn void1() {
            soft_unreachable!()
        }
        void1();
        fn void2() {
            soft_unreachable!("custom message {}", 42)
        }
        void2();
    }

    #[test]
    fn test_soft_unwrap() {
        #![allow(clippy::bool_assert_comparison)]

        assert_eq!(Some(42).soft_unwrap(), 42);
        assert_eq!(None::<i32>.soft_unwrap(), 0);
        assert_eq!(Ok::<i32, &str>(42).soft_unwrap(), 42);
        assert_eq!(Err::<i32, &str>("abc").soft_unwrap(), 0);

        assert_eq!("42".parse::<i32>().ok().soft_unwrap(), 42);
        assert_eq!("abc".parse::<i32>().ok().soft_unwrap(), 0);
        assert_eq!("42".parse::<bool>().ok().soft_unwrap(), false);
        assert_eq!("42".parse::<i32>().soft_unwrap(), 42);
        assert_eq!("abc".parse::<i32>().soft_unwrap(), 0);
        assert_eq!("42".parse::<bool>().soft_unwrap(), false);
    }

    #[test]
    fn test_dbg_iterator() {
        let mut count = 0;
        let mut finished = false;
        [1, 2, 3, 4, 5]
            .iter()
            .dbg_count(|cnt, fin| {
                count = cnt;
                finished = fin;
            })
            .for_each(|_| {});
        assert_eq!(count, 5);
        assert!(finished);

        [1, 2, 3, 4, 5]
            .iter()
            .dbg_count(|cnt, fin| {
                count = cnt;
                finished = fin;
            })
            .take(3)
            .for_each(|_| {});
        assert_eq!(count, 3);
        assert!(!finished);
    }
}
