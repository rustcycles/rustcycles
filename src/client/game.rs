//! Client-side gamelogic.
//!
//! Mainly receiving updates from the server and updating local state.

use std::{io::ErrorKind, thread, time::Duration};

use fyrox::{
    gui::{message::MessageDirection, text::TextMessage, UiNode},
    scene::{
        camera::{CameraBuilder, Projection, SkyBoxBuilder},
        debug::{Line, SceneDrawingContext},
    },
};

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{ClientMessage, InitData, PlayerCycle, PlayerProjectile, ServerMessage},
        net::{self, Connection},
        GameState, Input,
    },
    debug::{
        self,
        details::{DebugShape, Shape, DEBUG_SHAPES, DEBUG_TEXTS},
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
    connection: Box<dyn Connection>,
}

impl ClientGame {
    pub(crate) async fn new(
        engine: &mut Engine,
        debug_text: Handle<UiNode>,
        mut connection: Box<dyn Connection>,
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
            let (message, closed) = connection.receive_one_sm();
            if closed {
                panic!("connection closed before init"); // LATER Don't crash
            }
            if let Some(message) = message {
                if let ServerMessage::InitData(InitData {
                    player_indices,
                    local_player_index,
                    player_cycles,
                    player_projectiles,
                }) = message
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
            connection,
        }
    }

    pub(crate) fn update(&mut self, engine: &mut Engine, game_time_target: f32) {
        // LATER read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
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

            self.tick_after_physics(engine, dt);

            // Update UI
            engine.post_update(dt);
        }

        engine.get_window().request_redraw();
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
    #[allow(dead_code)]
    fn debug_engine_updates(&self, pos: Vec3, steps: usize) {
        let step = (self.gs.frame_number % steps) as f32;
        let angle = 2.0 * std::f32::consts::PI / steps as f32 * step as f32;
        let rot = UnitQuaternion::from_axis_angle(&Vec3::forward_axis(), angle);
        let dir = rot * Vec3::up();
        dbg_arrow!(pos, dir);
        dbg_textd!(self.gs.frame_number, pos, angle.to_degrees());
    }

    pub(crate) fn send_input(&mut self) {
        self.network_send(ClientMessage::Input(self.lp.input));
    }

    /// All once-per-frame networking.
    fn tick_begin_frame(&mut self, engine: &mut Engine) {
        // LATER Always send key/mouse presses immediately
        // but maybe rate-limit mouse movement updates
        // in case some systems update mouse position at a very high rate.
        self.send_input();

        let scene = &mut engine.scenes[self.gs.scene];

        scene.drawing_context.clear_lines();

        let (messages, _) = self.connection.receive_sm();
        for message in messages {
            match message {
                ServerMessage::InitData(_) => {
                    // LATER Make this type safe? Init part of handshake?
                    panic!("Received unexpected init")
                }
                ServerMessage::AddPlayer(add_player) => {
                    let player = Player::new(None);
                    self.gs.players.spawn_at(add_player.player_index, player).unwrap();
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
                ServerMessage::Update {
                    update_physics,
                    debug_texts,
                    debug_shapes,
                } => {
                    for cycle_physics in update_physics.cycle_physics {
                        let cycle = self.gs.cycles.at_mut(cycle_physics.cycle_index).unwrap();
                        let body = scene.graph[cycle.body_handle].as_rigid_body_mut();
                        body.local_transform_mut().set_position(cycle_physics.translation);
                        body.local_transform_mut().set_rotation(cycle_physics.rotation);
                        body.set_lin_vel(cycle_physics.velocity);
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
        let yaw = UnitQuaternion::from_axis_angle(&Vec3::up_axis(), yaw_angle);

        let pitch_angle = self.lp.input.pitch.0.to_radians();
        let pitch_axis = yaw * Vec3::left_axis();
        let pitch = UnitQuaternion::from_axis_angle(&pitch_axis, pitch_angle);

        let cam_rot = pitch * yaw;
        camera.local_transform_mut().set_rotation(cam_rot);

        dbg_rot!(v!(0 7 0), cam_rot);
        dbg_arrow!(v!(0 5 0), cam_rot * Vec3::forward());

        // Camera movement
        let mut camera_pos = **camera.local_transform().position();
        if ps == PlayerState::Observing {
            let forward = camera.forward_vec_normed();
            let left = camera.left_vec_normed();
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
        } else if ps == PlayerState::Playing {
            // LATER cvars
            let back = -(cam_rot * Vec3::forward() * 2.0);
            let up = Vec3::up() * 0.5;
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

        dbg_line!(v!(15 5 5), v!(15 5 7));

        dbg_arrow!(v!(10 5 5), v!(0 0 2)); // Forward
        dbg_arrow!(v!(10 5 5), v!(0 0 -1)); // Back
        dbg_arrow!(v!(10 5 5), v!(-1 0 0)); // Left
        dbg_arrow!(v!(10 5 5), v!(1 0 0)); // Right
        dbg_arrow!(v!(10 5 5), v!(0 1 0)); // Up
        dbg_arrow!(v!(10 5 5), v!(0 -1 0)); // Down

        dbg_arrow!(v!(10 10 5), v!(1 1 1), 0.0, BLUE);
        dbg_arrow!(v!(10 10 10), v!(2 2 2), 0.0, BLUE2);

        dbg_cross!(v!(5 5 5), 0.0, CYAN);
    }

    fn tick_after_physics(&mut self, engine: &mut Engine, dt: f32) {
        let scene = &mut engine.scenes[self.gs.scene];

        //scene.graph.update_hierarchical_data(); TODO

        // Testing
        for cycle in &self.gs.cycles {
            let body_pos = scene.graph[cycle.body_handle].global_position();
            // Note: Drawing arrows here can reduce FPS in debug builds
            // to single digits if also using physics.draw(). No idea why.
            // Drawing a cross hides that *sometimes* the normal red cross
            // from before physics also appears here.
            dbg_line!(body_pos, body_pos + Vec3::up(), 0.0, BLUE2);
        }

        // Debug
        // LATER Warn when drawing text/shaped from prev frame.

        // This ruins perf in debug builds: https://github.com/rg3dengine/rg3d/issues/237
        // Try engine.renderer.set_quality_settings(&QualitySettings::low()).unwrap();
        // Keep this first so it draws below other debug stuff.
        scene.graph.physics.draw(&mut scene.drawing_context);

        DEBUG_SHAPES.with(|shapes| {
            let mut shapes = shapes.borrow_mut();
            for shape in shapes.iter_mut() {
                // LATER if cvars.d_draw && cvars.d_draw_crosses {
                draw_shape(&mut scene.drawing_context, shape);
                // LATER }
                shape.time -= dt;
            }
        });

        let mut debug_string = String::new();
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
        engine.user_interface.send_message(TextMessage::text(
            self.debug_text,
            MessageDirection::ToWidget,
            debug_string,
        ));

        debug::details::cleanup();
    }

    fn network_send(&mut self, message: ClientMessage) {
        let network_message = net::serialize(message);
        let res = self.connection.send(&network_message);
        if let Err(ref e) = res {
            if e.kind() == ErrorKind::ConnectionReset {
                dbg_logf!("Server disconnected, exitting");
                std::process::exit(0);
            }
        }
        res.unwrap();
    }
}

fn draw_shape(drawing_context: &mut SceneDrawingContext, shape: &DebugShape) {
    match shape.shape {
        Shape::Line { begin, end } => {
            drawing_context.add_line(Line {
                begin,
                end,
                color: shape.color,
            });
        }
        Shape::Arrow { begin, dir } => {
            let end = begin + dir;
            drawing_context.add_line(Line {
                begin,
                end,
                color: shape.color,
            });

            // When the arrow is horizontal, we want two of the side lines
            // to be above and below the arrow body and the other two to the sides.
            // When it's not horizontal, we want it to appear pitched up/down,
            // no weird rotations around the axis.

            // Make sure dir and up are not colinear.
            let up = if dir.x < f32::EPSILON && dir.z < f32::EPSILON {
                Vec3::forward()
            } else {
                Vec3::up()
            };

            let rot = UnitQuaternion::face_towards(&dir, &up);
            let len = dir.magnitude();
            let left = rot * Vec3::left() * len;
            let up = rot * Vec3::up() * len;
            drawing_context.add_line(Line {
                begin: end,
                end: end + (-dir + left) * 0.25,
                color: shape.color,
            });
            drawing_context.add_line(Line {
                begin: end,
                end: end + (-dir - left) * 0.25,
                color: shape.color,
            });
            drawing_context.add_line(Line {
                begin: end,
                end: end + (-dir + up) * 0.25,
                color: shape.color,
            });
            drawing_context.add_line(Line {
                begin: end,
                end: end + (-dir - up) * 0.25,
                color: shape.color,
            });
        }
        Shape::Cross { point } => {
            let half_len = 0.5; // LATER cvar
            let dir = v!(1 1 1) * half_len;
            drawing_context.add_line(Line {
                begin: point - dir,
                end: point + dir,
                color: shape.color,
            });

            let dir = v!(-1 1 1) * half_len;
            drawing_context.add_line(Line {
                begin: point - dir,
                end: point + dir,
                color: shape.color,
            });

            let dir = v!(1 1 -1) * half_len;
            drawing_context.add_line(Line {
                begin: point - dir,
                end: point + dir,
                color: shape.color,
            });

            let dir = v!(-1 1 -1) * half_len;
            drawing_context.add_line(Line {
                begin: point - dir,
                end: point + dir,
                color: shape.color,
            });

            let from_origin = false; // LATER cvar
            if from_origin {
                drawing_context.add_line(Line {
                    begin: Vec3::zeros(),
                    end: point,
                    color: shape.color,
                });
            }
        }
        Shape::Rot { point, rot } => {
            let matrix = rot.to_homogeneous().append_translation(&point);
            drawing_context.draw_transform(matrix);
        }
    }
}

/// State of the local player
///
/// LATER maybe just merge into ClientGame?
#[derive(Debug)]
pub(crate) struct LocalPlayer {
    pub(crate) player_handle: Handle<Player>,
    pub(crate) input: Input,
}

impl LocalPlayer {
    pub(crate) fn new(player_handle: Handle<Player>) -> Self {
        Self {
            player_handle,
            input: Input::default(),
        }
    }
}
