use std::{io::Write, net::TcpStream};

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
    common::{GameState, Input},
    GameEngine,
};

pub(crate) struct Client {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) ps: PlayerState,
    pub(crate) camera: Handle<Node>,
    stream: TcpStream,
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

        let mut debug_cross = |handle| {
            let cycle = &scene.graph[handle];
            let pos = cycle.global_position();

            let dir = Vector3::new(1.0, 1.0, 1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color: Color::RED,
            });

            let dir = Vector3::new(-1.0, 1.0, 1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color: Color::RED,
            });

            let dir = Vector3::new(1.0, 1.0, -1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color: Color::RED,
            });

            let dir = Vector3::new(-1.0, 1.0, -1.0) * 0.25;
            scene.drawing_context.add_line(Line {
                begin: pos - dir,
                end: pos + dir,
                color: Color::RED,
            });
        };
        debug_cross(self.gs.cycle1.node_handle);
        debug_cross(self.gs.cycle2.node_handle);

        scene.physics.draw(&mut scene.drawing_context);

        let pos1 = scene
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
        });
    }

    fn network_receive(&mut self) {}

    fn network_send(&mut self) {
        self.stream.write_all(b"Test            ").unwrap();
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
