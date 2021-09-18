mod client;
mod common;
mod server;

use rg3d::{
    core::{algebra::Vector3, instant::Instant, pool::Handle},
    engine::{resource_manager::MaterialSearchOptions, Engine},
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::node::StubNode,
    scene::{
        base::BaseBuilder, camera::CameraBuilder, node::Node, transform::TransformBuilder, Scene,
    },
    window::WindowBuilder,
};

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

            // TODO gamelogic here

            self.engine.update(dt);
        }

        self.engine.get_window().request_redraw();
    }
}

struct GameState {
    /// This gamelogic frame's time in seconds.
    ///
    /// This does *not* have to run at the same speed as real world time.
    /// TODO d_speed, pause
    /// LATER using f32 for time might lead to instability if a match is left running for a day or so
    game_time: f32,
    arena: Handle<Scene>,
    camera: Handle<Node>,
}

impl GameState {
    async fn new(engine: &mut GameEngine) -> Self {
        let mut scene = Scene::new();
        engine
            .resource_manager
            .request_model(
                "data/arena/arena.rgs",
                MaterialSearchOptions::UsePathDirectly,
            )
            .await
            .unwrap()
            .instantiate_geometry(&mut scene);

        let camera = CameraBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 1.0, -3.0))
                    .build(),
            ),
        )
        .build(&mut scene.graph);

        let arena = engine.scenes.add(scene);

        Self {
            game_time: 0.0,
            arena,
            camera,
        }
    }
}

/// State of the local player
#[derive(Debug)]
struct PlayerState {
    pitch: f32,
    yaw: f32,
}

impl PlayerState {
    fn new() -> Self {
        Self {
            pitch: 0.0,
            yaw: 0.0,
        }
    }
}

#[derive(Debug)]
struct Input {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
}

fn main() {
    let window_builder = WindowBuilder::new().with_title("RustCycles");
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
                        println!(
                            "{} keyboard input {:?}",
                            clock.elapsed().as_secs_f32(),
                            input
                        );
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
                    }
                    _ => {}
                }
            }
            // Using device event for mouse motion because
            // - it reports delta, not position
            // - it doesn't care whether we're at the edge of the screen
            // TODO(privacy) make sure we're not handling mouse movement when minimized
            Event::DeviceEvent { event, .. } => {
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
                        client.ps.yaw += delta.0 as f32; // LATER Normalize to [0, 2*PI) or something

                        // LATER We should use degrees for all user facing values but we must make sure to avoid conversion errors. Maybe add struct Deg(f32);?
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
