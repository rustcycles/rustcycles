//! Data and code shared between the client and server. Most gamelogic goes here.

pub(crate) mod entities;
pub(crate) mod messages;
pub(crate) mod net;
pub(crate) mod trace;

use std::fmt::{self, Debug, Display, Formatter};

use fyrox::scene::collider::InteractionGroups;
use rand::distributions::Uniform;
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

use crate::{
    common::entities::{Cycle, Player, PlayerState, Projectile},
    prelude::*,
};

/// The state of the game - all data needed to run the gamelogic.
pub(crate) struct GameState {
    pub(crate) gs_type: GameStateType,

    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// LATER d_speed, pause, configurable dt (don't forget integration_parameters.dt)
    /// LATER using f32 for time might lead to instability if a match is left running for a day or so
    pub(crate) game_time: f32,

    /// The previous gamelogic frame's time in seconds.
    pub(crate) game_time_prev: f32,

    /// Currently this is not synced between client and server,
    /// it's just a debugging aid (e.g. run something on odd/even frames).
    pub(crate) frame_number: usize,

    /// The RNG for all gamelogic
    ///
    /// TODO Should this even be here? Cl and sv will always desync
    /// because cl shouldn't run logic for all players (invisible should be culled).
    /// Even now they desync because they can shoot for a different number of frames
    /// (e.g. i think if input arrives a frame late to the server).
    pub(crate) rng: Xoshiro256PlusPlus,

    /// Inclusive range [-1.0, 1.0].
    /// Creating it once and saving it here might be faster than using gen_range according to docs.
    pub(crate) range_uniform11: Uniform<f64>,

    cycle_model: Model,

    pub(crate) scene_handle: Handle<Scene>,

    pub(crate) players: Pool<Player>,
    pub(crate) cycles: Pool<Cycle>,
    pub(crate) projectiles: Pool<Projectile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GameStateType {
    Server,
    Client,
    Shared,
}

/// All data necessary to run a frame of shared gamelogic in one convenient package.
///
/// It could be all passed as separate arguments to the functions that need it,
/// but this is more ergonomic.
///
/// This struct and {Client,Server}FrameData are meant to have impl blocks throughout the codebase.
pub(crate) struct FrameData<'a> {
    pub(crate) cvars: &'a Cvars,
    // We have two choices here:
    // 1) Borrow the whole engine
    // 2) Borrow only the scene
    // Option 1 lets us access things like the UI and resource manager
    // but it's also easy to do bad stuff like calling update() twice per frame
    // and we have to borrow the scene in every method.
    // Option 2 lets us have separate client and server scenes
    // and easily pass the one we want to update.
    // So currently we only borrow the scene and {Client,Server}Process deal with the engine.
    // We could also borrow the UI and the resource manager here too if needed.
    pub(crate) scene: &'a mut Scene,
    pub(crate) gs: &'a mut GameState,
}

impl GameState {
    pub(crate) async fn new(cvars: &Cvars, engine: &mut Engine, gs_type: GameStateType) -> Self {
        let mut scene = Scene::new();

        engine
            .resource_manager
            .request_model("data/arena/arena.rgs")
            .await
            .unwrap()
            .instantiate(&mut scene);

        let cycle_model = engine
            .resource_manager
            .request_model("data/rustcycle/rustcycle.fbx")
            .await
            .unwrap();

        let scene_handle = engine.scenes.add(scene);

        Self {
            gs_type,
            game_time: 0.0,
            // We wanna avoid having to specialcase divisions by zero in the first frame.
            // It would usually be 0.0 / 0.0 anyway so now it's 0.0 / -1.0.
            game_time_prev: -1.0,
            frame_number: 0,
            rng: Xoshiro256PlusPlus::seed_from_u64(cvars.d_seed),
            range_uniform11: Uniform::new_inclusive(-1.0, 1.0),
            cycle_model,
            scene_handle,
            players: Pool::new(),
            cycles: Pool::new(),
            projectiles: Pool::new(),
        }
    }
}

impl FrameData<'_> {
    pub(crate) fn tick_before_physics(&mut self, dt: f32) {
        self.scene.graph.physics.integration_parameters.max_ccd_substeps =
            self.cvars.g_physics_max_ccd_substeps;

        for cycle in &mut self.gs.cycles {
            let player = &self.gs.players[cycle.player_handle];

            let playing = player.ps == PlayerState::Playing;
            let input = player.input;
            let rot = input.yaw_rotation();
            let body = self.scene.graph[cycle.body_handle].as_rigid_body_mut();
            if playing {
                let forward = rot * FORWARD;
                let left = rot * LEFT;

                let mut wheel_accel = Vec3::zeros();
                if input.forward {
                    wheel_accel += forward * dt * self.cvars.g_wheel_acceleration;
                }
                if input.backward {
                    wheel_accel -= forward * dt * self.cvars.g_wheel_acceleration;
                }
                if input.left {
                    wheel_accel += left * dt * self.cvars.g_wheel_acceleration;
                }
                if input.right {
                    wheel_accel -= left * dt * self.cvars.g_wheel_acceleration;
                }

                let mut lin_vel = body.lin_vel();
                lin_vel += wheel_accel;
                body.set_lin_vel(lin_vel);
            }

            // LATER Does this allow clipping into geometry? Yes.
            //  Use an impulse proportional to mouse movement instead?
            //  https://www.rapier.rs/docs/user_guides/rust/rigid_bodies/#forces-and-impulses
            body.local_transform_mut().set_rotation(rot);

            if input.fire1
                && cycle.time_last_fired + self.cvars.g_projectile_refire < self.gs.game_time
            {
                let dir = input.look_rotation() * FORWARD;
                let forward = dir * self.cvars.g_projectile_speed;
                let rand = Vec3::new(
                    self.gs.rng.sample(StandardNormal),
                    self.gs.rng.sample(StandardNormal),
                    self.gs.rng.sample(StandardNormal),
                );
                dbg_logd!(rand); // To showcase desyncs between cl and sv
                let spread = rand * self.cvars.g_projectile_spread;

                let _ = self.gs.projectiles.spawn(Projectile {
                    player_handle: cycle.player_handle,
                    pos: **body.local_transform().position(),
                    vel: forward + spread,
                    time_fired: self.gs.game_time,
                });

                cycle.time_last_fired = self.gs.game_time;
            }
        }

        // LATER Split into functions
        let mut free = Vec::new();
        for (proj_handle, proj) in self.gs.projectiles.pair_iter_mut() {
            if proj.time_fired + self.cvars.g_projectile_lifetime < self.gs.game_time {
                free.push(proj_handle);
                continue;
            }

            let step = proj.vel * dt;

            let hits = trace_line(self.cvars, self.scene, proj.pos, step, Default::default());
            for hit in hits {
                let cycle_handle = self.gs.players[proj.player_handle].cycle_handle.unwrap();
                let cycle_collider_handle = self.gs.cycles[cycle_handle].collider_handle;
                if hit.collider == cycle_collider_handle {
                    // LATER Let the player shoot himself - enable self collision after the projectile clears the player's hitbox.
                    continue;
                }

                // Free projectile
                dbg_cross!(hit.position.coords, 0.5);
                free.push(proj_handle);
                break;
            }

            let step_norm = step.normalize();
            dbg_arrow!(proj.pos - step_norm, step_norm, 0.0);

            proj.pos += step;
        }
        for handle in free {
            self.gs.projectiles.free(handle);
        }

        dbg_textf!("Projectiles: {}", self.gs.projectiles.total_count());

        // Testing
        for player in &self.gs.players {
            let look_rot = player.input.look_rotation();
            dbg_rot!(v!(0 7 0), look_rot);
            dbg_arrow!(v!(0 5 0), look_rot * FORWARD, 0.5);

            let yaw_rot = player.input.yaw_rotation();
            dbg_arrow!(v!(0 3 0), yaw_rot * FORWARD, 0.5);
        }
    }

    pub(crate) fn free_player(&mut self, player_handle: Handle<Player>) {
        let player = self.gs.players.free(player_handle);
        if let Some(handle) = player.cycle_handle {
            let cycle = self.gs.cycles.free(handle);
            self.scene.graph.remove_node(cycle.body_handle);
        }
    }

    pub(crate) fn spawn_cycle(
        &mut self,
        player_handle: Handle<Player>,
        cycle_index: Option<u32>,
    ) -> Handle<Cycle> {
        let node_handle = self.gs.cycle_model.instantiate(self.scene);
        let collider_handle = ColliderBuilder::new(BaseBuilder::new())
            // Size manually copied from the result of rusty-editor's Fit Collider
            // LATER Remove rustcycle.rgs?
            .with_shape(ColliderShape::cuboid(0.125, 0.271, 0.271))
            .with_collision_groups(InteractionGroups::new(IG_ENTITIES, IG_ALL))
            .build(&mut self.scene.graph);
        // Slightly randomize spawn pos just to use the RNG
        let left = 3.0 * self.gs.rng.sample(self.gs.range_uniform11);
        dbg_logd!(left);
        let body_handle = RigidBodyBuilder::new(
            BaseBuilder::new()
                .with_local_transform(
                    TransformBuilder::new().with_local_position(v!(left, 5, 0)).build(),
                )
                .with_children(&[node_handle, collider_handle]),
        )
        .with_ccd_enabled(true)
        .with_locked_rotations(true)
        .with_can_sleep(false)
        .build(&mut self.scene.graph);

        let cycle = Cycle {
            player_handle,
            body_handle,
            collider_handle,
            time_last_fired: 0.0,
        };
        let cycle_handle = if let Some(index) = cycle_index {
            self.gs.cycles.spawn_at(index, cycle).unwrap()
        } else {
            self.gs.cycles.spawn(cycle)
        };

        self.gs.players[player_handle].cycle_handle = Some(cycle_handle);

        cycle_handle
    }

    /// Draw arrows in a different orientation every frame.
    ///
    /// This helps:
    /// - Notice issues with framerate
    /// - Notice tearing (but a solid object would make it even easier to see)
    /// - Make sure debug draws and prints issued on one frame happen on the same frame.
    ///   The intended usecase is to take a screenshot and compare
    ///   the direction of the arrow to the direction as text.
    ///
    /// The rotation is clockwise when looking in the forward direction.
    pub(crate) fn debug_engine_updates(&self, pos: Vec3) {
        if !self.cvars.d_draw || !self.cvars.d_draw_frame_timings {
            return;
        }

        let step = (self.gs.frame_number % self.cvars.d_draw_frame_timings_steps) as f32;
        let angle =
            2.0 * std::f32::consts::PI / self.cvars.d_draw_frame_timings_steps as f32 * step;
        let rot = UnitQuaternion::from_axis_angle(&FORWARD_AXIS, angle);
        let dir = rot * UP;
        dbg_arrow!(pos, dir);
        if self.cvars.d_draw_frame_timings_text {
            dbg_textd!(self.gs.frame_number, pos, angle.to_degrees());
        }
    }
}

// LATER Would be nice to send as little as possible since this is networked.
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub(crate) struct Input {
    /// LATER This should probably never be networked, since cl and sv have different time.
    /// LATER Default impl - should not be 0 if it's not the first match
    ///       in the same process - remove this entirely?
    pub(crate) real_time: f32,

    /// LATER specify whether this is the current or prev or next frame,
    /// it might get messy depending on when input vs timekeeping is done
    /// and when it's sent.
    pub(crate) game_time: f32,

    /// Counterclockwise: 0 is directly forward, negative is left, positive right.
    ///
    /// Nalgebra rotations follow the right hand rule,
    /// thumb points in +Y (up), the curl of fingers shows direction.
    ///
    /// Some things like shooting need the angle at the exact time
    /// so we send yaw and pitch with each input, not just once per frame.
    pub(crate) yaw: Deg,
    pub(crate) yaw_speed: Deg,
    pub(crate) pitch: Deg,
    pub(crate) pitch_speed: Deg,

    pub(crate) fire1: bool,
    pub(crate) fire2: bool,
    pub(crate) marker1: bool,
    pub(crate) marker2: bool,
    pub(crate) zoom: bool,
    pub(crate) forward: bool,
    pub(crate) backward: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) prev_weapon: bool,
    pub(crate) next_weapon: bool,
    pub(crate) reload: bool,
    pub(crate) flag: bool,
    pub(crate) grenade: bool,
    pub(crate) map: bool,
    pub(crate) score: bool,
    pub(crate) chat: bool,
    pub(crate) pause: bool,
    pub(crate) screenshot: bool,
    // ^ when adding fields, also add them to other impls and functions below
}

// LATER ClientInput? - zoom, map, chat, score, pause, screenshot, console, ...
// These don't need to be networked

impl Input {
    pub(crate) fn release_all_keys(&mut self) {
        self.fire1 = false;
        self.fire2 = false;
        self.marker1 = false;
        self.marker2 = false;
        self.zoom = false;
        self.forward = false;
        self.backward = false;
        self.left = false;
        self.right = false;
        self.up = false;
        self.down = false;
        self.prev_weapon = false;
        self.next_weapon = false;
        self.reload = false;
        self.flag = false;
        self.grenade = false;
        self.map = false;
        self.score = false;
        self.chat = false;
        self.pause = false;
        self.screenshot = false;
    }

    pub(crate) fn look_rotation(&self) -> UnitQuaternion<f32> {
        let yaw = self.yaw_rotation();

        let pitch_axis = yaw * LEFT_AXIS;
        let pitch = UnitQuaternion::from_axis_angle(&pitch_axis, self.pitch.to_radians());

        pitch * yaw
    }

    pub(crate) fn yaw_rotation(&self) -> UnitQuaternion<f32> {
        UnitQuaternion::from_axis_angle(&UP_AXIS, self.yaw.to_radians())
    }
}

impl Debug for Input {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Only output the pressed buttons so it's more readable.
        write!(
            f,
            "Input {{ time {} yaw {} {}/s pitch {} {}/s ",
            self.game_time, self.yaw, self.yaw_speed, self.pitch, self.pitch_speed,
        )?;
        if self.fire1 {
            write!(f, "fire1 ")?;
        }
        if self.fire2 {
            write!(f, "fire2 ")?;
        }
        if self.marker1 {
            write!(f, "marker1 ")?;
        }
        if self.marker2 {
            write!(f, "marker2 ")?;
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
        if self.up {
            write!(f, "up ")?;
        }
        if self.down {
            write!(f, "down ")?;
        }
        if self.prev_weapon {
            write!(f, "prev_weapon ")?;
        }
        if self.next_weapon {
            write!(f, "next_weapon ")?;
        }
        if self.reload {
            write!(f, "reload ")?;
        }
        if self.flag {
            write!(f, "flag ")?;
        }
        if self.grenade {
            write!(f, "grenade ")?;
        }
        if self.map {
            write!(f, "map ")?;
        }
        if self.score {
            write!(f, "score ")?;
        }
        if self.chat {
            write!(f, "chat ")?;
        }
        if self.pause {
            write!(f, "pause ")?;
        }
        if self.screenshot {
            write!(f, "screenshot ")?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}

// Why not use an existing crate like https://crates.io/crates/angle?
// - Not worth adding a dep for such a simple thing
// - It shows signs of lack of attention to detail
//   (bad readme, missing doc comments, inconsistent formatting)
//   on the surface so it probably contains deeper issues as well.
// - It tries to be smart and implements questionable ops such as comparisons.
// This reasoning might change if this struct gets larger but it'll probably mean
// only taking inspiration and bits of code from the angle crate, not adding it as a dep.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
pub(crate) struct Deg(pub(crate) f32);

impl Deg {
    pub(crate) fn to_radians(self) -> f32 {
        self.0.to_radians()
    }
}

impl Display for Deg {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}°", self.0)
    }
}
