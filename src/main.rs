mod client;
mod common;
mod server;

use rg3d::{
    core::{
        algebra::{Rotation, UnitQuaternion, Vector3},
        color::Color,
        instant::Instant,
        pool::Handle,
    },
    engine::{resource_manager::MaterialSearchOptions, Engine, RigidBodyHandle},
    event::{DeviceEvent, ElementState, Event, ScanCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::node::StubNode,
    physics::prelude::{ColliderBuilder, RigidBodyBuilder},
    scene::{
        base::BaseBuilder, camera::CameraBuilder, debug::Line, node::Node,
        transform::TransformBuilder, Scene,
    },
    window::{Fullscreen, WindowBuilder},
};

// TODO MVP:
//  - [x] Arena and wheel models
//  - [x] Rotate the camera
//  - [x] Move the camera
//  - [ ] Render wheel at player pos
//  - [ ] Primitive networking to force client/server split (QUIC?)
//  - [ ] Driving and collisions
//  - [ ] Trails
// TODO 0.1:
//  - [x] Readme
//  - [x] GH social preview (screenshot)
//  - [ ] CI, audit, badges
//      - [ ] All paths lowercase (or we might have issues on windows)
//  - [ ] CI artifacts - allow downloading from GH
//  - [ ] Trimesh colliders
//      - [ ] Poles - there's many - check perf
//      - [ ] Everything - is it possible to tunnel through at high speeds?
//          - Yes - try CCD?
//  - [ ] Texture the whole arena
//  - [ ] Finish RustCycle model
//  - [ ] Skybox - fractal resembling stars?

// TODO All the LATERs - They mean something can be done better but marking it as a todo would be just noise when grepping. They're things I'd do if I had infinite time and wanted to make the project perfect.

type GameEngine = Engine<(), StubNode>;

struct Client {
    engine: GameEngine,
    gs: GameState,
    ps: PlayerState,
}

impl Client {
    fn new(engine: GameEngine, gs: GameState) -> Self {
        Self {
            engine,
            gs,
            ps: PlayerState::new(),
        }
    }

    fn update(&mut self, elapsed: f32) {
        let dt = 1.0 / 60.0; // TODO configurable

        // TODO read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        let game_time_target = elapsed;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            self.gamelogic_tick(dt);

            self.engine.update(dt);
        }

        self.engine.get_window().request_redraw();
    }

    fn gamelogic_tick(&mut self, dt: f32) {
        let scene = &mut self.engine.scenes[self.gs.scene];

        let camera = &mut scene.graph[self.gs.camera];

        // Turning
        let yaw = Rotation::from_axis_angle(&Vector3::y_axis(), self.ps.yaw.to_radians());
        let x = yaw * Vector3::x_axis();
        let pitch = UnitQuaternion::from_axis_angle(&x, self.ps.pitch.to_radians());
        camera.local_transform_mut().set_rotation(pitch * yaw);

        // Movement
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

        // Testing physics
        if self.ps.input.fire1 || self.ps.input.fire2 {
            let wheel_accel = if self.ps.input.fire1 {
                camera.look_vector() * dt * 50.0
            } else {
                -camera.look_vector() * dt * 50.0
            };
            let mut accel = |handle| {
                let body = scene.physics.bodies.get_mut(&handle).unwrap();
                let mut linvel = *body.linvel();
                linvel += wheel_accel;
                body.set_linvel(linvel, true);
            };
            accel(self.gs.cycle1.body_handle);
            accel(self.gs.cycle2.body_handle);
        }

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
        dbg!(diff);
    }
}

struct Cycle {
    node_handle: Handle<Node>,
    body_handle: RigidBodyHandle,
}

impl Cycle {
    async fn new(engine: &mut GameEngine, scene: &mut Scene, pos: Vector3<f32>, ccd: bool) -> Self {
        let node_handle = engine
            .resource_manager
            .request_model(
                "data/rustcycle/rustcycle.fbx",
                MaterialSearchOptions::RecursiveUp,
            )
            .await
            .unwrap()
            .instantiate_geometry(scene);
        let body_handle = scene.physics.add_body(
            RigidBodyBuilder::new_dynamic()
                .ccd_enabled(ccd)
                .lock_rotations()
                .translation(pos)
                .build(),
        );
        scene.physics.add_collider(
            // Size manually copied from the result of rusty-editor's Fit Collider
            // LATER Remove rustcycle.rgs?
            ColliderBuilder::cuboid(0.125, 0.271, 0.271).build(),
            &body_handle,
        );
        scene.physics_binder.bind(node_handle, body_handle);

        Cycle {
            node_handle,
            body_handle,
        }
    }
}

struct GameState {
    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// TODO d_speed, pause
    /// LATER using f32 for time might lead to instability if a match is left running for a day or so
    game_time: f32,
    scene: Handle<Scene>,
    cycle1: Cycle,
    cycle2: Cycle,
    camera: Handle<Node>,
}

impl GameState {
    async fn new(engine: &mut GameEngine) -> Self {
        let mut scene = Scene::new();
        // This is needed because the default 1 causes the wheel to randomly stutter/stop
        // when just sliding on completely smooth floor. The higher the value, the less it slows down.
        // 2 is very noticeable, 5 is better, 10 is only noticeable at high speeds.
        // It never completely goes away, even with 100.
        // LATER Maybe there is a way to solve this by filtering collisions with the floor?
        scene.physics.integration_parameters.max_ccd_substeps = 1;
        // LATER allow changing scene.physics.integration_parameters.dt ?

        engine
            .resource_manager
            .request_model(
                "data/arena/arena.rgs",
                MaterialSearchOptions::UsePathDirectly,
            )
            .await
            .unwrap()
            .instantiate_geometry(&mut scene);

        let cycle1 = Cycle::new(engine, &mut scene, Vector3::new(-1.0, 5.0, 0.0), true).await;
        let cycle2 = Cycle::new(engine, &mut scene, Vector3::new(1.0, 5.0, 0.0), false).await;

        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 1.0, -3.0))
                    .build(),
            ),
        )
        .build(&mut scene.graph);

        let scene = engine.scenes.add(scene);

        Self {
            game_time: 0.0,
            scene,
            cycle1,
            cycle2,
            camera,
        }
    }
}

/// State of the local player
#[derive(Debug)]
struct PlayerState {
    input: Input,
    pitch: f32,
    yaw: f32,
}

impl PlayerState {
    fn new() -> Self {
        Self {
            input: Input::default(),
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

// LATER Bitfield?
#[derive(Debug, Clone, Default)]
struct Input {
    fire1: bool,
    fire2: bool,
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
}

fn main() {
    let window_builder = WindowBuilder::new()
        .with_title("RustCycles")
        .with_fullscreen(Some(Fullscreen::Borderless(None)));
    let event_loop = EventLoop::new();
    // LATER no vsync
    let mut engine = GameEngine::new(window_builder, &event_loop, true).unwrap();
    let gs = rg3d::core::futures::executor::block_on(GameState::new(&mut engine));
    let mut client = Client::new(engine, gs);

    let clock = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in acse it changes.
        *control_flow = ControlFlow::Poll;
        // This is great because we get events almost immediately,
        // e.g. 70-80 times each *milli*second when doing nothing else beside printing their times.
        // The downside is we occupy a full CPU core just for input.
        // TODO Send important input to server immediately (keyboard and mouse button changes, mouse movement up to those changes)
        // LATER Is there a way to not waste CPU cycles so much?

        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Resized(size) => {
                        client.engine.renderer.set_frame_size(size.into()).unwrap();
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::ReceivedCharacter(_) => {
                        // LATER might be useful for console/chat?
                    }
                    WindowEvent::Focused(focus) => {
                        println!("{} focus {:?}", clock.elapsed().as_secs_f32(), focus);
                        // LATER pause/unpause
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        // NOTE: This event is repeated if the key is held, that means
                        // there can be more `state: Pressed` events before a `state: Released`.
                        // println!(
                        //     "{} keyboard input {:?}",
                        //     clock.elapsed().as_secs_f32(),
                        //     input
                        // );

                        // Use scancodes, not virtual keys, because they don't depend on layout.
                        const W: ScanCode = 17;
                        const A: ScanCode = 30;
                        const S: ScanCode = 31;
                        const D: ScanCode = 32;
                        let pressed = input.state == ElementState::Pressed;
                        match input.scancode {
                            W => client.ps.input.forward = pressed,
                            A => client.ps.input.left = pressed,
                            S => client.ps.input.backward = pressed,
                            D => client.ps.input.right = pressed,
                            _ => {}
                        }
                    }
                    WindowEvent::MouseWheel { delta, phase, .. } => {
                        println!(
                            "{} wheel {:?} {:?}",
                            clock.elapsed().as_secs_f32(),
                            delta,
                            phase
                        );
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        println!(
                            "{} mouse input {:?} {:?}",
                            clock.elapsed().as_secs_f32(),
                            state,
                            button
                        );

                        let pressed = state == ElementState::Pressed;
                        match button {
                            rg3d::event::MouseButton::Left => client.ps.input.fire1 = pressed,
                            rg3d::event::MouseButton::Right => client.ps.input.fire2 = pressed,
                            rg3d::event::MouseButton::Middle => {}
                            rg3d::event::MouseButton::Other(_) => {}
                        }
                    }
                    _ => {}
                }
            }
            // Using device event for mouse motion because
            // - it reports delta, not position
            // - it doesn't care whether we're at the edge of the screen
            // TODO(privacy) make sure we're not handling mouse movement when minimized (and especially not sending to server)
            Event::DeviceEvent { event, .. } => {
                #[allow(clippy::single_match)] // Symmetry with WindowEvent
                match event {
                    DeviceEvent::MouseMotion { delta } => {
                        // LATER This event normally happens every 4 ms for me when moving the mouse. Print stats.
                        // Is it limited by my polling rate? Would it be helpful to teach players how to increase it?
                        // Sometimes i get 4 events every 16 ms. Detect this.
                        // https://github.com/martin-t/rustcycles/issues/1
                        // println!(
                        //     "{} DeviceEvent::MouseMotion {:?}",
                        //     clock.elapsed().as_secs_f32(),
                        //     delta
                        // );

                        // Subtract, don't add the delta - rotations follow the right hand rule
                        client.ps.yaw -= delta.0 as f32; // LATER Normalize to [0, 360°) or something

                        // TODO We should use degrees (or degrees per second) for all user facing values but we must make sure to avoid conversion errors.
                        // Maybe add struct Deg(f32);?
                        client.ps.pitch = (client.ps.pitch + delta.1 as f32).clamp(-90.0, 90.0);
                    }
                    _ => {}
                }
            }
            Event::UserEvent(_) => {}
            // LATER test suspend/resume
            Event::Suspended => {}
            Event::Resumed => {}
            Event::MainEventsCleared => {
                client.update(clock.elapsed().as_secs_f32());
            }
            Event::RedrawRequested(_) => {
                client.engine.render().unwrap(); // LATER only crash if failed multiple times
            }
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => println!("bye"),
        }
    });
}
