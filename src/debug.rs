//! debug tools for logging (LATER) and visualizing what is going on.
//!
//! LATER How does this interact with client vs server framerate?

#![allow(dead_code)]

use std::cell::RefCell;

use rg3d::core::{algebra::Vector3, color::Color};

#[macro_export]
macro_rules! soft_assert {
    ($cond:expr $(,)?) => {
        soft_assert!($cond, stringify!($cond));
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            println!("soft assertion failed: {}, {}:{}:{}", format!($($arg)+), file!(), line!(), column!());
        }
    };
}

/// Draw a cross at the given world coordinates.
/// Optionally specify
/// - how long it lasts in seconds (default is 0.0 which means 1 frame)
/// - color
#[macro_export]
macro_rules! dbg_cross {
    ($point:expr, $time:expr, $color:expr) => {
        $crate::debug::debug_cross($point, $time, $color);
    };
    ($point:expr, $time:expr) => {
        $crate::dbg_cross!($point, $time, Color::RED);
    };
    ($point:expr) => {
        $crate::dbg_cross!($point, 0.0);
    };
}

// Stuff below this line is considered private.
// --------------------------------------------
// You can use it (safety doesn't depend on it)
// but it's usually more ergonomic to use the macros and functions above above.
// LATER submod?

pub(crate) enum Shape {
    Cross { point: Vector3<f32> },
}

/// Helper struct, use one of the `dbg_*!()` macros.
pub(crate) struct DebugShape {
    pub(crate) shape: Shape,
    /// Time left (decreases every frame)
    pub(crate) time: f32,
    pub(crate) color: Color,
}

/// Helper function, prefer `dbg_cross!()` instead.
pub fn debug_cross(point: Vector3<f32>, time: f32, color: Color) {
    DEBUG_SHAPES.with(|shapes| {
        let shape = Shape::Cross { point };
        let shape = DebugShape { shape, time, color };
        shapes.borrow_mut().push(shape);
    });
}

thread_local! {
    pub(crate) static DEBUG_SHAPES: RefCell<Vec<DebugShape>> = RefCell::new(Vec::new());
}

pub(crate) fn cleanup() {
    DEBUG_SHAPES.with(|shapes| shapes.borrow_mut().retain(|shape| shape.time > 0.0));
}

// ^ When adding to this file, keep in mind the public/private split.
