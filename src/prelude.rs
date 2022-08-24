//! A bunch of QoL macros, traits and functions
//! to make gamedev in Rust faster and saner.
//!
//! Should be imported in most files via
//! `use crate::prelude::*`.

// The whole point of this mod is to provide a bunch of stuff
// that may or may not be used but should be *available*.
#![allow(unused_imports)]
#![allow(dead_code)]

use fyrox::core::algebra::Vector3;

// Make the most commonly used types available.
// Criteria for inclusion: used in a lot of files and unlikely to collide.
pub(crate) use fyrox::{
    core::{
        algebra::{Unit, UnitQuaternion},
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

// This doesn't increase compile times in any measureble way.
// Keep it here so it can be used immediately without adding to Cargo.toml or importing first.
pub(crate) use inline_tweak::tweak;

/// For debugging only, use `Color::YOUR_COLOR_HERE` in normal code.
pub(crate) use crate::debug::colors::*;

// Visibility of macros by example works diffrently from normal items,
// they behave as if they were defined in the crate's root
// so we import it here to make it part of prelude.
pub(crate) use crate::v;

/// Shorthand for `Vector3::new()`.
///
/// Short name, no decimal point (casts to f32), no commas between numbers.
///
/// X, Y, Z is left, up, forward.
///
/// Nalgebra's coordinate system is right-handed:
/// index finger is X, middle finger is Y, thumb is Z.
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
pub(crate) type Vec3 = Vector3<f32>;

/// QoL methods for nalgebra's Vector3.
///
/// Should be imported along with the rest of the prelude using a glob.
pub(crate) trait Vec3Ext
where
    Self: Sized,
{
    /// The column vector with a 1 as its first (X) component, and zero elsewhere.
    fn left() -> Self;
    /// The column vector with a 1 as its second (Y) component, and zero elsewhere.
    fn up() -> Self;
    /// The column vector with a 1 as its third (Z) component, and zero elsewhere.
    fn forward() -> Self;

    /// The unit column vector with a 1 as its first (X) component, and zero elsewhere.
    fn left_axis() -> Unit<Self>;
    /// The unit column vector with a 1 as its second (Y) component, and zero elsewhere.
    fn up_axis() -> Unit<Self>;
    /// The unit column vector with a 1 as its third (Z) component, and zero elsewhere.
    fn forward_axis() -> Unit<Self>;
}

impl Vec3Ext for Vec3 {
    fn left() -> Self {
        Self::x()
    }
    fn up() -> Self {
        Self::y()
    }
    fn forward() -> Self {
        Self::z()
    }

    fn left_axis() -> Unit<Self> {
        Self::x_axis()
    }
    fn up_axis() -> Unit<Self> {
        Self::y_axis()
    }
    fn forward_axis() -> Unit<Self> {
        Self::z_axis()
    }
}

/// QoL methods for fyrox's Node.
///
/// Should be imported along with the rest of the prelude using a glob.
pub(crate) trait NodeExt {
    /// The "side" vector of the global transform basis, might not be normalized.
    fn left_vec(&self) -> Vec3;
    /// The "up" vector of the global transform basis, might not be normalized.
    fn up_vec(&self) -> Vec3;
    /// The "look" vector of the global transform basis, might not be normalized.
    fn forward_vec(&self) -> Vec3;

    /// The normalized "side" vector of the global transform basis.
    fn left_vec_normed(&self) -> Vec3;
    /// The normalized "up" vector of the global transform basis.
    fn up_vec_normed(&self) -> Vec3;
    /// The normalized "look" vector of the global transform basis.
    fn forward_vec_normed(&self) -> Vec3;
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

    fn left_vec_normed(&self) -> Vec3 {
        self.left_vec().try_normalize(f32::EPSILON).unwrap_or_else(Vec3::left)
    }
    fn up_vec_normed(&self) -> Vec3 {
        self.up_vec().try_normalize(f32::EPSILON).unwrap_or_else(Vec3::up)
    }
    fn forward_vec_normed(&self) -> Vec3 {
        self.forward_vec().try_normalize(f32::EPSILON).unwrap_or_else(Vec3::forward)
    }
}

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
