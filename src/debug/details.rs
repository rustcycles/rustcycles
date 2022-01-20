//! Implementation details of debugging tools.
//!
//! They have to be public because the macros use them
//! but in normal usage you should prefer the `dbg_*` macros
//! and other items from the parent mod.

use std::cell::RefCell;

use rg3d::core::color::Color;

use crate::prelude::*;

/// Private helper to print the name and value of each given variable.
/// Not meant to be used directly.
#[macro_export]
macro_rules! __format_pairs {
    ( $e:expr ) => {
        format!("{}: {:.6?}", stringify!($e), $e)
    };
    ( $e:expr, $( $rest:expr ),+ ) => {
        format!(
            "{}, {}",
            $crate::__format_pairs!($e),
            $crate::__format_pairs!( $( $rest ),+ )
        )
    };
}

/// Helper struct, use one of the `dbg_*!()` macros.
pub(crate) enum Shape {
    Line { begin: Vec3, end: Vec3 },
    Arrow { begin: Vec3, end: Vec3 },
    Cross { point: Vec3 },
}

/// Helper struct, use one of the `dbg_*!()` macros.
pub(crate) struct DebugShape {
    pub(crate) shape: Shape,
    /// Time left (decreases every frame)
    pub(crate) time: f32,
    pub(crate) color: Color,
}

/// Helper function, prefer `dbg_line!()` instead.
pub(crate) fn debug_line(begin: Vec3, end: Vec3, time: f32, color: Color) {
    let shape = Shape::Line { begin, end };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_line!()` instead.
pub(crate) fn debug_arrow(begin: Vec3, end: Vec3, time: f32, color: Color) {
    let shape = Shape::Arrow { begin, end };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_cross!()` instead.
pub(crate) fn debug_cross(point: Vec3, time: f32, color: Color) {
    let shape = Shape::Cross { point };
    debug_shape(shape, time, color);
}

fn debug_shape(shape: Shape, time: f32, color: Color) {
    DEBUG_SHAPES.with(|shapes| {
        let shape = DebugShape { shape, time, color };
        shapes.borrow_mut().push(shape);
    });
}

thread_local! {
    pub(crate) static DEBUG_ENDPOINT: RefCell<&'static str> = RefCell::new("UNKNOWN");
    pub(crate) static DEBUG_SHAPES: RefCell<Vec<DebugShape>> = RefCell::new(Vec::new());
}

pub(crate) fn cleanup() {
    DEBUG_SHAPES.with(|shapes| shapes.borrow_mut().retain(|shape| shape.time > 0.0));
}
