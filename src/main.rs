//! This file contains entry points and event loops for the native client and server.
//! If you're looking for thŁe main game loop, it's in ClientGame and ServerGame.

// Keep this first so the macros are available everywhere without having to import them.
#[macro_use]
pub(crate) mod debug;

mod client;
mod common;
mod prelude;
mod server;

use std::{env, panic, process::Command, sync::Arc};

use fyrox::{
    core::{futures::executor, instant::Instant},
    dpi::{LogicalSize, PhysicalSize},
    engine::{resource_manager::ResourceManager, Engine, EngineInitParams, SerializationContext},
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    utils::log::{Log, MessageKind},
    window::{Fullscreen, WindowBuilder},
};
use strum_macros::EnumString;

#[cfg(feature = "cli")]
use structopt::StructOpt;

use crate::{
    client::process::ClientProcess,
    debug::details::{DebugEndpoint, DEBUG_ENDPOINT},
    prelude::*,
    server::process::ServerProcess,
};

// Master TODO list:
// v0.1 - MVP:
//  - [x] Arena and wheel models
//  - [x] Rotate the camera
//  - [x] Move the camera
//  - [x] Render wheel at player pos
//  - [x] Primitive networking to force client/server split
//  - [ ] Driving and collisions
//  - [ ] Trails
// yak-shaving:
//  - [ ] WASM local client for testing / showcases
//      - [ ] Client and server in one process - local gameplay
//  - [ ] What is happening when FPS drops to single digits
//        (e.g. when using physics.draw twice in a frame)
//      - [ ] Custom counter for FPS and durations - avg, max
// v0.2:
//  - [x] Readme
//  - [x] GH social preview (screenshot)
//  - [x] CI, audit, badges
//      - [ ] Check all paths are lowercase (or we might have issues on windows)
//  - [ ] CI artifacts - allow downloading from GH
//  - [ ] Trimesh colliders
//      - [ ] Poles - there's many - check perf
//      - [ ] Everything - is it possible to tunnel through at high speeds?
//          - Yes - try CCD?
//  - [ ] Use proper lights instead of just ambient light
//  - [ ] Texture the whole arena
//  - [ ] Finish RustCycle model
//  - [ ] Skybox - fractal resembling stars?
// v1.0:
//  - [ ] Include version number in binaries, report between cl and sv during handshake
//      - Must not increase incremental build time - worst case do it only for releases
// All the LATERs
//  - They mean something can be done better but marking it as a todo would be just noise when grepping.
//    They're things I'd do if I had infinite time and wanted to make the project perfect.
//    As the game matures, some of them might be promoted to todos.

// LATERs:
//  - [ ] Remove all unwraps - go through all the code, convert infallible ones to except, fallible ones to Result
//  - [x] Remove all prints and dbgs, convert them to a proper logger impl which differentiates client and server logs.
//  - [ ] If possible, lint against unwrap, print, println, dbg,
//          todo, panic, unreachable, unimplemented, ...

#[derive(Debug, Default)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
struct Opts {
    /// Use a window instead of fullscreen (doesn't apply to server)
    #[cfg_attr(feature = "cli", structopt(long))]
    windowed: bool,

    /// Whether to run the client, server or both.
    #[cfg_attr(feature = "cli", structopt(subcommand))]
    endpoint: Option<Endpoint>,
}

#[derive(Debug, EnumString)]
#[cfg_attr(feature = "cli", derive(StructOpt))]
enum Endpoint {
    /// Run a local game (client and server in one process)
    Local,
    /// Run only the game client
    Client,
    /// Run only the game server
    Server,
}

#[cfg(feature = "cli")]
fn main() {
    let opts = Opts::from_args();
    run(opts);
}

#[cfg(not(feature = "cli"))]
fn main() {
    let mut opts = Opts::default();

    // We kinda wanna use structopt because it has nice QoL features
    // but it adds a couple hundred ms to incremental debug builds.
    // So for dev builds we use this crude way of parsing input instead
    // to build and therefore iterate a tiny bit faster.
    //
    // If this gets too complex, might wanna consider https://github.com/RazrFalcon/pico-args.
    let args = env::args().skip(1); // Skip path to self
    for arg in args {
        match arg.as_str() {
            "local" => opts.endpoint = Some(Endpoint::Local),
            "client" => opts.endpoint = Some(Endpoint::Client),
            "server" => opts.endpoint = Some(Endpoint::Server),
            "--windowed" => opts.windowed = true,
            other => panic!("unexpected argument: {other}"),
        }
    }

    run(opts);
}

fn run(opts: Opts) {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        dbg_logf!("panicking");
        prev_hook(panic_info);
    }));

    match opts.endpoint {
        // LATER None should launch client and offer choice in menu
        None => client_server_main(opts),
        Some(Endpoint::Local) => client_main(opts, true),
        Some(Endpoint::Client) => client_main(opts, false),
        Some(Endpoint::Server) => server_main(),
    }
}

/// Run both client and server.
///
/// This is just a convenience for quicker testing.
/// It spawns 2 processes to make sure the other is killed if one crashes.
///
/// LATER It should do that explicitly, right now it only kills the server
/// because client quits without a server anyway.
fn client_server_main(opts: Opts) {
    init_global_state("launcher");

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
    dbg_logf!("Client exitted, killing server");
    server.kill().unwrap();
}

/// LATER Do we want a shared game state or just running both
/// client and server in one thread? Update docs on Endpoint or wherever.
fn client_main(opts: Opts, local_server: bool) {
    if local_server {
        init_global_state("lo");
    } else {
        init_global_state("cl");
    }

    let event_loop = EventLoop::new();
    let engine = init_engine_client(&event_loop, opts);

    let mut client = executor::block_on(ClientProcess::new(engine, local_server));
    let clock = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in case it changes.
        *control_flow = ControlFlow::Poll;
        // This is great because we get events almost immediately,
        // e.g. 70-80 times each *milli*second when doing nothing else beside printing their times.
        // The downside is we occupy a full CPU core just for input.
        // LATER Offload gamelogic and rendering to another thread so input can be received at any time and sent to server immediately.
        // LATER Is there a way to not waste CPU cycles so much?

        // Exhaustively match all variants so we notice if the enum changes.
        #[allow(clippy::single_match)]
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Resized(size) => {
                        client.resized(size);
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::ReceivedCharacter(_) => {
                        // LATER might be useful for console/chat?
                    }
                    WindowEvent::Focused(focus) => {
                        //dbg_logf!("{} focus {:?}", clock.elapsed().as_secs_f32(), focus);

                        client.focused(focus);
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        // NOTE: This event is repeated if the key is held, that means
                        // there can be more `state: Pressed` events before a `state: Released`.
                        // dbg_logf!(
                        //     "{} keyboard input {:?}",
                        //     clock.elapsed().as_secs_f32(),
                        //     input
                        // );

                        client.keyboard_input(input);
                    }
                    WindowEvent::MouseWheel { delta, phase, .. } => {
                        dbg_logf!(
                            "{} mouse wheel {:?} {:?}",
                            clock.elapsed().as_secs_f32(),
                            delta,
                            phase
                        );
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        client.mouse_input(state, button);
                    }
                    WindowEvent::CursorMoved {
                        position: _position,
                        ..
                    } => {
                        //dbg_logd!(_position);
                    }
                    WindowEvent::AxisMotion { value: _value, .. } => {
                        //dbg_logd!(_value);
                    }
                    _ => {}
                }
            }
            // Using device event for mouse motion because
            // - it reports delta, not position
            // - it doesn't care whether we're at the edge of the screen
            Event::DeviceEvent { event, .. } => {
                match event {
                    DeviceEvent::MouseMotion { delta } => {
                        // LATER This event normally happens every 4 ms for me when moving the mouse. Print stats.
                        // Is it limited by my polling rate? Would it be helpful to teach players how to increase it?
                        // Sometimes i get a batch of 4 events every 16 ms. Detect this.
                        // https://github.com/martin-t/rustcycles/issues/1
                        // dbg_logf!(
                        //     "{} DeviceEvent::MouseMotion {:?}",
                        //     clock.elapsed().as_secs_f32(),
                        //     delta
                        // );

                        // LATER This doesn't have enough precision, and neither do the other events.
                        // the smallest delta is a whole pixel.
                        // dbg_logd!(delta);

                        client.mouse_motion(delta);
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
                while let Some(_ui_message) = client.engine.user_interface.poll_message() {}
            }
            Event::RedrawRequested(_) => {
                client.engine.render().unwrap(); // LATER only crash if failed multiple times
            }
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => dbg_logf!("bye"),
        }
    });
}

fn server_main() {
    init_global_state("sv");

    let event_loop = EventLoop::new();
    let engine = init_engine_server(&event_loop);

    let mut server = executor::block_on(ServerProcess::new(engine));
    let clock = Instant::now();
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in case it changes.
        *control_flow = ControlFlow::Poll;

        // Exhaustively match all variants so we notice if the enum changes.
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
                while let Some(_ui_message) = server.engine.user_interface.poll_message() {}
            }
            Event::RedrawRequested(_) => {}
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => dbg_logf!("bye"),
        }
    });
}

fn init_global_state(endpoint_name: &'static str) {
    let color = match endpoint_name {
        "sv" => Color::GREEN,
        "cl" => Color::RED,
        _ => Color::WHITE,
    };

    DEBUG_ENDPOINT.with(|endpoint| {
        *endpoint.borrow_mut() = DebugEndpoint {
            name: endpoint_name,
            default_color: color,
        }
    });

    // LATER Switch rg3d to a more standard logger
    // or at least add a level below INFO so load times can remain as INFO
    // and the other messages are hidden by default.
    // Also used in server_main().
    Log::set_verbosity(MessageKind::Warning);
}

fn init_engine_client(event_loop: &EventLoop<()>, opts: Opts) -> Engine {
    let mut window_builder = WindowBuilder::new().with_title("RustCycles");
    if opts.windowed {
        let width = 600;
        let height = width / 16 * 9;
        // Currently using PhysicalSize because my Thinkpad scales logical size by 1.5
        // and it can't handle a large window at any reasonable FPS.
        window_builder = window_builder.with_inner_size(PhysicalSize::new(width, height));
    } else {
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }
    let serialization_context = Arc::new(SerializationContext::new());
    let resource_manager = ResourceManager::new(serialization_context.clone());

    // LATER no vsync
    Engine::new(EngineInitParams {
        window_builder,
        serialization_context,
        resource_manager,
        events_loop: event_loop,
        vsync: true,
    })
    .unwrap()
}

fn init_engine_server(event_loop: &EventLoop<()>) -> Engine {
    // LATER Headless - do all this without creating a window.
    let window_builder = WindowBuilder::new()
        .with_title("RustCycles server")
        .with_inner_size(LogicalSize::new(400, 100));
    let serialization_context = Arc::new(SerializationContext::new());
    let resource_manager = ResourceManager::new(serialization_context.clone());

    // LATER Does vsync have any effect here?
    Engine::new(EngineInitParams {
        window_builder,
        serialization_context,
        resource_manager,
        events_loop: event_loop,
        vsync: true,
    })
    .unwrap()
}
