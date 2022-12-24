use fyrox::scene::{
    collider::{BitMask, InteractionGroups},
    graph::physics::{FeatureId, Intersection, RayCastOptions},
};

use crate::prelude::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct TraceOptions {
    /// This is passed to `InteractionGroups::new`.
    pub(crate) memberships: BitMask,
    /// This is passed to `InteractionGroups::new`.
    pub(crate) filter: BitMask,
    /// Sort the results by distance from the ray origin.
    pub(crate) sort: bool,
    /// When the raycast hits something, return a position slightly before the hit
    /// to avoid accidentally getting to the other side due to float imprecision.
    ///
    /// This is on by default because it's easy to forget and usually this is what we want anyway.
    pub(crate) nudge: Option<f32>,
    /// Include a dummy Intersection at the end with the position where the raycast ended
    /// so that the result is always non-empty.
    ///
    /// This can simplify some code that wants to trace limited distance or until it hits something.
    pub(crate) end: bool,
}

impl Default for TraceOptions {
    fn default() -> Self {
        Self {
            memberships: IG_ALL,
            filter: IG_ALL,
            sort: true,
            nudge: None,
            end: false,
        }
    }
}

#[allow(dead_code)]
impl TraceOptions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn memberships(memberships: BitMask) -> Self {
        Self {
            memberships,
            ..Default::default()
        }
    }

    pub(crate) fn filter(filter: BitMask) -> Self {
        Self {
            filter,
            ..Default::default()
        }
    }

    pub(crate) fn sort(sort: bool) -> Self {
        Self {
            sort,
            ..Default::default()
        }
    }

    pub(crate) fn nudge(nudge: Option<f32>) -> Self {
        Self {
            nudge,
            ..Default::default()
        }
    }

    pub(crate) fn end(end: bool) -> Self {
        Self {
            end,
            ..Default::default()
        }
    }

    pub(crate) fn with_memberships(mut self, memberships: BitMask) -> Self {
        self.memberships = memberships;
        self
    }

    pub(crate) fn with_filter(mut self, filter: BitMask) -> Self {
        self.filter = filter;
        self
    }

    pub(crate) fn with_sort(mut self, sort: bool) -> Self {
        self.sort = sort;
        self
    }

    pub(crate) fn with_nudge(mut self, nudge: Option<f32>) -> Self {
        self.nudge = nudge;
        self
    }

    pub(crate) fn with_end(mut self, end: bool) -> Self {
        self.end = end;
        self
    }
}

pub(crate) fn trace_line<P>(
    scene: &mut Scene,
    ray_origin: P,
    ray_direction: Vec3,
    options: TraceOptions,
) -> Vec<Intersection>
where
    P: Into<Point3>,
{
    let ray_origin = ray_origin.into();

    fn inner(
        scene: &mut Scene,
        ray_origin: Point3,
        ray_direction: Vec3,
        options: TraceOptions,
    ) -> Vec<Intersection> {
        // LATER(perf) Smallvec instead? ArrayVec can discard intersections if it overflows. Other raycasts too.
        let mut intersections = Vec::new();
        let max_len = ray_direction.norm();

        scene.graph.physics.cast_ray(
            RayCastOptions {
                ray_origin: ray_origin,
                ray_direction,
                max_len,
                groups: InteractionGroups::new(options.memberships, options.filter),
                sort_results: options.sort,
            },
            &mut intersections,
        );

        let nudge = options.nudge.unwrap_or(0.01); // LATER Cvar
        for intersection in &mut intersections {
            intersection.position -= ray_direction.normalize() * nudge;
        }

        if options.end {
            intersections.push(Intersection {
                collider: Handle::NONE,
                normal: Vec3::zeros(),
                position: ray_origin + ray_direction,
                feature: FeatureId::Unknown,
                toi: max_len,
            });
        }
        intersections
    }

    inner(scene, ray_origin, ray_direction, options)
}
