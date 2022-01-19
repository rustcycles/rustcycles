//! A bunch of QoL macros, traits and functions
//! to make gamedev in Rust faster and saner.
//!
//! Most files in this game should import it via
//! `use crate::prelude::*`.

use rg3d::{
    core::algebra::{Unit, Vector3},
    scene::node::Node,
};

// Visibility of macros by example works diffrently from normal items,
// they behave as if they were defined in the crate's root
// so we import it here to make it part of prelude.
pub use crate::v;

/// Shorthand for Vector3::new().
///
/// Short name, no commas between numbers, no decimal point.
///
/// # Usage
///
/// ```rust
/// v!(1 2 3)
/// ```
#[macro_export]
macro_rules! v {
    ($x:literal $y:literal $z:literal) => {
        Vec3::new($x as f32, $y as f32, $z as f32)
    };
}

pub(crate) type Vec3 = Vector3<f32>;

/// Nalgebra's coordinate system is right-handed, I think.
pub(crate) trait Vec3Ext
where
    Self: Sized,
{
    fn left() -> Self;
    fn up() -> Self;
    fn forward() -> Self;

    fn left_axis() -> Unit<Self>;
    fn up_axis() -> Unit<Self>;
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

pub(crate) trait NodeExt {
    fn left_vec(&self) -> Vec3;
    fn up_vec(&self) -> Vec3;
    fn forward_vec(&self) -> Vec3;

    fn left_vec_normed(&self) -> Vec3;
    fn up_vec_normed(&self) -> Vec3;
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
        self.left_vec()
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vec3::left)
    }
    fn up_vec_normed(&self) -> Vec3 {
        self.up_vec()
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vec3::up)
    }
    fn forward_vec_normed(&self) -> Vec3 {
        self.forward_vec()
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vec3::forward)
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn test_v() {
        assert_eq!(v!(-42 0 420.69), Vec3::new(-42.0, 0.0, 420.69));
    }
}
