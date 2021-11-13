use std::{
    collections::VecDeque,
    io::{ErrorKind, Read, Write},
    net::TcpStream,
};

use rg3d::{
    core::{
        algebra::{Rotation, UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
    },
    engine::Engine,
    error::ExternalError,
    scene::{
        base::BaseBuilder, camera::CameraBuilder, debug::Line, node::Node,
        transform::TransformBuilder,
    },
};

use crate::common::{GameState, Input, Player, ServerMessage};

pub(crate) struct Client {
    pub(crate) mouse_grabbed: bool,
    pub(crate) engine: Engine,
    pub(crate) gs: GameState,
    pub(crate) ps: PlayerState,
    pub(crate) camera: Handle<Node>,
    stream: TcpStream,
    buf: VecDeque<u8>,
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
        stream.set_nonblocking(true).unwrap();

        let gs = GameState::new(&mut engine).await;

        let scene = &mut engine.scenes[gs.scene];
        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 1.0, -3.0))
                    .build(),
            ),
        )
        .build(&mut scene.graph);

        Self {
            mouse_grabbed: false,
            engine,
            gs,
            ps: PlayerState::new(),
            camera,
            stream,
            buf: VecDeque::new(),
            server_packets: Vec::new(),
        }
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
        let dt = 1.0 / 60.0; // TODO configurable

        // TODO read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            self.network_receive();

            self.gs.tick(&mut self.engine, dt);

            self.tick(dt);

            self.engine.update(dt);

            // LATER sending (some) input should happen as soon as we receive it
            self.network_send();
        }

        self.engine.get_window().request_redraw();
    }

    fn tick(&mut self, dt: f32) {
        let scene = &mut self.engine.scenes[self.gs.scene];

        for packet in self.server_packets.drain(..) {
            match packet {
                ServerMessage::Spawn(init) => {
                    for init_player in init.players {
                        let player = Player::new(Handle::NONE);
                        let player_index = usize::try_from(init_player.player_index).unwrap();
                        let player_handle = self.gs.players.spawn_at(player_index, player).unwrap();
                        let cycle_handle = self.gs.spawn_cycle(
                            scene,
                            player_handle,
                            Some(usize::try_from(init_player.cycle_index).unwrap()),
                        );
                        self.gs.players[player_handle].cycle_handle = cycle_handle;
                    }
                }
                ServerMessage::Update(update) => {
                    for update_cycle in update.cycles {
                        let index = usize::try_from(update_cycle.cycle_index).unwrap();
                        let cycle = self.gs.cycles.at_mut(index).unwrap();
                        let body = scene.physics.bodies.get_mut(&cycle.body_handle).unwrap();
                        body.set_translation(update_cycle.translation, true);
                        body.set_linvel(update_cycle.velocity, true);
                    }
                }
            }
        }

        let camera = &mut scene.graph[self.camera];

        // Camera turning
        let yaw = Rotation::from_axis_angle(&Vector3::y_axis(), self.ps.yaw.to_radians());
        let x = yaw * Vector3::x_axis();
        let pitch = UnitQuaternion::from_axis_angle(&x, self.ps.pitch.to_radians());
        camera.local_transform_mut().set_rotation(pitch * yaw);

        // Camera movement
        let mut pos = **camera.local_transform().position();
        let camera_speed = 10.0;
        if self.ps.input.forward {
            // TODO normalize?
            pos += camera.look_vector() * dt * camera_speed;
        }
        if self.ps.input.backward {
            pos += -camera.look_vector() * dt * camera_speed;
        }
        if self.ps.input.left {
            pos += camera.side_vector() * dt * camera_speed;
        }
        if self.ps.input.right {
            pos += -camera.side_vector() * dt * camera_speed;
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

        scene.physics.draw(&mut scene.drawing_context);

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

    fn network_receive(&mut self) {
        // Read all available bytes until the stream would block.
        // LATER Test networking thoroughly
        //      - large amounts of data
        //      - lossy and slow connections
        //      - fragmented and merged packets
        // TODO Err(ref e) if e.kind() == ErrorKind::Interrupted => {} ???
        loop {
            // No particular reason for the buffer size, except BufReader uses the same.
            let mut buf = [0; 8192];
            let res = self.stream.read(&mut buf);
            match res {
                Ok(0) => {
                    // The connection has been closed, don't get stuck in this loop.
                    // This can happen for example when the server crashes.
                    // LATER Some kind of clean client shutdown.
                    //  Currently the client crashes later when attempting to send.
                    break;
                }
                Ok(n) => {
                    self.buf.extend(&buf[0..n]);
                }
                Err(err) => match err.kind() {
                    ErrorKind::WouldBlock => {
                        break;
                    }
                    _ => panic!("network error (read): {}", err),
                },
            }
        }

        // Parse the received bytes
        loop {
            if self.buf.len() < 2 {
                break;
            }
            let len_bytes = [self.buf[0], self.buf[1]];
            let len = usize::from(u16::from_le_bytes(len_bytes));
            if self.buf.len() < len + 2 {
                // Not enough bytes in buffer for a full frame.
                break;
            }
            self.buf.pop_front();
            self.buf.pop_front();
            let bytes: Vec<_> = self.buf.drain(0..len).collect();
            let message = bincode::deserialize(&bytes).unwrap();
            self.server_packets.push(message);
        }
    }

    fn network_send(&mut self) {
        let data = bincode::serialize(&self.ps.input).unwrap();
        self.stream.write_all(&data).unwrap();
    }
}

/// State of the local player
#[derive(Debug)]
pub(crate) struct PlayerState {
    pub(crate) input: Input,
    pub(crate) pitch: f32,
    pub(crate) yaw: f32,
}

impl PlayerState {
    pub(crate) fn new() -> Self {
        Self {
            input: Input::default(),
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}
