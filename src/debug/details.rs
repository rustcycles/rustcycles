//! Implementation details of debugging tools.
//!
//! They have to be public because the macros use them
//! but in normal usage you should prefer the `dbg_*` macros
//! and other items from the parent mod.

// Some items in this file could trivially be inlined into debug.rs.
// Usually, they're here because they differ between RecWars and RustCycles.

use fxhash::FxHashMap;
use fyrox::{core::algebra::Vector3, scene::debug::Line};
use serde::{Deserialize, Serialize};

use crate::{debug::DEBUG_SHAPES, prelude::*};

#[macro_export]
macro_rules! __println {
    ($($t:tt)*) => {
        println!($($t)*)
    }
}

/// Helper struct, use one of the `dbg_*!()` macros.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorldText {
    pub pos: Vec3,
    pub msg: String,
}

impl WorldText {
    pub fn new(pos: Vec3, msg: String) -> Self {
        Self { pos, msg }
    }
}

/// Helper struct, use one of the `dbg_*!()` macros.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DebugShape {
    pub shape: Shape,
    /// Time left (decreases every frame)
    pub time: f32,
    #[serde(with = "ColorDef")]
    pub color: Color,
}

/// Helper enum, use one of the `dbg_*!()` macros.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Shape {
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
        scale: f32,
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

/// Helper function, prefer `dbg_line!()` instead.
pub fn debug_line(begin: Vec3, end: Vec3, time: f32, color: Color) {
    let shape = Shape::Line { begin, end };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_arrow!()` instead.
pub fn debug_arrow(begin: Vec3, dir: Vec3, time: f32, color: Color) {
    let shape = Shape::Arrow { begin, dir };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_cross!()` instead.
pub fn debug_cross(point: Vec3, time: f32, color: Color) {
    let shape = Shape::Cross { point };
    debug_shape(shape, time, color);
}

/// Helper function, prefer `dbg_rot!()` instead.
pub fn debug_rot(point: Vec3, rot: UnitQuaternion<f32>, time: f32, scale: f32) {
    let shape = Shape::Rot { point, rot, scale };
    // Color is not used
    debug_shape(shape, time, WHITE);
}

fn debug_shape(shape: Shape, time: f32, color: Color) {
    DEBUG_SHAPES.with(|shapes| {
        let shape = DebugShape { shape, time, color };
        shapes.borrow_mut().push(shape);
    });
}

impl DebugShape {
    pub fn to_lines(&self, cvars: &Cvars, lines: &mut UniqueLines) {
        match self.shape {
            Shape::Line { begin, end } => {
                if !cvars.d_draw_lines {
                    return;
                }

                lines.insert(begin, end, self.color);

                // LATER d_draw_lines_ends_half_length line in RecWars
            }
            Shape::Arrow { begin, dir } => {
                if !cvars.d_draw_arrows {
                    return;
                }

                let end = begin + dir;
                lines.insert(begin, end, self.color);

                // When the arrow is exactly horizontal, we want two of the side lines
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
                    lines.insert(Vec3::zeros(), point, self.color);
                }
            }
            Shape::Rot { point, rot, scale } => {
                if !cvars.d_draw_rots {
                    return;
                }

                let size = scale * cvars.d_draw_rots_size;

                // Oringally, this used SceneDrawingContext::draw_transform
                // but this way we can use BLUE2 instead of the hard to see BLUE.
                lines.insert(point, point + rot * (size * LEFT), RED);
                lines.insert(point, point + rot * (size * UP), GREEN);
                lines.insert(point, point + rot * (size * FORWARD), BLUE2);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct UniqueLines(pub FxHashMap<(Vector3<u32>, Vector3<u32>), Line>);

impl UniqueLines {
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

#[cfg(test)]
pub const V1: Vec3 = v!(1 2 3);
#[cfg(test)]
pub const V2: Vec3 = v!(4 5 6);
#[cfg(test)]
#[macro_export]
macro_rules! r1 {
    () => {
        // There's no way to construct a unit quaternion
        // in a const context, just use a macro.
        UnitQuaternion::from_euler_angles(0.1, 0.2, 0.3)
    };
}
