//! Client-side gamelogic.
//!
//! Mainly receiving updates from the server and updating local state.

use std::{io::ErrorKind, thread, time::Duration};

use fyrox::{
    gui::{message::MessageDirection, text::TextMessage, UiNode, UserInterface},
    renderer::Renderer,
    scene::camera::{CameraBuilder, Projection, SkyBoxBuilder},
};

use crate::{
    common::{
        entities::{Player, PlayerState},
        net::{self, Connection},
        Input,
    },
    debug::{
        self,
        details::UniqueLines,
        {DEBUG_SHAPES, DEBUG_TEXTS, DEBUG_TEXTS_WORLD},
    },
    prelude::*,
};

/// Game data inside a client process.
///
/// Needs to be connected to a game Server to play. Contains a local copy of the game state
/// which might not be entirely accurate due to network lag and packet loss.
pub struct ClientGame {
    debug_text: Handle<UiNode>,
    conn: Box<dyn Connection<ServerMessage>>,
    pub camera_handle: Handle<Node>,
    pub player_handle: Handle<Player>,
    pub delta_yaw: f32,
    pub delta_pitch: f32,
    pub input: Input,
    pub input_prev: Input,
}

/// All data necessary to run a frame of client-side game logic in one convenient package.
///
/// See also `ServerFrameCtx` and `FrameCtx`.
pub struct ClientFrameCtx<'a> {
    pub cvars: &'a Cvars,
    pub scene: &'a mut Scene,
    pub gs: &'a mut GameState,
    pub cg: &'a mut ClientGame,
    pub renderer: Option<&'a mut Renderer>,
    pub ui: &'a mut UserInterface,
}

impl ClientGame {
    pub async fn new(
        cvars: &Cvars,
        engine: &mut Engine,
        debug_text: Handle<UiNode>,
        mut conn: Box<dyn Connection<ServerMessage>>,
        gs: &mut GameState,
    ) -> Self {
        let scene = &mut engine.scenes[gs.scene_handle];

        // LATER Load everything in parallel (i.e. with GameState)
        // LATER Report error if loading fails
        let front = engine.resource_manager.request("data/skybox/front.png").await.ok();
        let back = engine.resource_manager.request("data/skybox/back.png").await.ok();
        let left = engine.resource_manager.request("data/skybox/left.png").await.ok();
        let right = engine.resource_manager.request("data/skybox/right.png").await.ok();
        let top = engine.resource_manager.request("data/skybox/top.png").await.ok();
        let bottom = engine.resource_manager.request("data/skybox/bottom.png").await.ok();
        let camera_handle = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(cvars.cl_camera_initial_position.into())
                    .build(),
            ),
        )
        .with_skybox(
            SkyBoxBuilder {
                front,
                back,
                left,
                right,
                top,
                bottom,
            }
            .build()
            .unwrap(),
        )
        .build(&mut scene.graph);

        let mut ctx = FrameCtx { cvars, scene, gs };

        let mut init_attempts = 0;
        let player_handle = loop {
            init_attempts += 1;
            let (msg, closed) = conn.receive_one();
            if closed {
                panic!("connection closed before init"); // LATER Don't crash
            }
            if let Some(msg) = msg {
                if let ServerMessage::Init(init) = msg {
                    let local_player_handle = ctx.init(init);
                    dbg_logf!("init attempts: {}", init_attempts);
                    break local_player_handle;
                } else {
                    panic!("First message wasn't init"); // LATER Don't crash
                }
            }
            if init_attempts % 100 == 0 {
                dbg_logf!("init attempts: {}", init_attempts);
            }
            thread::sleep(Duration::from_millis(10));
        };
        dbg_logf!("local player_index is {}", player_handle.index());

        if cvars.d_testing {
            let f = false;
            soft_assert!(f);
            soft_assert!(f, "test");
            let one = 1;
            let two = 2;
            soft_assert_eq!(one, two);
            soft_assert_eq!(one, two, "test");
            soft_assert_ne!(one, one);
            soft_assert_ne!(one, one, "test");

            //for x in [1, 2, 3].iter().dbg_count(|cnt| dbg_logf!("element count: {}", cnt)) {}
            for _ in [1, 2, 3, 4, 5, 6, 7].iter().dbg_count_log("element count full") {}
            let limit = 5;
            for _ in [1, 2, 3, 4, 5, 6, 7]
                .iter()
                .dbg_count_log(format!("element count before take({limit})"))
                .take(limit)
                .dbg_count_log(format!("element count after take({limit})"))
                .filter(|&&x| x % 2 == 1)
                .dbg_count_log("element count odd")
            {}
        }

        Self {
            debug_text,
            conn,
            camera_handle,
            player_handle,
            delta_yaw: 0.0,
            delta_pitch: 0.0,
            input: Input::default(),
            input_prev: Input::default(),
        }
    }

    pub fn send_input(&mut self) {
        self.network_send(ClientMessage::Input(self.input));
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

impl FrameCtx<'_> {
    pub fn init(&mut self, init: Init) -> Handle<Player> {
        if self.gs.gs_type == GameStateType::Shared {
            // The player has already been spawned when running server logic.
            // LATER Deduplicate this line. Maybe don't even return the handle?
            return self.gs.players.handle_from_index(init.local_player_index);
        }

        for player_index in init.player_indices {
            let player = Player::new(None);
            self.gs.players.spawn_at(player_index, player).unwrap();
        }
        let local_player_handle = self.gs.players.handle_from_index(init.local_player_index);

        for PlayerCycle {
            player_index,
            cycle_index,
        } in init.player_cycles
        {
            let player_handle = self.gs.players.handle_from_index(player_index);
            self.spawn_cycle(player_handle, Some(cycle_index));
        }

        for PlayerProjectile {
            player_index: _,
            projectile_index: _,
        } in init.player_projectiles
        {
            todo!("init projectiles");
        }

        local_player_handle
    }
}

impl ClientFrameCtx<'_> {
    pub fn ctx(&mut self) -> FrameCtx<'_> {
        FrameCtx {
            cvars: self.cvars,
            scene: self.scene,
            gs: self.gs,
        }
    }

    /// All once-per-frame networking.
    pub fn tick_begin_frame(&mut self) {
        // LATER Always send key/mouse presses immediately
        // but maybe rate-limit mouse movement updates
        // in case some systems update mouse position at a very high rate.
        self.cg.input_prev = self.cg.input;

        self.cg.input.yaw.0 += self.cg.delta_yaw; // LATER Normalize to [0, 360°) or something
        self.cg.input.pitch.0 = (self.cg.input.pitch.0 + self.cg.delta_pitch)
            .clamp(self.cvars.m_pitch_min, self.cvars.m_pitch_max);

        let delta_time = self.gs.game_time - self.gs.game_time_prev;
        soft_assert!(delta_time > 0.0);
        self.cg.input.yaw_speed.0 = self.cg.delta_yaw / delta_time;
        self.cg.input.pitch_speed.0 = self.cg.delta_pitch / delta_time;

        self.cg.delta_yaw = 0.0;
        self.cg.delta_pitch = 0.0;

        self.cg.send_input();

        self.scene.drawing_context.clear_lines();

        let (msgs, closed) = self.cg.conn.receive();

        if self.gs.gs_type == GameStateType::Shared {
            // Shared mode ignores all messages that update game state
            // since it's updated when running server logic.
            return; // LATER Early return in the middle of a function is ugly
        }

        for msg in msgs {
            match msg {
                ServerMessage::Version(_) => todo!(),
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
                    self.ctx().free_player(player_handle);
                }
                ServerMessage::Observe { player_index } => {
                    self.gs.players.at_mut(player_index).unwrap().state = PlayerState::Observing;
                    dbg_logf!("player {} is now observing", player_index);
                }
                ServerMessage::Spectate {
                    player_index,
                    spectatee_index,
                } => {
                    let spectatee_handle = self.gs.players.handle_from_index(spectatee_index);
                    self.gs.players.at_mut(player_index).unwrap().state =
                        PlayerState::Spectating { spectatee_handle };
                    dbg_logf!(
                        "player {} is now spectating player {}",
                        player_index,
                        spectatee_index
                    );
                }
                ServerMessage::Join { player_index } => {
                    self.gs.players.at_mut(player_index).unwrap().state = PlayerState::Playing;
                    dbg_logf!("player {} is now playing", player_index);
                }
                ServerMessage::SpawnCycle(PlayerCycle {
                    player_index,
                    cycle_index,
                }) => {
                    let player_handle = self.gs.players.handle_from_index(player_index);
                    self.ctx().spawn_cycle(player_handle, Some(cycle_index));
                }
                ServerMessage::DespawnCycle { cycle_index } => {
                    dbg_logd!(cycle_index);
                    todo!("despawn cycle");
                }
                ServerMessage::Update(Update {
                    player_inputs,
                    cycle_physics,
                    debug_texts,
                    debug_texts_world,
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
                        let body = self.scene.graph[cycle.body_handle].as_rigid_body_mut();
                        body.local_transform_mut().set_position(translation);
                        body.local_transform_mut().set_rotation(rotation);
                        body.set_lin_vel(velocity);
                    }

                    DEBUG_TEXTS.with_borrow_mut(|texts| {
                        texts.extend(debug_texts);
                    });
                    DEBUG_TEXTS_WORLD.with_borrow_mut(|texts| {
                        texts.extend(debug_texts_world);
                    });
                    DEBUG_SHAPES.with_borrow_mut(|shapes| {
                        shapes.extend(debug_shapes);
                    });
                }
            }
        }

        if closed {
            dbg_logf!("Server closed the connection, exitting"); // LATER Don't exit
            std::process::exit(0);
        }
    }

    pub fn tick_before_physics(&mut self, dt: f32) {
        // Join / spec
        let ps = self.gs.players[self.cg.player_handle].state;
        if ps == PlayerState::Observing && self.cg.input.fire1 {
            self.cg.network_send(ClientMessage::Join);
        } else if ps == PlayerState::Playing && self.cg.input.fire2 {
            self.cg.network_send(ClientMessage::Observe);
        }

        let player_cycle_handle = self.gs.players[self.cg.player_handle].cycle_handle.unwrap();
        let player_body_handle = self.gs.cycles[player_cycle_handle].body_handle;
        let player_cycle_pos = **self.scene.graph[player_body_handle].local_transform().position();

        let camera = &mut self.scene.graph[self.cg.camera_handle];

        // Camera turning
        let cam_rot = self.cg.input.look_rotation();
        camera.local_transform_mut().set_rotation(cam_rot);

        dbg_rot!(v!(0 7 0), cam_rot);
        dbg_arrow!(v!(0 5 0), cam_rot * FORWARD);

        // Camera movement
        let camera_pos_old = **camera.local_transform().position();
        let trace_opts = TraceOptions::filter(!IG_ENTITIES).with_end(true);
        if ps == PlayerState::Observing {
            let forward = camera.forward_vec_normed();
            let left = camera.left_vec_normed();
            let up = camera.up_vec_normed();
            let mut delta = Vec3::zeros();
            if self.cg.input.forward {
                delta += forward * dt * self.cvars.cl_camera_speed;
            }
            if self.cg.input.backward {
                delta += -forward * dt * self.cvars.cl_camera_speed;
            }
            if self.cg.input.left {
                delta += left * dt * self.cvars.cl_camera_speed;
            }
            if self.cg.input.right {
                delta += -left * dt * self.cvars.cl_camera_speed;
            }
            if self.cg.input.up {
                delta += up * dt * self.cvars.cl_camera_speed;
            }
            if self.cg.input.down {
                delta += -up * dt * self.cvars.cl_camera_speed;
            }

            let hits = self.ctx().trace_line(camera_pos_old, delta, trace_opts);
            let new_pos = hits[0].position.coords;
            self.scene.graph[self.cg.camera_handle]
                .local_transform_mut()
                .set_position(new_pos);
        } else if ps == PlayerState::Playing {
            let up = UP * self.cvars.cl_camera_3rd_person_up;
            let back = cam_rot * BACK * self.cvars.cl_camera_3rd_person_back;

            let hits = self.ctx().trace_line(player_cycle_pos, up, trace_opts);
            let hits = self.ctx().trace_line(hits[0].position, back, trace_opts);
            let new_pos = hits[0].position.coords;
            self.scene.graph[self.cg.camera_handle]
                .local_transform_mut()
                .set_position(new_pos);
        } else {
            unreachable!(); // LATER Spectating
        }

        // Camera zoom
        let camera = self.scene.graph[self.cg.camera_handle].as_camera_mut();
        if let Projection::Perspective(perspective) = camera.projection_mut() {
            let zoom_factor = if self.cg.input.zoom {
                self.cvars.cl_zoom_factor
            } else {
                1.0
            };
            perspective.fov = self.cvars.cl_camera_fov.to_radians() / zoom_factor;
            perspective.z_near = self.cvars.cl_camera_z_near;
            perspective.z_far = self.cvars.cl_camera_z_far;
        } else {
            unreachable!();
        }

        // Testing
        for cycle in &self.gs.cycles {
            let body_pos = self.scene.graph[cycle.body_handle].global_position();
            dbg_cross!(body_pos, 3.0);
        }

        // LATER Intersect with each pole (currently it probably assumes they're all one object)
        let hits = self.ctx().trace_line(0.5 * DOWN + BACK, FORWARD, TraceOptions::default());
        for hit in hits {
            dbg_cross!(hit.position.coords, 0.0);
        }

        // Examples of all the debug shapes

        dbg_line!(v!(25 5 5), v!(25 5 6));

        dbg_arrow!(v!(20 5 5), v!(0 0 2)); // Forward
        dbg_arrow!(v!(20 5 5), v!(0 0 -1)); // Back
        dbg_arrow!(v!(20 5 5), v!(-1 0 0)); // Left
        dbg_arrow!(v!(20 5 5), v!(1 0 0)); // Right
        dbg_arrow!(v!(20 5 5), v!(0 1 0)); // Up
        dbg_arrow!(v!(20 5 5), v!(0 -1 0)); // Down

        dbg_cross!(v!(15 5 5), 0.0, CYAN);

        dbg_rot!(v!(10 5 5), UnitQuaternion::default());

        dbg_arrow!(v!(20 10 5), v!(1 1 1), 0.0, BLUE);
        dbg_arrow!(v!(20 10 6), v!(2 2 2), 0.0, BLUE2);

        dbg_arrow!(v!(15 10 5), v!(0 0 2), 0.0, RED);
        dbg_arrow!(v!(15 10 5), v!(0 0.01 2), 0.0, GREEN);
        dbg_arrow!(v!(15 11 5), v!(0 0 2), 0.0, GREEN);
        dbg_arrow!(v!(15 11 5), v!(0 0.01 2), 0.0, RED);

        // The smallest possible difference in the up direction
        // that doesn't get rounded to nothing.
        // This doesn't really test anything,
        // it just gives interesting results sometimes
        // like changing colors when you move the camera.
        dbg_arrow!(v!(14 10 5), v!(0 0 2), 0.0, RED);
        dbg_arrow!(v!(14 10 5), v!(0 0.000001 2), 0.0, GREEN);
        dbg_arrow!(v!(14 11 5), v!(0 0 2), 0.0, GREEN);
        dbg_arrow!(v!(14 11 5), v!(0 0.000001 2), 0.0, RED);

        dbg_arrow!(v!(13 10 5), v!(0 0 2), 0.0, RED);
        dbg_arrow!(v!(13 10 5), v!(0 0 2), 0.0, GREEN);
        dbg_arrow!(v!(13 11 5), v!(0 0 2), 0.0, GREEN);
        dbg_arrow!(v!(13 11 5), v!(0 0 2), 0.0, RED);

        // Depending whether overlap detection is done on shapes or individual lines,
        // the arrows' centers will be red/green or yellow.
        dbg_arrow!(v!(12 10 5), v!(0 0 2), 0.0, RED);
        dbg_line!(v!(12 10 5), v!(12 10 7), 0.0, GREEN);
        dbg_arrow!(v!(12 11 5), v!(0 0 2), 0.0, GREEN);
        dbg_line!(v!(12 11 5), v!(12 11 7), 0.0, RED);

        dbg_arrow!(v!(20 15 5), v!(-0.01 0.03 -1));

        // An example of why euler angles cause issues with interpolation.
        // Start with no rotation, then rotate 180° around each axis.
        // The result is visually the same as no rotation but the euler angles are different.

        let t = self.gs.game_time % 7.0;
        let tyaw = t.clamp(1.0, 2.0) - 1.0;
        let tpitch = t.clamp(3.0, 4.0) - 3.0;
        let troll = t.clamp(5.0, 6.0) - 5.0;
        let yaw = tyaw * PI;
        let pitch = tpitch * PI;
        let roll = troll * PI;

        // This is off to the side because it's hard to see above the map.
        let pos_rot = v!(40 5 5);
        let mut pos_indicator = pos_rot + 1.1 * BACK;
        dbg_line!(pos_indicator, pos_indicator + tyaw * UP, 0, GREEN);
        pos_indicator += 0.1 * RIGHT;
        dbg_line!(pos_indicator, pos_indicator + tpitch * UP, 0, RED);
        pos_indicator += 0.1 * RIGHT;
        dbg_line!(pos_indicator, pos_indicator + troll * UP, 0, BLUE2);

        let rot = UnitQuaternion::from_ypr(yaw, pitch, roll);
        dbg_rot!(pos_rot, rot);
        dbg_rot!(pos_rot, Default::default(), 0, 0.25);

        // Show the difference between intrinsic and extrinsic rotations
        // (easier to see with just 45° instead of 180°).
        let yaw45 = tyaw * PI / 4.0;
        let pitch45 = tpitch * PI / 4.0;
        let roll45 = troll * PI / 4.0;
        let rot45_intrinsic = UnitQuaternion::from_ypr(yaw45, pitch45, roll45);
        dbg_rot!(pos_rot + 4.0 * FORWARD, rot45_intrinsic);
        let rot45_extrinsic = UnitQuaternion::from_ypr_extrinsic(yaw45, pitch45, roll45);
        dbg_rot!(pos_rot + 6.0 * FORWARD, rot45_extrinsic);

        // For understanding the difference between global and local pitch.

        let yaw_angle = self.cg.input.yaw.to_radians();
        let pitch_angle = self.cg.input.pitch.to_radians();
        let yaw_rot = UnitQuaternion::from_axis_angle(&UP_AXIS, yaw_angle);
        let pitch_rot1 = UnitQuaternion::from_axis_angle(&LEFT_AXIS, pitch_angle);
        let pitch_axis = yaw_rot * LEFT_AXIS;
        let pitch_rot2 = UnitQuaternion::from_axis_angle(&pitch_axis, pitch_angle);

        dbg_rot!(v!(52 7 5), yaw_rot);
        dbg_rot!(v!(51 7 5), pitch_rot1);
        dbg_rot!(v!(50 7 5), pitch_rot2);
        dbg_rot!(v!(51 7 7), pitch_rot1 * yaw_rot);
        dbg_rot!(v!(50 7 7), pitch_rot2 * yaw_rot);

        dbg_arrow!(v!(52 5 5), yaw_rot * FORWARD);
        dbg_arrow!(v!(51 5 5), pitch_rot1 * FORWARD);
        dbg_arrow!(v!(50 5 5), pitch_rot2 * FORWARD);
        dbg_arrow!(v!(51 5 7), pitch_rot1 * yaw_rot * FORWARD);
        dbg_arrow!(v!(50 5 7), pitch_rot2 * yaw_rot * FORWARD);
    }

    pub fn tick_after_physics(&mut self, dt: f32) {
        if self.cvars.d_physics_extra_sync {
            self.scene.graph.update_hierarchical_data();
        }

        // Debug
        // LATER Warn when drawing text/shapes from prev frame.

        // Keep this first so it draws below other debug stuff.
        if self.cvars.d_draw && self.cvars.d_draw_physics {
            self.scene.graph.physics.draw(&mut self.scene.drawing_context);
        }

        // Testing
        for cycle in &self.gs.cycles {
            let body_pos = self.scene.graph[cycle.body_handle].global_position();
            // Note: Drawing arrows here can reduce FPS in debug builds
            // to single digits if also using physics.draw(). No idea why.
            // Note2: We draw a line here because drawing a cross hides the fact
            // that *sometimes* the normal red cross from before physics
            // also appears in the same position.
            dbg_line!(body_pos, body_pos + UP, 0.0);
        }

        // Deduplicate and draw debug shapes
        DEBUG_SHAPES.with_borrow_mut(|shapes| {
            // Sometimes debug shapes overlap and only the last one gets drawn.
            // This is especially common when both client and server wanna draw.
            // So instead, we convert everything to lines,
            // merge colors if they overlap and only then draw it.
            // This way if cl and sv shapes overlap, they end up yellow (red + green).
            let mut lines = UniqueLines::default();
            for shape in shapes.iter_mut() {
                if self.cvars.d_draw {
                    shape.to_lines(self.cvars, &mut lines);
                }
                shape.time -= dt;
            }
            for (_, line) in lines.0 {
                self.scene.drawing_context.add_line(line);
            }
        });

        // Compose per-frame debug string
        let mut debug_string = String::new();
        if self.cvars.d_draw && self.cvars.d_draw_text {
            if self.cvars.d_engine_stats {
                if let Some(renderer) = &self.renderer {
                    debug_string.push_str(&renderer.get_statistics().to_string());
                }
                debug_string.push_str(&self.scene.performance_statistics.to_string());
                debug_string.push('\n');
                debug_string.push('\n');
            }
            DEBUG_TEXTS.with_borrow(|texts| {
                for text in texts.iter() {
                    debug_string.push_str(text);
                    debug_string.push('\n');
                }
            });
        }

        // Draw per-frame debug string.
        // Do this even if printing is disabled - we still need to clear the previous text.
        self.ui.send_message(TextMessage::text(
            self.cg.debug_text,
            MessageDirection::ToWidget,
            debug_string,
        ));

        DEBUG_TEXTS_WORLD.with_borrow(|texts| {
            if !texts.is_empty() {
                soft_unreachable!("world texts not yet implemented"); // LATER
            }
        });

        // Cleanup
        debug::clear_expired();
    }
}
