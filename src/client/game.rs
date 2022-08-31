//! Client-side gamelogic.
//!
//! Mainly receiving updates from the server and updating local state.

use std::{io::ErrorKind, thread, time::Duration};

use fyrox::{
    gui::{message::MessageDirection, text::TextMessage, UiNode},
    scene::camera::{CameraBuilder, Projection, SkyBoxBuilder},
};

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{
            AddPlayer, ClientMessage, CyclePhysics, Init, PlayerCycle, PlayerInput,
            PlayerProjectile, ServerMessage, Update,
        },
        net::{self, Connection},
        GameState, Input,
    },
    cvars::Cvars,
    debug::{
        self,
        details::{Lines, DEBUG_SHAPES, DEBUG_TEXTS},
    },
    prelude::*,
};

/// Game data inside a client process.
///
/// Needs to be connected to a game Server to play. Contains a local copy of the game state
/// which might not be entirely accurate due to network lag and packet loss.
pub(crate) struct ClientGame {
    debug_text: Handle<UiNode>,
    pub(crate) gs: GameState,
    pub(crate) lp: LocalPlayer,
    pub(crate) camera: Handle<Node>,
    conn: Box<dyn Connection>,
}

impl ClientGame {
    pub(crate) async fn new(
        engine: &mut Engine,
        debug_text: Handle<UiNode>,
        mut conn: Box<dyn Connection>,
    ) -> Self {
        let mut gs = GameState::new(engine).await;

        // LATER Load everything in parallel (i.e. with GameState)
        // LATER Report error if loading fails
        let top = engine.resource_manager.request_texture("data/skybox/top.png").await.ok();

        let scene = &mut engine.scenes[gs.scene];

        let camera =
            CameraBuilder::new(BaseBuilder::new().with_local_transform(
                TransformBuilder::new().with_local_position(v!(0 1 -3)).build(),
            ))
            .with_skybox(
                SkyBoxBuilder {
                    front: None,
                    back: None,
                    left: None,
                    right: None,
                    top,
                    bottom: None,
                }
                .build()
                .unwrap(),
            )
            .build(&mut scene.graph);

        let mut init_attempts = 0;
        let lp = loop {
            init_attempts += 1;
            let (msg, closed) = conn.receive_one_sm();
            if closed {
                panic!("connection closed before init"); // LATER Don't crash
            }
            if let Some(msg) = msg {
                if let ServerMessage::Init(Init {
                    player_indices,
                    local_player_index,
                    player_cycles,
                    player_projectiles,
                }) = msg
                {
                    for player_index in player_indices {
                        let player = Player::new(None);
                        gs.players.spawn_at(player_index, player).unwrap();
                    }
                    let local_player_handle = gs.players.handle_from_index(local_player_index);
                    let lp = LocalPlayer::new(local_player_handle);

                    for PlayerCycle {
                        player_index,
                        cycle_index,
                    } in player_cycles
                    {
                        let player_handle = gs.players.handle_from_index(player_index);
                        gs.spawn_cycle(scene, player_handle, Some(cycle_index));
                    }

                    for PlayerProjectile {
                        player_index: _,
                        projectile_index: _,
                    } in player_projectiles
                    {
                        todo!("init projectiles");
                    }

                    dbg_logf!("init attempts: {}", init_attempts);
                    break lp;
                } else {
                    panic!("First message wasn't init"); // LATER Don't crash
                }
            }
            if init_attempts % 100 == 0 {
                dbg_logf!("init attempts: {}", init_attempts);
            }
            thread::sleep(Duration::from_millis(10));
        };
        dbg_logf!("local_player_index is {}", lp.player_handle.index());

        Self {
            debug_text,
            gs,
            lp,
            camera,
            conn,
        }
    }

    pub(crate) fn update(&mut self, cvars: &Cvars, engine: &mut Engine, game_time_target: f32) {
        // LATER read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            self.gs.frame_number += 1;

            self.tick_begin_frame(engine);

            self.gs.tick_before_physics(engine, dt);

            self.tick_before_physics(engine, dt);

            // Update animations, transformations, physics, ...
            // Dummy control flow since we don't use fyrox plugins.
            let mut cf = fyrox::event_loop::ControlFlow::Poll;
            engine.pre_update(dt, &mut cf);
            assert_eq!(cf, fyrox::event_loop::ControlFlow::Poll);

            // `tick_after_physics` tells the engine to draw debug shapes and text.
            // Any debug calls after it will show up next frame.
            if cvars.d_draw && cvars.d_draw_arrows_frame {
                // LATER put cvars inside
                self.gs.debug_engine_updates(v!(-5 3 3), 4);
            }
            self.tick_after_physics(cvars, engine, dt);
            if cvars.d_draw && cvars.d_draw_arrows_frame {
                self.gs.debug_engine_updates(v!(-6 3 3), 4);
            }

            // Update UI
            engine.post_update(dt);
        }

        engine.get_window().request_redraw();
    }

    pub(crate) fn send_input(&mut self) {
        self.network_send(ClientMessage::Input(self.lp.input));
    }

    /// All once-per-frame networking.
    fn tick_begin_frame(&mut self, engine: &mut Engine) {
        // LATER Always send key/mouse presses immediately
        // but maybe rate-limit mouse movement updates
        // in case some systems update mouse position at a very high rate.
        self.lp.input_prev = self.lp.input;

        self.lp.input.yaw.0 += self.lp.delta_yaw; // LATER Normalize to [0, 360°) or something
        self.lp.input.pitch.0 = (self.lp.input.pitch.0 + self.lp.delta_pitch).clamp(-90.0, 90.0);

        let delta_time = self.gs.game_time - self.gs.game_time_prev;
        soft_assert!(delta_time > 0.0);
        self.lp.input.yaw_speed.0 = self.lp.delta_yaw / delta_time;
        self.lp.input.pitch_speed.0 = self.lp.delta_pitch / delta_time;

        self.lp.delta_yaw = 0.0;
        self.lp.delta_pitch = 0.0;

        self.send_input();

        let scene = &mut engine.scenes[self.gs.scene];

        scene.drawing_context.clear_lines();

        let (msgs, _) = self.conn.receive_sm();
        for msg in msgs {
            match msg {
                ServerMessage::Init(_) => {
                    // LATER Make this type safe? Init part of handshake?
                    panic!("Received unexpected init")
                }
                ServerMessage::AddPlayer(AddPlayer { player_index, name }) => {
                    let player = Player::new(None);
                    self.gs.players.spawn_at(player_index, player).unwrap();
                    dbg_logd!("player {} added", name);
                }
                ServerMessage::RemovePlayer { player_index } => {
                    let player_handle = self.gs.players.handle_from_index(player_index);
                    self.gs.free_player(scene, player_handle);
                }
                ServerMessage::Observe { player_index } => {
                    self.gs.players.at_mut(player_index).unwrap().ps = PlayerState::Observing;
                    dbg_logf!("player {} is now observing", player_index);
                }
                ServerMessage::Spectate {
                    player_index,
                    spectatee_index,
                } => {
                    let spectatee_handle = self.gs.players.handle_from_index(spectatee_index);
                    self.gs.players.at_mut(player_index).unwrap().ps =
                        PlayerState::Spectating { spectatee_handle };
                    dbg_logf!(
                        "player {} is now spectating player {}",
                        player_index,
                        spectatee_index
                    );
                }
                ServerMessage::Join { player_index } => {
                    self.gs.players.at_mut(player_index).unwrap().ps = PlayerState::Playing;
                    dbg_logf!("player {} is now playing", player_index);
                }
                ServerMessage::SpawnCycle(PlayerCycle {
                    player_index,
                    cycle_index,
                }) => {
                    let player_handle = self.gs.players.handle_from_index(player_index);
                    self.gs.spawn_cycle(scene, player_handle, Some(cycle_index));
                }
                ServerMessage::DespawnCycle { cycle_index } => {
                    dbg_logd!(cycle_index);
                    todo!("despawn cycle");
                }
                ServerMessage::Update(Update {
                    player_inputs,
                    cycle_physics,
                    debug_texts,
                    debug_shapes,
                }) => {
                    for PlayerInput {
                        player_index,
                        input,
                    } in player_inputs
                    {
                        self.gs.players.at_mut(player_index).unwrap().input = input;
                    }

                    for CyclePhysics {
                        cycle_index,
                        translation,
                        rotation,
                        velocity,
                    } in cycle_physics
                    {
                        let cycle = self.gs.cycles.at_mut(cycle_index).unwrap();
                        let body = scene.graph[cycle.body_handle].as_rigid_body_mut();
                        body.local_transform_mut().set_position(translation);
                        body.local_transform_mut().set_rotation(rotation);
                        body.set_lin_vel(velocity);
                    }

                    DEBUG_TEXTS.with(|texts| {
                        let mut texts = texts.borrow_mut();
                        texts.extend(debug_texts);
                    });

                    DEBUG_SHAPES.with(|shapes| {
                        let mut shapes = shapes.borrow_mut();
                        shapes.extend(debug_shapes);
                    })
                }
            }
        }
    }

    fn tick_before_physics(&mut self, engine: &mut Engine, dt: f32) {
        // Join / spec
        let ps = self.gs.players[self.lp.player_handle].ps;
        if ps == PlayerState::Observing && self.lp.input.fire1 {
            self.network_send(ClientMessage::Join);
        } else if ps == PlayerState::Playing && self.lp.input.fire2 {
            self.network_send(ClientMessage::Observe);
        }

        let scene = &mut engine.scenes[self.gs.scene];

        let player_cycle_handle = self.gs.players[self.lp.player_handle].cycle_handle.unwrap();
        let player_body_handle = self.gs.cycles[player_cycle_handle].body_handle;
        let player_cycle_pos = **scene.graph[player_body_handle].local_transform().position();

        let camera = &mut scene.graph[self.camera];

        // Camera turning
        let yaw_angle = self.lp.input.yaw.0.to_radians();
        let yaw = UnitQuaternion::from_axis_angle(&UP_AXIS, yaw_angle);

        let pitch_angle = self.lp.input.pitch.0.to_radians();
        let pitch_axis = yaw * LEFT_AXIS;
        let pitch = UnitQuaternion::from_axis_angle(&pitch_axis, pitch_angle);

        let cam_rot = pitch * yaw;
        camera.local_transform_mut().set_rotation(cam_rot);

        dbg_rot!(v!(0 7 0), cam_rot);
        dbg_arrow!(v!(0 5 0), cam_rot * FORWARD);

        // Camera movement
        let mut camera_pos = **camera.local_transform().position();
        if ps == PlayerState::Observing {
            let forward = camera.forward_vec_normed();
            let left = camera.left_vec_normed();
            let up = camera.up_vec_normed();
            let camera_speed = 10.0;
            if self.lp.input.forward {
                camera_pos += forward * dt * camera_speed;
            }
            if self.lp.input.backward {
                camera_pos += -forward * dt * camera_speed;
            }
            if self.lp.input.left {
                camera_pos += left * dt * camera_speed;
            }
            if self.lp.input.right {
                camera_pos += -left * dt * camera_speed;
            }
            if self.lp.input.up {
                camera_pos += up * dt * camera_speed;
            }
            if self.lp.input.down {
                camera_pos += -up * dt * camera_speed;
            }
        } else if ps == PlayerState::Playing {
            // LATER cvars
            let back = -(cam_rot * FORWARD * 2.0);
            let up = UP * 0.5;
            camera_pos = player_cycle_pos + back + up;
        }
        camera.local_transform_mut().set_position(camera_pos);

        // Camera zoom
        let camera = camera.as_camera_mut();
        if let Projection::Perspective(perspective) = camera.projection_mut() {
            // LATER cvar
            if self.lp.input.zoom {
                perspective.fov = 20.0_f32.to_radians();
            } else {
                perspective.fov = 75.0_f32.to_radians();
            }
        } else {
            unreachable!();
        }

        // Testing
        for cycle in &self.gs.cycles {
            let body_pos = scene.graph[cycle.body_handle].global_position();
            dbg_cross!(body_pos, 3.0);
        }

        // Examples of all the debug shapes

        dbg_line!(v!(25 5 5), v!(25 5 7));

        dbg_arrow!(v!(20 5 5), v!(0 0 2)); // Forward
        dbg_arrow!(v!(20 5 5), v!(0 0 -1)); // Back
        dbg_arrow!(v!(20 5 5), v!(-1 0 0)); // Left
        dbg_arrow!(v!(20 5 5), v!(1 0 0)); // Right
        dbg_arrow!(v!(20 5 5), v!(0 1 0)); // Up
        dbg_arrow!(v!(20 5 5), v!(0 -1 0)); // Down

        dbg_arrow!(v!(20 10 5), v!(1 1 1), 0.0, BLUE);
        dbg_arrow!(v!(20 10 6), v!(2 2 2), 0.0, BLUE2);

        dbg_cross!(v!(15 5 5), 0.0, CYAN);

        dbg_rot!(v!(10 5 5), UnitQuaternion::default());

        dbg_arrow!(v!(15 10 5), v!(0 0 2), 0.0, GREEN);
        dbg_arrow!(v!(15 10 5), v!(0 0.01 2), 0.0, RED);
        dbg_arrow!(v!(15 11 5), v!(0 0 2), 0.0, RED);
        dbg_arrow!(v!(15 11 5), v!(0 0.01 2), 0.0, GREEN);
        dbg_arrow!(v!(15 12 5), v!(0 0 2), 0.0, RED);
        dbg_arrow!(v!(15 12 5), v!(0 0 2), 0.0, GREEN);
    }

    fn tick_after_physics(&mut self, cvars: &Cvars, engine: &mut Engine, dt: f32) {
        let scene = &mut engine.scenes[self.gs.scene];

        if cvars.d_dbg {
            scene.graph.update_hierarchical_data();
        }

        // Debug
        // LATER Warn when drawing text/shapes from prev frame.

        // Keep this first so it draws below other debug stuff.
        if cvars.d_draw && cvars.d_draw_physics {
            scene.graph.physics.draw(&mut scene.drawing_context);
        }

        // Testing
        for cycle in &self.gs.cycles {
            let body_pos = scene.graph[cycle.body_handle].global_position();
            // Note: Drawing arrows here can reduce FPS in debug builds
            // to single digits if also using physics.draw(). No idea why.
            // Drawing a cross hides that *sometimes* the normal red cross
            // from before physics also appears here.
            dbg_line!(body_pos, body_pos + UP, 0.0, BLUE2);
        }

        DEBUG_SHAPES.with(|shapes| {
            // Sometimes debug shapes overlap and only the last one gets drawn.
            // This is especially common when both client and server wanna draw.
            // So instead, we convert everything to lines,
            // merge colors if they overlap and only then draw it.
            // This way if cl and sv shapes overlap, they end up yellow (red + green).
            // LATER would be more efficient to merge whole shapes, not individual lines.
            let mut shapes = shapes.borrow_mut();
            let mut lines = Lines::new();
            for shape in shapes.iter_mut() {
                if cvars.d_draw {
                    shape.to_lines(&mut lines);
                }
                shape.time -= dt;
            }
            for (_, line) in lines.0 {
                scene.drawing_context.add_line(line);
            }
        });

        let mut debug_string = String::new();
        if cvars.d_draw_text {
            debug_string.push_str(&engine.renderer.get_statistics().to_string());
            debug_string.push_str(&scene.performance_statistics.to_string());
            debug_string.push('\n');
            debug_string.push('\n');
            DEBUG_TEXTS.with(|texts| {
                let texts = texts.borrow();
                for text in texts.iter() {
                    debug_string.push_str(text);
                    debug_string.push('\n');
                }
            });
        }

        // We send an empty string to clear the previous text if printing is disabled.
        engine.user_interface.send_message(TextMessage::text(
            self.debug_text,
            MessageDirection::ToWidget,
            debug_string,
        ));

        debug::details::cleanup();
    }

    fn network_send(&mut self, msg: ClientMessage) {
        let network_msg = net::serialize(msg);
        let res = self.conn.send(&network_msg);
        if let Err(ref e) = res {
            if e.kind() == ErrorKind::ConnectionReset {
                dbg_logf!("Server disconnected, exitting");
                std::process::exit(0);
            }
        }
        res.unwrap();
    }
}

/// State of the local player
///
/// LATER maybe just merge into ClientGame?
#[derive(Debug)]
pub(crate) struct LocalPlayer {
    pub(crate) player_handle: Handle<Player>,
    pub(crate) delta_yaw: f32,
    pub(crate) delta_pitch: f32,
    pub(crate) input: Input,
    pub(crate) input_prev: Input,
}

impl LocalPlayer {
    pub(crate) fn new(player_handle: Handle<Player>) -> Self {
        Self {
            player_handle,
            delta_yaw: 0.0,
            delta_pitch: 0.0,
            // LATER real_time should not be 0 if it's not the first match in the same process?
            input: Input::default(),
            input_prev: Input::default(),
        }
    }
}
