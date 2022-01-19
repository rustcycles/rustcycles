//! The client in a client-server multiplayer game architecture.

use std::{collections::VecDeque, net::TcpStream, thread, time::Duration};

use rg3d::{
    core::{
        algebra::{Rotation3, UnitQuaternion},
        color::Color,
        pool::Handle,
    },
    engine::Engine,
    error::ExternalError,
    event::{ElementState, KeyboardInput, MouseButton, ScanCode},
    scene::{
        base::BaseBuilder,
        camera::{CameraBuilder, SkyBoxBuilder},
        debug::Line,
        node::Node,
        transform::TransformBuilder,
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
        details::{Shape, DEBUG_SHAPES},
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
}

impl GameClient {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let mut connect_attempts = 0;
        let mut stream = loop {
            connect_attempts += 1;
            // LATER Don't block the main thread - no sleep in async
            // LATER Limit the number of attempts.
            if let Ok(stream) = TcpStream::connect("127.0.0.1:26000") {
                println!("C connect attempts: {}", connect_attempts);
                break stream;
            }
            if connect_attempts % 100 == 0 {
                println!("C connect attempts: {}", connect_attempts);
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

                    println!("C init attempts: {}", init_attempts);
                    break lp;
                } else {
                    panic!("First message wasn't init"); // LATER Don't crash
                }
            }
            if init_attempts % 100 == 0 {
                println!("C init attempts: {}", init_attempts);
            }
            thread::sleep(Duration::from_millis(10));
        };
        println!("C local_player_index is {}", lp.player_handle.index());

        Self {
            mouse_grabbed: false,
            engine,
            gs,
            lp,
            camera,
            stream,
            buffer,
            server_messages,
        }
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
                    println!("C pressed scancode: {}", c);
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
            rg3d::event::MouseButton::Middle => {}
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
                    println!("C player {} is now observing", player_index);
                }
                ServerMessage::Spectate {
                    player_index,
                    spectatee_index,
                } => {
                    let spectatee_handle = self.gs.players.handle_from_index(spectatee_index);
                    self.gs.players.at_mut(player_index).unwrap().ps =
                        PlayerState::Spectating { spectatee_handle };
                    println!(
                        "C player {} is now spectating player {}",
                        player_index, spectatee_index
                    );
                }
                ServerMessage::Join { player_index } => {
                    self.gs.players.at_mut(player_index).unwrap().ps = PlayerState::Playing;
                    println!("C player {} is now playing", player_index);
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
                ServerMessage::UpdatePhysics(update_physics) => {
                    for cycle_physics in update_physics.cycle_physics {
                        let cycle = self.gs.cycles.at_mut(cycle_physics.cycle_index).unwrap();
                        let body = scene.graph[cycle.body_handle].as_rigid_body_mut();
                        body.local_transform_mut()
                            .set_position(cycle_physics.translation);
                        body.local_transform_mut()
                            .set_rotation(cycle_physics.rotation);
                        body.set_lin_vel(cycle_physics.velocity);
                    }
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
        let camera = &mut scene.graph[self.camera];

        // Camera turning
        let yaw = Rotation3::from_axis_angle(&Vec3::up_axis(), self.lp.input.yaw.0.to_radians());
        let x = yaw * Vec3::left_axis();
        let pitch = UnitQuaternion::from_axis_angle(&x, self.lp.input.pitch.0.to_radians());
        camera.local_transform_mut().set_rotation(pitch * yaw);

        let forward = camera.forward_vec_normed();
        let left = camera.left_vec_normed();

        // Camera movement
        let mut camera_pos = **camera.local_transform().position();
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
        camera.local_transform_mut().set_position(camera_pos);

        // Debug
        scene.drawing_context.clear_lines();

        for cycle in &self.gs.cycles {
            let pos = scene.graph[cycle.node_handle].global_position();
            dbg_cross!(pos, 3.0, Color::GREEN);
        }
        dbg_cross!(v!(5 5 5), 0.0, Color::WHITE);

        DEBUG_SHAPES.with(|shapes| {
            let mut shapes = shapes.borrow_mut();
            for shape in shapes.iter_mut() {
                let Shape::Cross { point } = shape.shape;
                // LATER if cvars.d_draw && cvars.d_draw_crosses {
                let half_len = 0.5; // LATER cvar
                let dir = v!(1 1 1) * half_len;
                scene.drawing_context.add_line(Line {
                    begin: point - dir,
                    end: point + dir,
                    color: shape.color,
                });

                let dir = v!(-1 1 1) * half_len;
                scene.drawing_context.add_line(Line {
                    begin: point - dir,
                    end: point + dir,
                    color: shape.color,
                });

                let dir = v!(1 1 -1) * half_len;
                scene.drawing_context.add_line(Line {
                    begin: point - dir,
                    end: point + dir,
                    color: shape.color,
                });

                let dir = v!(-1 1 -1) * half_len;
                scene.drawing_context.add_line(Line {
                    begin: point - dir,
                    end: point + dir,
                    color: shape.color,
                });

                let from_origin = false; // LATER cvar
                if from_origin {
                    scene.drawing_context.add_line(Line {
                        begin: Vec3::zeros(),
                        end: point,
                        color: shape.color,
                    });
                }
                // LATER }
                shape.time -= dt;
            }
        });
        debug::details::cleanup();

        // This ruins perf in debug builds: https://github.com/rg3dengine/rg3d/issues/237
        scene.graph.physics.draw(&mut scene.drawing_context);
    }

    /// Send all once-per-frame stuff to the server.
    fn sys_send_input(&mut self) {
        self.network_send(ClientMessage::Input(self.lp.input));
    }

    fn network_send(&mut self, message: ClientMessage) {
        let network_message = net::serialize(message);
        net::send(&network_message, &mut self.stream).unwrap();
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
