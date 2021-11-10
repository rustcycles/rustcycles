mod client;
mod common;
mod server;

use std::{env, process::Command};

use rg3d::{
    core::instant::Instant,
    dpi::LogicalSize,
    engine::Engine,
    event::{DeviceEvent, ElementState, Event, ScanCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    utils::log::{Log, MessageKind},
    window::{Fullscreen, WindowBuilder},
};
use structopt::StructOpt;
use strum_macros::EnumString;

use client::Client;
use server::Server;

// TODO MVP:
//  - [x] Arena and wheel models
//  - [x] Rotate the camera
//  - [x] Move the camera
//  - [x] Render wheel at player pos
//  - [ ] Primitive networking to force client/server split
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

type GameEngine = Engine;

#[derive(StructOpt, Debug)]
struct Opts {
    /// Use a window instead of fullscreen (doesn't apply to server)
    #[structopt(long)]
    windowed: bool,

    #[structopt(subcommand)]
    endpoint: Option<Endpoint>,
}

#[derive(StructOpt, Debug, EnumString)]
enum Endpoint {
    /// Run only the game client
    Client,
    /// Run only the game server
    Server,
}

fn main() {
    let opts = Opts::from_args();

    match opts.endpoint {
        None => client_server_main(opts),
        Some(Endpoint::Client) => client_main(opts),
        Some(Endpoint::Server) => server_main(),
    }
}

/// Run both client and server.
///
/// This is currently just a convenience for quicker testing
/// but eventually should allow running singleplayer games
/// without most of the overhead of the client-server split.
fn client_server_main(opts: Opts) {
    // LATER Find a way to run client and server in one process,
    // maybe even one thread - sharing GameState woul be ideal for singleplayer.
    //
    // This is broken - most input gets ignored (on Kubuntu):
    // thread::spawn(|| {
    //     // LATER EventLoop::new_any_thread is Unix only, what happens on Windows?
    //     server_main(EventLoop::new_any_thread());
    // });
    // thread::sleep(Duration::from_secs(1));
    // client_main();

    let path = env::args().next().unwrap();

    let mut server = Command::new(&path).arg("server").spawn().unwrap();

    let mut client_cmd = Command::new(&path);
    if opts.windowed {
        client_cmd.arg("--windowed");
    }
    client_cmd.arg("client");
    let mut client = client_cmd.spawn().unwrap();

    // We wanna close just the client and automatically close the server that way.
    client.wait().unwrap();
    server.kill().unwrap();
}

fn client_main(opts: Opts) {
    // LATER Switch rg3d to a more standard logger
    // or at least add a level below INFO so load times can remain as INFO
    // and the other messages are hidden by default.
    // Also used in server_main().
    Log::set_verbosity(MessageKind::Warning);

    let mut window_builder = WindowBuilder::new().with_title("RustCycles");
    if !opts.windowed {
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let event_loop = EventLoop::new();
    // LATER no vsync
    let engine = GameEngine::new(window_builder, &event_loop, true).unwrap();
    let mut client = rg3d::core::futures::executor::block_on(Client::new(engine));

    let clock = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in acse it changes.
        *control_flow = ControlFlow::Poll;
        // This is great because we get events almost immediately,
        // e.g. 70-80 times each *milli*second when doing nothing else beside printing their times.
        // The downside is we occupy a full CPU core just for input.
        // TODO Send important input to server immediately (keyboard and mouse button changes, mouse movement up to those changes)
        // LATER Is there a way to not waste CPU cycles so much?

        #[allow(clippy::single_match)]
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Resized(size) => {
                        client.engine.set_frame_size(size.into()).unwrap();
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::ReceivedCharacter(_) => {
                        // LATER might be useful for console/chat?
                    }
                    WindowEvent::Focused(focus) => {
                        //println!("{} focus {:?}", clock.elapsed().as_secs_f32(), focus);
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
            Event::LoopDestroyed => println!("C bye"),
        }
    });
}

fn server_main() {
    // See note in client_main().
    Log::set_verbosity(MessageKind::Warning);

    // LATER Headless - do all this without creating a window.
    let window_builder = WindowBuilder::new()
        .with_title("RustCycles server")
        .with_inner_size(LogicalSize::new(400, 100));
    let event_loop = EventLoop::new();
    // LATER Does vsync have any effect here?
    let engine = GameEngine::new(window_builder, &event_loop, false).unwrap();
    let mut server = rg3d::core::futures::executor::block_on(Server::new(engine));

    // Render pure black just once so the window doesn't look broken.
    server.engine.render().unwrap();

    let clock = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in acse it changes.
        *control_flow = ControlFlow::Poll;

        #[allow(clippy::single_match)]
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::DeviceEvent { .. } => {}
            Event::UserEvent(_) => {}
            Event::Suspended => {}
            Event::Resumed => {}
            Event::MainEventsCleared => {
                server.update(clock.elapsed().as_secs_f32());
            }
            Event::RedrawRequested(_) => {}
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => println!("S bye"),
        }
    });
}
