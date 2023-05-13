//! Implementation details of debugging tools.
//!
//! They have to be public because the macros use them
//! but in normal usage you should prefer the `dbg_*` macros
//! and other items from the parent mod.

use std::cell::RefCell;

use fxhash::FxHashMap;
use fyrox::{core::algebra::Vector3, scene::debug::Line};
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
pub(crate) struct DebugShape {
    pub(crate) shape: Shape,
    /// Time left (decreases every frame)
    pub(crate) time: f32,
    #[serde(with = "ColorDef")]
    pub(crate) color: Color,
}

/// Helper struct, use one of the `dbg_*!()` macros.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) enum Shape {
    Line {
        begin: Vec3,
        end: Vec3,
    },
    Arrow {
        begin: Vec3,
        dir: Vec3,
    },
    Cross {
        point: Vec3,
    },
    Rot {
        point: Vec3,
        rot: UnitQuaternion<f32>,
    },
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

impl DebugShape {
    pub(crate) fn to_lines(&self, cvars: &Cvars, lines: &mut Lines) {
        match self.shape {
            Shape::Line { begin, end } => {
                if !cvars.d_draw_lines {
                    return;
                }

                lines.insert(begin, end, self.color);
            }
            Shape::Arrow { begin, dir } => {
                if !cvars.d_draw_arrows {
                    return;
                }

                let end = begin + dir;
                lines.insert(begin, end, self.color);

                // When the arrow is horizontal, we want two of the side lines
                // to be above and below the arrow body and the other two to the sides.
                // When it's not horizontal, we want it to appear pitched up/down,
                // no weird rotations around the axis.

                // Make sure dir and up are not colinear.
                let up = if dir.left().abs() < f32::EPSILON && dir.forward().abs() < f32::EPSILON {
                    FORWARD
                } else {
                    UP
                };

                let rot = UnitQuaternion::face_towards(&dir, &up);
                let len = dir.magnitude();
                let left = rot * LEFT * len;
                let up = rot * UP * len;
                lines.insert(end, end + (-dir + left) * 0.25, self.color);
                lines.insert(end, end + (-dir - left) * 0.25, self.color);
                lines.insert(end, end + (-dir + up) * 0.25, self.color);
                lines.insert(end, end + (-dir - up) * 0.25, self.color);
            }
            Shape::Cross { point } => {
                if !cvars.d_draw_crosses {
                    return;
                }

                let dir1 = v!(1 1 1) * cvars.d_draw_crosses_half_len;
                let dir2 = v!(-1 1 1) * cvars.d_draw_crosses_half_len;
                let dir3 = v!(1 1 -1) * cvars.d_draw_crosses_half_len;
                let dir4 = v!(-1 1 -1) * cvars.d_draw_crosses_half_len;
                lines.insert(point - dir1, point + dir1, self.color);
                lines.insert(point - dir2, point + dir2, self.color);
                lines.insert(point - dir3, point + dir3, self.color);
                lines.insert(point - dir4, point + dir4, self.color);

                if cvars.d_draw_crosses_line_from_origin {
                    // This is sometimes useful if we have trouble finding the cross.
                    lines.insert(Vec3::zeros(), point, self.color);
                }
            }
            Shape::Rot { point, rot } => {
                if !cvars.d_draw_rots {
                    return;
                }

                // Oringally, this used SceneDrawingContext::draw_transform
                // but this way we can use BLUE2 instead of the hard to see BLUE.
                lines.insert(point, point + rot * LEFT, RED);
                lines.insert(point, point + rot * UP, GREEN);
                lines.insert(point, point + rot * FORWARD, BLUE2);
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Lines(pub(crate) FxHashMap<(Vector3<u32>, Vector3<u32>), Line>);

impl Lines {
    pub(crate) fn new() -> Self {
        Self(FxHashMap::default())
    }

    /// Insert the line into the hashmap, merging colors if a line already exists
    /// in the exact same place.
    fn insert(&mut self, begin: Vec3, end: Vec3, color: Color) {
        // It might be tempting to add a tiny bit of tolerance
        // so lines close enough get merged
        // but it would make it hard to notice cl/sv desyncs.
        // At least it should be off by default.
        let bits_begin = begin.map(|v| v.to_bits());
        let bits_end = end.map(|v| v.to_bits());

        self.0
            .entry((bits_begin, bits_end))
            .and_modify(|line| line.color += color)
            .or_insert(Line { begin, end, color });
    }
}

/// Helper function, prefer `dbg_line!()` instead.
pub(crate) fn debug_line(begin: Vec3, end: Vec3, time: f32, color: Color) {
    let shape = Shape::Line { begin, end };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_arrow!()` instead.
pub(crate) fn debug_arrow(begin: Vec3, dir: Vec3, time: f32, color: Color) {
    let shape = Shape::Arrow { begin, dir };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_cross!()` instead.
pub(crate) fn debug_cross(point: Vec3, time: f32, color: Color) {
    let shape = Shape::Cross { point };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_rot!()` instead.
pub(crate) fn debug_rot(point: Vec3, rot: UnitQuaternion<f32>, time: f32) {
    let shape = Shape::Rot { point, rot };
    // Color is not used
    debug_shape(shape, time, Color::WHITE);
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
    // something is wrong - it's very early in startup or somebody spawned
    // more threads without setting this.
    static DEBUG_ENDPOINT: RefCell<DebugEndpoint> = RefCell::new(DebugEndpoint{
        name: "??cl/sv",
        default_color: Color::WHITE,
    });

    pub(crate) static DEBUG_TEXTS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    pub(crate) static DEBUG_SHAPES: RefCell<Vec<DebugShape>> = RefCell::new(Vec::new());
}

pub(crate) fn set_endpoint(name: &'static str) {
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

pub(crate) fn endpoint_name() -> &'static str {
    DEBUG_ENDPOINT.with(|endpoint| endpoint.borrow().name)
}

pub(crate) fn endpoint_color() -> Color {
    DEBUG_ENDPOINT.with(|endpoint| endpoint.borrow().default_color)
}

pub(crate) fn clear_expired() {
    DEBUG_TEXTS.with(|texts| texts.borrow_mut().clear());
    DEBUG_SHAPES.with(|shapes| shapes.borrow_mut().retain(|shape| shape.time > 0.0));
}
