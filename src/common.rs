pub(crate) mod net;

use std::fmt::{self, Debug, Formatter};

use rg3d::{
    core::{
        algebra::Vector3,
        pool::{Handle, Pool},
    },
    engine::{resource_manager::MaterialSearchOptions, Engine},
    physics3d::{rapier::prelude::*, RigidBodyHandle},
    resource::model::Model,
    scene::{node::Node, Scene},
};
use serde::{Deserialize, Serialize};

pub(crate) struct GameState {
    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// TODO d_speed, pause (don't forget integration_parameters.dt)
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

        let dir = Vector3::new(0.0, 0.0, 1.0); // TODO camera direction

        for cycle in &self.cycles {
            let input = &self.players[cycle.player_handle].input;
            if input.fire1 || input.fire2 {
                let wheel_accel = if input.fire1 {
                    dir * dt * 50.0
                } else {
                    -dir * dt * 50.0
                };
                let body = scene.physics.bodies.get_mut(&cycle.body_handle).unwrap();
                let mut linvel = *body.linvel();
                linvel += wheel_accel;
                body.set_linvel(linvel, true);
            }
        }
    }

    #[must_use]
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
        if let Some(index) = cycle_index {
            self.cycles.spawn_at(index, cycle).unwrap()
        } else {
            self.cycles.spawn(cycle)
        }
    }
}

#[derive(Debug)]
pub(crate) struct Player {
    pub(crate) input: Input,
    pub(crate) cycle_handle: Option<Handle<Cycle>>,
}

impl Player {
    pub(crate) fn new(cycle_handle: Option<Handle<Cycle>>) -> Self {
        Self {
            input: Input::default(),
            cycle_handle,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Cycle {
    pub(crate) node_handle: Handle<Node>,
    pub(crate) body_handle: RigidBodyHandle,
    pub(crate) player_handle: Handle<Player>,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct Input {
    // Some things like shooting might need the angle at the exact time
    // so we send pitch and yaw with each input.
    pub(crate) pitch: Deg,
    pub(crate) yaw: Deg,
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
        write!(f, "Input {{ pitch {}° yaw {}° ", self.pitch.0, self.yaw.0)?;
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

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ClientMessage {
    Input(Input),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ServerMessage {
    InitData(InitData),
    AddPlayer(AddPlayer),
    SpawnCycle(SpawnCycle),
    UpdatePhysics(UpdatePhysics),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InitData {
    pub(crate) player_cycles: Vec<PlayerCycle>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct AddPlayer {
    pub(crate) player_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SpawnCycle {
    pub(crate) player_cycle: PlayerCycle,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PlayerCycle {
    pub(crate) player_index: u32,
    pub(crate) cycle_index: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct UpdatePhysics {
    pub(crate) cycle_physics: Vec<CyclePhysics>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CyclePhysics {
    pub(crate) cycle_index: u32,
    pub(crate) translation: Vector3<f32>,
    pub(crate) velocity: Vector3<f32>,
}
