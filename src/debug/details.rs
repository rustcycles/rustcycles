//! Implementation details of debugging tools.
//!
//! They have to be public because the macros use them
//! but in normal usage you should prefer the `dbg_*` macros
//! and other items from the parent mod.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::prelude::*;

/// Private helper to print the name and value of each given variable.
/// Not meant to be used directly.
#[macro_export]
macro_rules! __format_pairs {
    ( ) => {
        format!("")
    };
    ( $e:expr ) => {
        // We use {:?} instead of {} here because it's more likely to stay on one line.
        // E.g. nalgebra vectors get printed as columns when using {}.
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) enum Shape {
    Line { begin: Vec3, end: Vec3 },
    Arrow { begin: Vec3, dir: Vec3 },
    Cross { point: Vec3 },
}

/// Helper struct, use one of the `dbg_*!()` macros.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct DebugShape {
    pub(crate) shape: Shape,
    /// Time left (decreases every frame)
    pub(crate) time: f32,
    #[serde(with = "ColorDef")]
    pub(crate) color: Color,
}

/// Fyrox's Color doesn't impl serde traits
/// so we do this: https://serde.rs/remote-derive.html
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(remote = "Color")]
pub struct ColorDef {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Helper function, prefer `dbg_line!()` instead.
pub(crate) fn debug_line(begin: Vec3, end: Vec3, time: f32, color: Color) {
    let shape = Shape::Line { begin, end };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_line!()` instead.
pub(crate) fn debug_arrow(begin: Vec3, dir: Vec3, time: f32, color: Color) {
    let shape = Shape::Arrow { begin, dir };
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

#[derive(Debug, Clone)]
pub(crate) struct DebugEndpoint {
    pub(crate) name: &'static str,
    pub(crate) default_color: Color,
}

// LATER(multithreading) Make debug tools work correctly from all threads.
thread_local! {
    // The default value here should be overwritten as soon as it's decided
    // whether the thread is a client or a server. If you see it in stdout/stderr,
    // something is very wrong - it crashed very early or somebody spawned
    // more threads without setting this.
    pub(crate) static DEBUG_ENDPOINT: RefCell<DebugEndpoint> = RefCell::new(DebugEndpoint{
        name: "??cl/sv",
        default_color: Color::WHITE,
    });

    pub(crate) static DEBUG_TEXTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    pub(crate) static DEBUG_SHAPES: RefCell<Vec<DebugShape>> = RefCell::new(Vec::new());
}

pub(crate) fn default_color() -> Color {
    DEBUG_ENDPOINT.with(|endpoint| endpoint.borrow().default_color)
}

pub(crate) fn cleanup() {
    DEBUG_TEXTS.with(|texts| texts.borrow_mut().clear());
    DEBUG_SHAPES.with(|shapes| shapes.borrow_mut().retain(|shape| shape.time > 0.0));
}
