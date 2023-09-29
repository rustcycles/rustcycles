//! A bunch of QoL consts, macros, traits and functions
//! to make gamedev in Rust faster and saner.
//!
//! Should be imported in most files via
//! `use crate::prelude::*`.

// The whole point of this mod is to provide a bunch of stuff
// that may or may not be used but should be *available*.
#![allow(unused_imports)]
#![allow(dead_code)]

// Some private imports that are intentionally *not* re-exported.
use fyrox::{core::algebra, scene::collider::BitMask};

// Public re-exports.
// Make the most commonly used types available without importing manually.
// Criteria for inclusion: used in a lot of files and unlikely to collide.

pub use std::{
    collections::VecDeque,
    f32::consts::PI,
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

// This should generally be used instead of std's HashMap and HashSet
// because we usually don't need HashDoS protection but do need determinism.
pub use fxhash::{FxHashMap, FxHashSet};

pub use fyrox::{
    core::{
        algebra::{Unit, UnitQuaternion, Vector2, Vector3, Vector4},
        color::Color,
        pool::{Handle, Pool},
    },
    engine::Engine,
    resource::model::Model,
    scene::{
        base::{Base, BaseBuilder},
        collider::{Collider, ColliderBuilder, ColliderShape},
        node::Node,
        rigidbody::{RigidBody, RigidBodyBuilder, RigidBodyType},
        transform::TransformBuilder,
        Scene,
    },
};

// This doesn't increase incremental compile times in any measureble way.
// Keep it here so it can be used immediately without adding to Cargo.toml or importing first.
pub use inline_tweak::tweak;

pub use rand::prelude::*;
// `rng.sample(Normal::new(mean, std_dev))` gives exactly the same results as
// `rng.sample(StandardNormal) * std_dev + mean`.
// The latter sometimes requires type annotations.
pub use rand_distr::{Normal, StandardNormal, Uniform};
pub use rand_xoshiro::Xoshiro256PlusPlus;

pub use serde::{Deserialize, Serialize};

pub use strum::EnumCount;
pub use strum_macros::{EnumCount, FromRepr};

pub use crate::{
    client::game::ClientFrameCtx,
    common::{
        messages::*,
        trace::{trace_line, TraceOptions},
        FrameCtx, GameState, GameStateType,
    },
    cvars::*,
    debug::{DbgIterator, SoftUnwrap},
    server::game::ServerFrameCtx,
};

// Visibility of macros by example works diffrently from normal items,
// they behave as if they were defined in the crate's root
// so we import it here to make it part of prelude.
pub use crate::v;

/// Shorthand for `Vector3::new()`.
///
/// Short name, no decimal point (casts to f32), no commas between numbers.
///
/// X, Y, Z is **left, up, forward**.
///
/// Nalgebra's coordinate system is right-handed:
/// index finger is X, middle finger is Y, thumb is Z.
/// Alternatively (easier to remember?):
/// thumb is X, index finger is Y, middle finger is Z.
///
/// Nalgebra's rotations also use the right-hand rule:
/// thumb is the axis, the curl of fingers is the direction of rotation.
///
/// ---
///
/// The most common usecase is a constant vector with all coords as number literals,
/// e.g. `v!(-42 0 420.69)`. If you need arbitrary expressions
/// (e.g. `v!(-s.x, 0, a + b)`), you need to use commas
/// because expressions can contain spaces so they wouldn't work as a separator.
///
/// LATER Check f32 represents the input value exactly, log warn if not, rate limit it.
///
/// # Usage
///
/// ```rust
/// v!(1 2 3)
/// ```
#[macro_export]
macro_rules! v {
    // Support for arbitrary expressions - requires commas.
    ($x:expr, $y:expr, $z:expr) => {
        Vec3::new($x as f32, $y as f32, $z as f32)
    };
    // The simple usecase - no commas.
    ($x:literal $y:literal $z:literal) => {
        Vec3::new($x as f32, $y as f32, $z as f32)
    };
}

/// Shorthand for `Vector3<f32>`
///
/// X, Y, Z is left, up, forward.
pub type Vec3 = Vector3<f32>;
pub type Point3 = algebra::Point3<f32>;

// Consts take less typing than using an extension trait - e.g. `Vec3::up()`
// even though it's less explicit we're talking about 3D vectors.

/// The column vector with a 1 as its first (X) component, and zero elsewhere.
pub const LEFT: Vec3 = v!(1 0 0);
/// The column vector with a 1 as its second (Y) component, and zero elsewhere.
pub const UP: Vec3 = v!(0 1 0);
/// The column vector with a 1 as its third (Z) component, and zero elsewhere.
pub const FORWARD: Vec3 = v!(0 0 1);
/// The column vector with a -1 as its first (X) component, and zero elsewhere.
pub const RIGHT: Vec3 = v!(-1 0 0);
/// The column vector with a -1 as its second (Y) component, and zero elsewhere.
pub const DOWN: Vec3 = v!(0 -1 0);
/// The column vector with a -1 as its third (Z) component, and zero elsewhere.
pub const BACK: Vec3 = v!(0 0 -1);

/// The unit column vector with a 1 as its first (X) component, and zero elsewhere.
pub const LEFT_AXIS: Unit<Vec3> = Unit::new_unchecked(LEFT);
/// The unit column vector with a 1 as its second (Y) component, and zero elsewhere.
pub const UP_AXIS: Unit<Vec3> = Unit::new_unchecked(UP);
/// The unit column vector with a 1 as its third (Z) component, and zero elsewhere.
pub const FORWARD_AXIS: Unit<Vec3> = Unit::new_unchecked(FORWARD);
/// The unit column vector with a -1 as its first (X) component, and zero elsewhere.
pub const RIGHT_AXIS: Unit<Vec3> = Unit::new_unchecked(RIGHT);
/// The unit column vector with a -1 as its second (Y) component, and zero elsewhere.
pub const DOWN_AXIS: Unit<Vec3> = Unit::new_unchecked(DOWN);
/// The unit column vector with a -1 as its third (Z) component, and zero elsewhere.
pub const BACK_AXIS: Unit<Vec3> = Unit::new_unchecked(BACK);

/// QoL methods for Vec3
///
/// Should be imported along with the rest of the prelude using a glob.
pub trait Vec3Ext {
    /// The X component of the vector.
    fn left(&self) -> f32;
    /// The Y component of the vector.
    fn up(&self) -> f32;
    /// The Z component of the vector.
    fn forward(&self) -> f32;
    /// The negative X component of the vector.
    fn right(&self) -> f32;
    /// The negative Y component of the vector.
    fn down(&self) -> f32;
    /// The negative Z component of the vector.
    fn back(&self) -> f32;
}

impl Vec3Ext for Vec3 {
    fn left(&self) -> f32 {
        self.x
    }
    fn up(&self) -> f32 {
        self.y
    }
    fn forward(&self) -> f32 {
        self.z
    }
    fn right(&self) -> f32 {
        -self.x
    }
    fn down(&self) -> f32 {
        -self.y
    }
    fn back(&self) -> f32 {
        -self.z
    }
}

/// QoL methods for UnitQuaternion
///
/// Should be imported along with the rest of the prelude using a glob.
pub trait UnitQuaternionExt {
    /// Create a unit quaternion from yaw, pitch and roll in that order.
    fn from_ypr(yaw: f32, pitch: f32, roll: f32) -> Self;
    /// Create a unit quaternion from yaw, pitch and roll in that order.
    fn from_ypr_extrinsic(yaw: f32, pitch: f32, roll: f32) -> Self;
}

impl UnitQuaternionExt for UnitQuaternion<f32> {
    fn from_ypr(yaw: f32, pitch: f32, roll: f32) -> Self {
        let mut quat = UnitQuaternion::from_axis_angle(&UP_AXIS, yaw);

        let local_left = quat * LEFT_AXIS;
        quat = UnitQuaternion::from_axis_angle(&local_left, pitch) * quat;
        let local_forward = quat * FORWARD_AXIS;
        quat = UnitQuaternion::from_axis_angle(&local_forward, roll) * quat;

        quat
    }

    fn from_ypr_extrinsic(yaw: f32, pitch: f32, roll: f32) -> Self {
        let qyaw = UnitQuaternion::from_axis_angle(&UP_AXIS, yaw);
        let qpitch = UnitQuaternion::from_axis_angle(&LEFT_AXIS, pitch);
        let qroll = UnitQuaternion::from_axis_angle(&FORWARD_AXIS, roll);
        qroll * qpitch * qyaw
    }
}

/// QoL methods for fyrox's Node.
///
/// Should be imported along with the rest of the prelude using a glob.
pub trait NodeExt {
    /// The "side" vector of the global transform basis, might not be normalized.
    fn left_vec(&self) -> Vec3;
    /// The "up" vector of the global transform basis, might not be normalized.
    fn up_vec(&self) -> Vec3;
    /// The "look" vector of the global transform basis, might not be normalized.
    fn forward_vec(&self) -> Vec3;

    /// The negative "side" vector of the global transform basis, might not be normalized.
    fn right_vec(&self) -> Vec3 {
        -self.left_vec()
    }

    /// The negative "up" vector of the global transform basis, might not be normalized.
    fn down_vec(&self) -> Vec3 {
        -self.up_vec()
    }

    /// The negative "look" vector of the global transform basis, might not be normalized.
    fn back_vec(&self) -> Vec3 {
        -self.forward_vec()
    }

    /// The normalized "side" vector of the global transform basis.
    fn left_vec_normed(&self) -> Vec3 {
        self.left_vec().try_normalize(f32::EPSILON).unwrap_or(LEFT)
    }

    /// The normalized "up" vector of the global transform basis.
    fn up_vec_normed(&self) -> Vec3 {
        self.up_vec().try_normalize(f32::EPSILON).unwrap_or(UP)
    }

    /// The normalized "look" vector of the global transform basis.
    fn forward_vec_normed(&self) -> Vec3 {
        self.forward_vec().try_normalize(f32::EPSILON).unwrap_or(FORWARD)
    }

    /// The normalized negative "side" vector of the global transform basis.
    fn right_vec_normed(&self) -> Vec3 {
        -self.left_vec_normed()
    }

    /// The normalized negative "up" vector of the global transform basis.
    fn down_vec_normed(&self) -> Vec3 {
        -self.up_vec_normed()
    }

    /// The normalized negative "look" vector of the global transform basis.
    fn back_vec_normed(&self) -> Vec3 {
        -self.forward_vec_normed()
    }
}

impl NodeExt for Node {
    fn left_vec(&self) -> Vec3 {
        self.side_vector()
    }
    fn up_vec(&self) -> Vec3 {
        self.up_vector()
    }
    fn forward_vec(&self) -> Vec3 {
        self.look_vector()
    }
}

// Associated consts can't be imported (or even glob-imported)
// so we have to redefine them here.
pub const WHITE: Color = Color::WHITE;
pub const BLACK: Color = Color::BLACK;
pub const RED: Color = Color::RED;
pub const GREEN: Color = Color::GREEN;
/// A hard to see dark blue, prefer BLUE2 instead.
pub const BLUE: Color = Color::BLUE;
pub const TRANSPARENT: Color = Color::TRANSPARENT;
pub const ORANGE: Color = Color::ORANGE;

// And a couple more custom colors.
// This doesn't follow any standard color naming scheme.
/// A blue you can actually see
pub const BLUE2: Color = Color::opaque(0, 100, 255);
pub const YELLOW: Color = Color::opaque(255, 255, 0);
pub const MAGENTA: Color = Color::opaque(255, 0, 255);
pub const CYAN: Color = Color::opaque(0, 255, 255);

// Note: These are a bit weird because the default is all bits set
// so most colliders have all bits but some special objects only have a subset.
// For example, the player can have only IG_ENTITIES and then we can
// raycast while ignoring the player by setting `filter` to !IG_ENTITIES.
pub const IG_ENTITIES: BitMask = BitMask(1 << 0);
pub const IG_ALL: BitMask = BitMask(u32::MAX);

pub trait PoolExt<T> {
    /// Collect the handles into a `Vec`.
    ///
    /// This is a workaround for borrowck limitations so we can
    /// iterate over the pool without keeping it borrowed.
    /// You can reborrow each iteration of the loop by indexing the pool using the handle
    /// and release the borrow if you need to pass the pool (or usually the whole frame context)
    /// into another function.
    ///
    /// This is inefficient and ideally should be avoided
    /// but contrary to everyone in Rust gamedev circles talking about performance,
    /// most games are not limited by how fast their gamelogic runs.
    /// When/if we have perf issues and profiling says this is the cause,
    /// then we can restructure the code to avoid it.
    /// Until then writing code faster is more important than writing faster code.
    fn collect_handles(&self) -> Vec<Handle<T>>;
}

impl<T: 'static> PoolExt<T> for Pool<T> {
    fn collect_handles(&self) -> Vec<Handle<T>> {
        self.pair_iter().map(|(h, _)| h).collect()
    }
}

// For easly switching between f32 and f64.
// Currently only (meant to be) used in debug code.
#[allow(non_camel_case_types)]
pub type fl = f32;

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn test_v() {
        assert_eq!(v!(-42 0 420.69), Vec3::new(-42.0, 0.0, 420.69));

        struct S {
            x: i32,
        }
        let s = S { x: 42 };
        let a = 420.0;
        let b = 0.69;
        assert_eq!(v!(-s.x, 0, a + b), Vec3::new(-42.0, 0.0, 420.69));
    }
}
