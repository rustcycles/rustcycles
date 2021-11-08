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
    scene::{
        base::BaseBuilder, camera::CameraBuilder, debug::Line, node::Node,
        transform::TransformBuilder,
    },
};

use crate::{
    common::{GameState, Input, ServerPacket},
    GameEngine,
};

pub(crate) struct Client {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) ps: PlayerState,
    pub(crate) camera: Handle<Node>,
    stream: TcpStream,
    buf: VecDeque<u8>,
    server_packet: ServerPacket,
}

impl Client {
    pub(crate) async fn new(mut engine: GameEngine) -> Self {
        let mut connect_attempts = 0;
        let stream = loop {
            connect_attempts += 1;
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
            engine,
            gs,
            ps: PlayerState::new(),
            camera,
            stream,
            buf: VecDeque::default(),
            server_packet: ServerPacket::default(),
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

            self.gs.tick(&mut self.engine, dt, self.ps.input);

            self.tick(dt);

            self.engine.update(dt);

            // LATER sending (some) input should happen as soon as we receive it
            self.network_send();
        }

        self.engine.get_window().request_redraw();
    }

    fn tick(&mut self, dt: f32) {
        let scene = &mut self.engine.scenes[self.gs.scene];

        let camera = &mut scene.graph[self.camera];

        // Camera turning
        let yaw = Rotation::from_axis_angle(&Vector3::y_axis(), self.ps.yaw.to_radians());
        let x = yaw * Vector3::x_axis();
        let pitch = UnitQuaternion::from_axis_angle(&x, self.ps.pitch.to_radians());
        camera.local_transform_mut().set_rotation(pitch * yaw);

        // Camera movement
        let mut pos = *camera.local_transform().position().get();
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
        for &pos in &self.server_packet.positions {
            debug_cross(pos, Color::BLUE);
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
        loop {
            // Read all available bytes until the stream would block.
            // LATER Test networking thoroughly
            //      - large amounts of data
            //      - lossy and slow connections
            //      - fragmented and merged packets
            // TODO Err(ref e) if e.kind() == ErrorKind::Interrupted => {} ???

            // No particular reason for the buffer size, except BufReader uses the same.
            let mut buf = [0; 8192];
            let res = self.stream.read(&mut buf);
            match res {
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
            self.server_packet = bincode::deserialize(&bytes).unwrap();
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
