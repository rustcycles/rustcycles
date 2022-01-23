//! The client in a client-server multiplayer game architecture.

use std::{collections::VecDeque, io::ErrorKind, net::TcpStream, thread, time::Duration};

use rg3d::{
    dpi::PhysicalSize,
    engine::Engine,
    error::ExternalError,
    event::{ElementState, KeyboardInput, MouseButton, ScanCode},
    gui::{
        brush::Brush,
        formatted_text::WrapMode,
        message::MessageDirection,
        text::{TextBuilder, TextMessage},
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
    scene::{
        camera::{CameraBuilder, Projection, SkyBoxBuilder},
        debug::{Line, SceneDrawingContext},
    },
};

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{ClientMessage, InitData, PlayerCycle, PlayerProjectile, ServerMessage},
        net, GameState, Input,
    },
    debug::{
        self,
        details::{DebugShape, Shape, DEBUG_SHAPES, DEBUG_TEXTS},
    },
    prelude::*,
};

/// Game client.
///
/// Needs to be connected to a game Server to play. Contains a local copy of the game state
/// which might not be entirely accurate due to network lag and packet loss.
pub(crate) struct GameClient {
    pub(crate) mouse_grabbed: bool,
    pub(crate) engine: Engine,
    pub(crate) gs: GameState,
    pub(crate) lp: LocalPlayer,
    pub(crate) camera: Handle<Node>,
    stream: TcpStream,
    buffer: VecDeque<u8>,
    server_messages: Vec<ServerMessage>,
    debug_text: Handle<UiNode>,
}

impl GameClient {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let debug_text =
            TextBuilder::new(WidgetBuilder::new().with_foreground(Brush::Solid(Color::RED)))
                // Word wrap doesn't work if there's an extremely long word.
                .with_wrap(WrapMode::Letter)
                .build(&mut engine.user_interface.build_ctx());

        let mut connect_attempts = 0;
        let mut stream = loop {
            connect_attempts += 1;
            // LATER Don't block the main thread - no sleep in async
            // LATER Limit the number of attempts.
            if let Ok(stream) = TcpStream::connect("127.0.0.1:26000") {
                dbg_logf!("connect attempts: {}", connect_attempts);
                break stream;
            }
            if connect_attempts % 100 == 0 {
                dbg_logf!("connect attempts: {}", connect_attempts);
            }
            thread::sleep(Duration::from_millis(10));
        };
        stream.set_nodelay(true).unwrap();
        stream.set_nonblocking(true).unwrap();

        let mut gs = GameState::new(&mut engine).await;

        // LATER Load everything in parallel (i.e. with GameState)
        // LATER Report error if loading fails
        let top = engine
            .resource_manager
            .request_texture("data/skybox/top.png")
            .await
            .ok();

        let scene = &mut engine.scenes[gs.scene];

        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(v!(0 1 -3))
                    .build(),
            ),
        )
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

        let mut buffer = VecDeque::new();
        let mut server_messages = Vec::new();

        let mut init_attempts = 0;
        let lp = loop {
            init_attempts += 1;
            let closed = net::receive(&mut stream, &mut buffer, &mut server_messages);
            if closed {
                panic!("connection closed before init"); // LATER Don't crash
            }
            if !server_messages.is_empty() {
                let message = server_messages.remove(0);
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
            mouse_grabbed: false,
            engine,
            gs,
            lp,
            camera,
            stream,
            buffer,
            server_messages,
            debug_text,
        }
    }

    pub(crate) fn resized(&mut self, size: PhysicalSize<u32>) {
        // This is also called when the window is first created.

        self.engine.set_frame_size(size.into()).unwrap();

        // mrDIMAS on discord:
        // The root element of the UI is Canvas,
        // it has infinite constraints so it does not stretch its contents.
        // If you'll have some complex UI, I'd advise you to create either
        // a window-sized Border or Grid and attach all your ui elements to it,
        // instead of root canvas.
        self.engine
            .user_interface
            .send_message(WidgetMessage::width(
                self.debug_text,
                MessageDirection::ToWidget,
                size.width as f32,
            ));
    }

    pub(crate) fn focused(&mut self, focus: bool) {
        // Ungrab here is needed in addition to ESC,
        // otherwise the mouse stays grabbed when alt+tabbing to other windows.
        // However, don't automatically grab it when gaining focus,
        // the game can get stuck in a loop (bugs like this are most common on startup)
        // and it would never ungrab.
        if !focus {
            self.set_mouse_grab(false);
        }

        // LATER pause/unpause
    }

    pub(crate) fn keyboard_input(&mut self, input: KeyboardInput) {
        // Use scancodes, not virtual keys, because they don't depend on layout.
        const ESC: ScanCode = 1;
        const W: ScanCode = 17;
        const A: ScanCode = 30;
        const S: ScanCode = 31;
        const D: ScanCode = 32;
        let pressed = input.state == ElementState::Pressed;
        match input.scancode {
            ESC => self.set_mouse_grab(false),
            W => self.lp.input.forward = pressed,
            A => self.lp.input.left = pressed,
            S => self.lp.input.backward = pressed,
            D => self.lp.input.right = pressed,
            c => {
                if pressed {
                    dbg_logf!("pressed scancode: {}", c);
                }
            }
        }

        self.sys_send_input();
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        self.set_mouse_grab(true);

        let pressed = state == ElementState::Pressed;
        match button {
            rg3d::event::MouseButton::Left => self.lp.input.fire1 = pressed,
            rg3d::event::MouseButton::Right => self.lp.input.fire2 = pressed,
            rg3d::event::MouseButton::Middle => self.lp.input.zoom = pressed,
            rg3d::event::MouseButton::Other(_) => {}
        }

        self.sys_send_input();
    }

    pub(crate) fn mouse_motion(&mut self, delta: (f64, f64)) {
        if !self.mouse_grabbed {
            // LATER (privacy) Recheck we're not handling mouse movement when minimized (and especially not sending to server)
            return;
        }

        // LATER cvars
        let mouse_sensitivity_horizontal = 0.5;
        let mouse_sensitivity_vertical = 0.5;
        let delta_yaw = delta.0 as f32 * mouse_sensitivity_horizontal;
        let delta_pitch = delta.1 as f32 * mouse_sensitivity_vertical;

        // Subtract, don't add the delta X - rotations follow the right hand rule
        self.lp.input.yaw.0 -= delta_yaw; // LATER Normalize to [0, 360°) or something
        self.lp.input.pitch.0 = (self.lp.input.pitch.0 + delta_pitch).clamp(-90.0, 90.0);
    }

    /// Either grab mouse and hide cursor
    /// or ungrab mouse and show cursor.
    pub(crate) fn set_mouse_grab(&mut self, grab: bool) {
        // LATER Don't hide cursor in menu.
        if grab != self.mouse_grabbed {
            let window = self.engine.get_window();
            let res = window.set_cursor_grab(grab);
            match res {
                Ok(_) | Err(ExternalError::NotSupported(_)) => {}
                Err(_) => res.unwrap(),
            }
            window.set_cursor_visible(!grab);
            self.mouse_grabbed = grab;
        }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        let dt = 1.0 / 60.0;

        // LATER read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            self.sys_receive_updates();

            self.gs.tick(&mut self.engine, dt);

            self.tick(dt);

            // TODO This runs physics - maybe some gamelogic should run after it?
            // What happens if we draw physics world both before and after?
            // Same on server.
            self.engine.update(dt);

            self.sys_send_input();
        }

        self.engine.get_window().request_redraw();
    }

    fn sys_receive_updates(&mut self) {
        let _ = net::receive(
            &mut self.stream,
            &mut self.buffer,
            &mut self.server_messages,
        ); // LATER Clean disconnect

        let scene = &mut self.engine.scenes[self.gs.scene];

        for message in self.server_messages.drain(..) {
            match message {
                ServerMessage::InitData(_) => {
                    panic!("Received unexpected init")
                }
                ServerMessage::AddPlayer(add_player) => {
                    let player = Player::new(None);
                    self.gs
                        .players
                        .spawn_at(add_player.player_index, player)
                        .unwrap();
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
                    dbg!(cycle_index);
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
                        body.local_transform_mut()
                            .set_position(cycle_physics.translation);
                        body.local_transform_mut()
                            .set_rotation(cycle_physics.rotation);
                        dbg_textd!(cycle_physics.rotation);
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

    fn tick(&mut self, dt: f32) {
        // Join / spec
        let ps = self.gs.players[self.lp.player_handle].ps;
        if ps == PlayerState::Observing && self.lp.input.fire1 {
            self.network_send(ClientMessage::Join);
        } else if ps == PlayerState::Playing && self.lp.input.fire2 {
            self.network_send(ClientMessage::Observe);
        }

        let scene = &mut self.engine.scenes[self.gs.scene];

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
            dbg_cross!(body_pos, 5.0, Color::GREEN);
        }

        dbg_line!(v!(15 5 5), v!(15 5 7));

        dbg_arrow!(v!(10 5 5), v!(10 5 7)); // Forward
        dbg_arrow!(v!(10 5 5), v!(10 5 4)); // Back
        dbg_arrow!(v!(10 5 5), v!(9 5 5)); // Left
        dbg_arrow!(v!(10 5 5), v!(11 5 5)); // Right
        dbg_arrow!(v!(10 5 5), v!(10 6 5)); // Up
        dbg_arrow!(v!(10 5 5), v!(10 4 5)); // Down

        dbg_arrow!(v!(10 10 5), v!(11 11 6));
        dbg_arrow!(v!(10 10 10), v!(12 12 12));

        dbg_cross!(v!(5 5 5), 0.0, Color::WHITE);

        // Debug
        scene.drawing_context.clear_lines();

        // This ruins perf in debug builds: https://github.com/rg3dengine/rg3d/issues/237
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
        debug_string.push_str(&self.engine.renderer.get_statistics().to_string());
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
        self.engine.user_interface.send_message(TextMessage::text(
            self.debug_text,
            MessageDirection::ToWidget,
            debug_string,
        ));

        debug::details::cleanup();
    }

    /// Send all once-per-frame stuff to the server.
    fn sys_send_input(&mut self) {
        self.network_send(ClientMessage::Input(self.lp.input));
    }

    fn network_send(&mut self, message: ClientMessage) {
        let network_message = net::serialize(message);
        let res = net::send(&network_message, &mut self.stream);
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
        Shape::Arrow { begin, end } => {
            drawing_context.add_line(Line {
                begin,
                end,
                color: shape.color,
            });

            // We want two of the side lines to be above and below the arrow body
            // and the other two to the sides if the arrow is pointing horizontally
            // and appear pitched up/down if not.

            let dir = end - begin;
            let rot = UnitQuaternion::face_towards(&dir, &Vec3::up());
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
    }
}

/// State of the local player
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
