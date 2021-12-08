//! The client in a client-server multiplayer game architecture.

use std::{collections::VecDeque, net::TcpStream};

use rg3d::{
    core::{
        algebra::{Rotation3, UnitQuaternion, Vector3},
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

use crate::common::{
    entities::{Participation, Player},
    messages::{ClientMessage, ServerMessage},
    net, GameState, Input,
};

/// Game client.
///
/// Needs to be connected to a game Server to play. Contains a local copy of the game state
/// which might not be entirely accurate due to network lag and packet loss.
pub(crate) struct Client {
    pub(crate) mouse_grabbed: bool,
    pub(crate) engine: Engine,
    pub(crate) gs: GameState,
    pub(crate) ps: PlayerState,
    pub(crate) camera: Handle<Node>,
    stream: TcpStream,
    buffer: VecDeque<u8>,
    server_packets: Vec<ServerMessage>,
}

impl Client {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let mut connect_attempts = 0;
        let stream = loop {
            connect_attempts += 1;
            // LATER Don't block the main thread.
            // LATER Limit the number of attempts.
            if let Ok(stream) = TcpStream::connect("127.0.0.1:26000") {
                println!("C connect attempts: {}", connect_attempts);
                break stream;
            }
            // LATER Maybe add a short delay (test local vs remove server)?
        };
        stream.set_nodelay(true).unwrap();
        stream.set_nonblocking(true).unwrap();

        let gs = GameState::new(&mut engine).await;

        // LATER Load everything in parallel (i.e. with GameState)
        // LATER Report error if loading fails
        let top = engine
            .resource_manager
            .request_texture("data/skybox/top.png", None)
            .await
            .ok();

        let scene = &mut engine.scenes[gs.scene];
        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 1.0, -3.0))
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

        Self {
            mouse_grabbed: false,
            engine,
            gs,
            ps: PlayerState::new(),
            camera,
            stream,
            buffer: VecDeque::new(),
            server_packets: Vec::new(),
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
            W => self.ps.input.forward = pressed,
            A => self.ps.input.left = pressed,
            S => self.ps.input.backward = pressed,
            D => self.ps.input.right = pressed,
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
            rg3d::event::MouseButton::Left => self.ps.input.fire1 = pressed,
            rg3d::event::MouseButton::Right => self.ps.input.fire2 = pressed,
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

        // Subtract, don't add the delta X - rotations follow the right hand rule
        self.ps.input.yaw.0 -= delta.0 as f32; // LATER Normalize to [0, 360°) or something
        self.ps.input.pitch.0 = (self.ps.input.pitch.0 + delta.1 as f32).clamp(-90.0, 90.0);
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
        let _ = net::receive(&mut self.stream, &mut self.buffer, &mut self.server_packets); // LATER Clean disconnect

        let scene = &mut self.engine.scenes[self.gs.scene];

        for packet in self.server_packets.drain(..) {
            match packet {
                ServerMessage::InitData(init_data) => {
                    for player_cycle in init_data.player_cycles {
                        let player = Player::new(None);
                        let player_handle = self
                            .gs
                            .players
                            .spawn_at(player_cycle.player_index, player)
                            .unwrap();

                        if let Some(cycle_index) = player_cycle.cycle_index {
                            let cycle_handle =
                                self.gs.spawn_cycle(scene, player_handle, Some(cycle_index));
                            self.gs.players[player_handle].cycle_handle = Some(cycle_handle);
                        }
                    }
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
                ServerMessage::SpawnCycle(spawn_cycle) => {
                    let player_handle = self
                        .gs
                        .players
                        .handle_from_index(spawn_cycle.player_cycle.player_index);

                    let cycle_index = spawn_cycle.player_cycle.cycle_index.unwrap();
                    let cycle_handle = self.gs.spawn_cycle(scene, player_handle, Some(cycle_index));
                    self.gs.players[player_handle].cycle_handle = Some(cycle_handle);
                }
                ServerMessage::DespawnCycle { cycle_index } => {
                    dbg!(cycle_index);
                    todo!();
                }
                ServerMessage::UpdatePhysics(update_physics) => {
                    for cycle_physics in update_physics.cycle_physics {
                        let cycle = self.gs.cycles.at_mut(cycle_physics.cycle_index).unwrap();
                        let body = scene.physics.bodies.get_mut(&cycle.body_handle).unwrap();
                        body.set_translation(cycle_physics.translation, true);
                        body.set_linvel(cycle_physics.velocity, true);
                    }
                }
            }
        }
    }

    fn tick(&mut self, dt: f32) {
        let scene = &mut self.engine.scenes[self.gs.scene];

        // Join / spec
        if self.ps.participation == Participation::Observing && self.ps.input.fire1 {
            self.ps.participation = Participation::Playing;
        } else if self.ps.participation == Participation::Playing && self.ps.input.fire2 {
            self.ps.participation = Participation::Observing;
        }

        let camera = &mut scene.graph[self.camera];

        // Camera turning
        let yaw = Rotation3::from_axis_angle(&Vector3::y_axis(), self.ps.input.yaw.0.to_radians());
        let x = yaw * Vector3::x_axis();
        let pitch = UnitQuaternion::from_axis_angle(&x, self.ps.input.pitch.0.to_radians());
        camera.local_transform_mut().set_rotation(pitch * yaw);

        let forward = camera
            .look_vector()
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vector3::z);
        let left = camera
            .side_vector()
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vector3::x);

        // Camera movement
        let mut pos = **camera.local_transform().position();
        let camera_speed = 10.0;
        if self.ps.input.forward {
            pos += forward * dt * camera_speed;
        }
        if self.ps.input.backward {
            pos += -forward * dt * camera_speed;
        }
        if self.ps.input.left {
            pos += left * dt * camera_speed;
        }
        if self.ps.input.right {
            pos += -left * dt * camera_speed;
        }
        camera.local_transform_mut().set_position(pos);

        // Debug
        scene.drawing_context.clear_lines();

        let mut debug_cross = |pos, color| {
            let dir = Vector3::new(1.0, 1.0, 1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color,
            });

            let dir = Vector3::new(-1.0, 1.0, 1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color,
            });

            let dir = Vector3::new(1.0, 1.0, -1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color,
            });

            let dir = Vector3::new(-1.0, 1.0, -1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color,
            });
        };

        for cycle in &self.gs.cycles {
            scene.graph[cycle.node_handle].global_position();
            debug_cross(pos, Color::GREEN);
        }

        // This ruins perf in debug builds: https://github.com/rg3dengine/rg3d/issues/237
        //scene.physics.draw(&mut scene.drawing_context);

        /*let pos1 = scene
            .physics
            .bodies
            .get(&self.gs.cycle1.body_handle)
            .unwrap()
            .position()
            .translation
            .vector;
        let pos2 = scene
            .physics
            .bodies
            .get(&self.gs.cycle2.body_handle)
            .unwrap()
            .position()
            .translation
            .vector;
        scene.drawing_context.add_line(Line {
            begin: pos1,
            end: pos2,
            color: Color::GREEN,
        });
        let diff = pos1 - pos2;
        let my_center = Vector3::new(0.0, 3.0, 0.0);
        scene.drawing_context.add_line(Line {
            begin: my_center,
            end: my_center + diff,
            color: Color::GREEN,
        });*/
    }

    /// Send all once-per-frame stuff to the server.
    fn sys_send_input(&mut self) {
        let packet = ClientMessage::Input(self.ps.input);
        let network_message = net::serialize(packet);
        net::send(&network_message, &mut self.stream).unwrap();
    }
}

/// State of the *local* player
#[derive(Debug)]
pub(crate) struct PlayerState {
    pub(crate) input: Input,
    pub(crate) participation: Participation,
}

impl PlayerState {
    pub(crate) fn new() -> Self {
        Self {
            input: Input::default(),
            participation: Participation::Observing,
        }
    }
}
