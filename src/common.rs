//! Data and code shared between the client and server. Most gamelogic goes here.

pub(crate) mod entities;
pub(crate) mod messages;
pub(crate) mod net;

use std::fmt::{self, Debug, Formatter};

use serde::{Deserialize, Serialize};

use crate::{
    common::entities::{Cycle, Player, PlayerState},
    prelude::*,
};

/// The state of the game - all data needed to run the gamelogic.
pub(crate) struct GameState {
    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// LATER d_speed, pause, configurable dt (don't forget integration_parameters.dt)
    /// LATER using f32 for time might lead to instability if a match is left running for a day or so
    pub(crate) game_time: f32,
    /// Currently this is not synced between client and server,
    /// it's just a debugging aid (e.g. run something on odd/even frames).
    pub(crate) frame_number: usize,
    pub(crate) scene: Handle<Scene>,
    cycle_model: Model,
    pub(crate) players: Pool<Player>,
    pub(crate) cycles: Pool<Cycle>,
}

impl GameState {
    pub(crate) async fn new(engine: &mut Engine) -> Self {
        let mut scene = Scene::new();
        // This is needed because the default 1 causes the wheel to randomly stutter/stop
        // when passing between poles - they use a single trimesh collider.
        // 2 is very noticeable, 5 is better, 10 is only noticeable at high speeds.
        // It never completely goes away, even with 100.
        // TODO rg3d 0.24 broke this
        //scene.physics.integration_parameters.max_ccd_substeps = 100;
        // LATER allow changing scene.physics.integration_parameters.dt ?

        engine
            .resource_manager
            .request_model("data/arena/arena.rgs")
            .await
            .unwrap()
            .instantiate_geometry(&mut scene);

        let cycle_model = engine
            .resource_manager
            .request_model("data/rustcycle/rustcycle.fbx")
            .await
            .unwrap();

        let scene = engine.scenes.add(scene);

        Self {
            game_time: 0.0,
            frame_number: 0,
            scene,
            cycle_model,
            players: Pool::new(),
            cycles: Pool::new(),
        }
    }

    pub(crate) fn tick_before_physics(&mut self, engine: &mut Engine, dt: f32) {
        let scene = &mut engine.scenes[self.scene];

        for cycle in &self.cycles {
            let player = &self.players[cycle.player_handle];

            let playing = player.ps == PlayerState::Playing;
            let input = player.input;
            let rot = UnitQuaternion::from_axis_angle(&Vec3::up_axis(), input.yaw.to_radians());
            let body = scene.graph[cycle.body_handle].as_rigid_body_mut();
            if playing {
                let forward = rot * Vec3::forward();
                let left = rot * Vec3::left();

                let mut wheel_accel = Vec3::zeros();
                if input.forward {
                    wheel_accel += forward * dt * 20.0;
                }
                if input.backward {
                    wheel_accel -= forward * dt * 20.0;
                }
                if input.left {
                    wheel_accel += left * dt * 20.0;
                }
                if input.right {
                    wheel_accel -= left * dt * 20.0;
                }

                let mut lin_vel = body.lin_vel();
                lin_vel += wheel_accel;
                body.set_lin_vel(lin_vel);
            }
            let dir = rot * Vec3::forward();
            dbg_arrow!(v!(0 3 0), dir.normalize(), 0.5);
            dbg_textd!(rot, dir);
            dbg_textf!("rot {rot}, dir {dir:?}");

            // LATER Does this allow clipping into geometry?
            //  Use an impulse proportional to mouse movement instead?
            //  https://www.rapier.rs/docs/user_guides/rust/rigid_bodies/#forces-and-impulses
            // FIXME this is broken
            //body.local_transform_mut().set_rotation(rot);
            body.local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(
                    &Vec3::up_axis(),
                    self.game_time,
                ));
        }
    }

    pub(crate) fn free_player(&mut self, scene: &mut Scene, player_handle: Handle<Player>) {
        let player = self.players.free(player_handle);
        if let Some(handle) = player.cycle_handle {
            let cycle = self.cycles.free(handle);
            scene.remove_node(cycle.body_handle);
        }
    }

    pub(crate) fn spawn_cycle(
        &mut self,
        scene: &mut Scene,
        player_handle: Handle<Player>,
        cycle_index: Option<u32>,
    ) -> Handle<Cycle> {
        let node_handle = self.cycle_model.instantiate_geometry(scene);
        let collider_handle = ColliderBuilder::new(BaseBuilder::new())
            // Size manually copied from the result of rusty-editor's Fit Collider
            // LATER Remove rustcycle.rgs?
            .with_shape(ColliderShape::cuboid(0.125, 0.271, 0.271))
            .build(&mut scene.graph);
        let body_handle = RigidBodyBuilder::new(
            BaseBuilder::new()
                .with_local_transform(
                    TransformBuilder::new()
                        .with_local_position(v!(-1 5 0))
                        .build(),
                )
                .with_children(&[node_handle, collider_handle]),
        )
        .with_ccd_enabled(true)
        .with_locked_rotations(true)
        .with_can_sleep(false)
        .build(&mut scene.graph);

        let cycle = Cycle {
            body_handle,
            player_handle,
        };
        let cycle_handle = if let Some(index) = cycle_index {
            self.cycles.spawn_at(index, cycle).unwrap()
        } else {
            self.cycles.spawn(cycle)
        };

        self.players[player_handle].cycle_handle = Some(cycle_handle);

        cycle_handle
    }
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct Input {
    // Some things like shooting might need the angle at the exact time
    // so we send yaw and pitch with each input.
    pub(crate) yaw: Deg,
    pub(crate) pitch: Deg,
    pub(crate) fire1: bool,
    pub(crate) fire2: bool,
    pub(crate) zoom: bool,
    pub(crate) forward: bool,
    pub(crate) backward: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    // ^ when adding fields, also add them to Debug
}

impl Debug for Input {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Only output the pressed buttons so it's more readable.
        write!(f, "Input {{ yaw {}° pitch {}° ", self.yaw.0, self.pitch.0)?;
        if self.fire1 {
            write!(f, "fire1 ")?;
        }
        if self.fire2 {
            write!(f, "fire2 ")?;
        }
        if self.zoom {
            write!(f, "zoom ")?;
        }
        if self.forward {
            write!(f, "forward ")?;
        }
        if self.backward {
            write!(f, "backward ")?;
        }
        if self.left {
            write!(f, "left ")?;
        }
        if self.right {
            write!(f, "right ")?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub(crate) struct Deg(pub(crate) f32);

impl Deg {
    pub(crate) fn to_radians(self) -> f32 {
        self.0.to_radians()
    }
}
