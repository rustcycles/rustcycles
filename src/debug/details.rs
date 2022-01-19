//! Implementation details of debugging tools.
//!
//! They have to be public because the macros use them
//! but for debugging, you should prefer the `dbg_*` macros
//! and other items from the parent mod.

use std::cell::RefCell;

use rg3d::core::color::Color;

use crate::prelude::*;

pub(crate) enum Shape {
    Cross { point: Vec3 },
}

/// Helper struct, use one of the `dbg_*!()` macros.
pub(crate) struct DebugShape {
    pub(crate) shape: Shape,
    /// Time left (decreases every frame)
    pub(crate) time: f32,
    pub(crate) color: Color,
}

/// Helper function, prefer `dbg_cross!()` instead.
pub(crate) fn debug_cross(point: Vec3, time: f32, color: Color) {
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
