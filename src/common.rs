//! Data and code shared between the client and server. Most gamelogic goes here.

pub(crate) mod entities;
pub(crate) mod messages;
pub(crate) mod net;

use std::fmt::{self, Debug, Formatter};

use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        pool::{Handle, Pool},
    },
    engine::{resource_manager::MaterialSearchOptions, Engine},
    physics3d::rapier::prelude::*,
    resource::model::Model,
    scene::Scene,
};
use serde::{Deserialize, Serialize};

use crate::common::entities::{Cycle, Player, PlayerState};

/// The state of the game - all data needed to run the gamelogic.
pub(crate) struct GameState {
    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// LATER d_speed, pause, configurable dt (don't forget integration_parameters.dt)
    /// LATER using f32 for time might lead to instability if a match is left running for a day or so
    pub(crate) game_time: f32,
    pub(crate) scene: Handle<Scene>,
    cycle_model: Model,
    pub(crate) players: Pool<Player>,
    pub(crate) cycles: Pool<Cycle>,
}

impl GameState {
    pub(crate) async fn new(engine: &mut Engine) -> Self {
        let mut scene = Scene::new();
        // This is needed because the default 1 causes the wheel to randomly stutter/stop
        // when just sliding on completely smooth floor. The higher the value, the less it slows down.
        // 2 is very noticeable, 5 is better, 10 is only noticeable at high speeds.
        // It never completely goes away, even with 100.
        // NOTE: It might not actually be the floor that's causing it,
        // it seems to happen when passing between poles.
        // LATER Maybe there is a way to solve this by filtering collisions with the floor?
        scene.physics.integration_parameters.max_ccd_substeps = 100;
        // LATER allow changing scene.physics.integration_parameters.dt ?

        engine
            .resource_manager
            .request_model(
                "data/arena/arena.rgs",
                MaterialSearchOptions::UsePathDirectly,
            )
            .await
            .unwrap()
            .instantiate_geometry(&mut scene);

        let cycle_model = engine
            .resource_manager
            .request_model(
                "data/rustcycle/rustcycle.fbx",
                MaterialSearchOptions::RecursiveUp,
            )
            .await
            .unwrap();

        let scene = engine.scenes.add(scene);

        Self {
            game_time: 0.0,
            scene,
            cycle_model,
            players: Pool::new(),
            cycles: Pool::new(),
        }
    }

    pub(crate) fn tick(&mut self, engine: &mut Engine, dt: f32) {
        let scene = &mut engine.scenes[self.scene];

        for cycle in &self.cycles {
            let player = &self.players[cycle.player_handle];
            if player.ps != PlayerState::Playing {
                continue;
            }

            let input = player.input;
            let rot = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), input.yaw.to_radians());
            let body = scene.physics.bodies.get_mut(&cycle.body_handle).unwrap();
            if input.forward || input.backward {
                let dir = rot * Vector3::z();
                let wheel_accel = if input.forward {
                    dir * dt * 50.0
                } else {
                    -dir * dt * 50.0
                };

                let mut linvel = *body.linvel();
                linvel += wheel_accel;
                body.set_linvel(linvel, true);
            }
            // FIXME WTF???
            if input.forward || input.backward {
                // LATER Does this allow clipping into geometry?
                //  Use an impulse proportional to mouse movement instead?
                //  https://www.rapier.rs/docs/user_guides/rust/rigid_bodies/#forces-and-impulses
                body.set_rotation(rot.scaled_axis(), true);
            }
        }
    }

    pub(crate) fn free_player(&mut self, scene: &mut Scene, player_handle: Handle<Player>) {
        let player = self.players.free(player_handle);
        if let Some(handle) = player.cycle_handle {
            let cycle = self.cycles.free(handle);
            scene.remove_node(cycle.node_handle);
        }
    }

    pub(crate) fn spawn_cycle(
        &mut self,
        scene: &mut Scene,
        player_handle: Handle<Player>,
        cycle_index: Option<u32>,
    ) -> Handle<Cycle> {
        let node_handle = self.cycle_model.instantiate_geometry(scene);
        let body_handle = scene.physics.add_body(
            RigidBodyBuilder::new_dynamic()
                .ccd_enabled(true)
                .lock_rotations()
                .translation(Vector3::new(-1.0, 5.0, 0.0))
                .build(),
        );
        scene.physics.add_collider(
            // Size manually copied from the result of rusty-editor's Fit Collider
            // LATER Remove rustcycle.rgs?
            ColliderBuilder::cuboid(0.125, 0.271, 0.271).build(),
            &body_handle,
        );
        scene.physics_binder.bind(node_handle, body_handle);

        let cycle = Cycle {
            node_handle,
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
