//! This file contains entry points and event loops for the native client and server.
//! If you're looking for the main _game_ loops, it's in ClientProcess and ServerProcess.

#![allow(clippy::option_map_unit_fn)] // Map is sometimes more readable.

// Keep this first so the macros are available everywhere without having to import them.
#[macro_use]
pub(crate) mod debug;

mod client;
mod common;
mod cvars;
mod prelude;
mod server;

use std::{env, error::Error, panic, process::Command, sync::Arc};

use fyrox::{
    core::futures::executor,
    dpi::{LogicalSize, PhysicalSize},
    engine::{resource_manager::ResourceManager, Engine, EngineInitParams, SerializationContext},
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    utils::{
        log::{Log, MessageKind},
        translate_event,
    },
    window::{Fullscreen, WindowBuilder},
};
use strum_macros::EnumString;

use crate::{client::process::ClientProcess, prelude::*, server::process::ServerProcess};

// Master TODO list:
// v0.1 - MVP:
//  - [x] Arena and wheel models
//  - [x] Rotate the camera
//  - [x] Move the camera
//  - [x] Render wheel at player pos
//  - [x] Primitive networking to force client/server split
//  - [ ] Driving and collisions
//  - [ ] Trails
//  - [ ] WASM local client for testing / showcases
//      - [ ] Client and server in one process - local gameplay
// yak-shaving:
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
//  - [ ] Maybe remove all unwraps - go through all the code, convert infallible ones to {soft,hard}_unwrap, fallible ones to Result.
//  - [x] Remove all prints and dbgs, convert them to a proper logger impl which differentiates client and server logs.
//  - [ ] If possible, lint against unwrap, print, println, dbg,
//          todo, panic, unreachable, unimplemented, ... See debug.rs for alternatives.

#[derive(Debug, Default)]
struct Opts {
    /// Whether to run the client, server or both.
    endpoint: Option<Endpoint>,

    // LATER Fix examples
    /// Set cvar values - use key value pairs (separated by space).
    /// Example: g_armor 150 hud_names false
    cvar_args: Vec<String>,
}

#[derive(Debug, EnumString)]
enum Endpoint {
    /// Run a local game (client and server in one process)
    Local,
    /// Run only the game client
    Client,
    /// Run only the game server
    Server,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut opts = Opts::default();

    // We are not using a derive-based library (anymore)
    // because they add a couple hundred ms to incremental debug builds.
    //
    // If hand parsing gets too complex, might wanna consider one of the libs here:
    // https://github.com/rosetta-rs/argparse-rosetta-rs
    //
    // LATER Add --help and --version
    let mut args = env::args().skip(1).peekable(); // Skip path to self
    match args.peek().map(String::as_str) {
        Some("launcher") => {
            args.next();
        }
        Some("local") => {
            opts.endpoint = Some(Endpoint::Local);
            args.next();
        }
        Some("client") => {
            opts.endpoint = Some(Endpoint::Client);
            args.next();
        }
        Some("server") => {
            opts.endpoint = Some(Endpoint::Server);
            args.next();
        }
        #[rustfmt::skip]
        Some("--help") => {
            println!("Usage: rustcycles [launcher|local|client|server] [cvar1 value1 cvar2 value2 ...]");
            println!();
            println!("Commands (optional):");
            println!("    launcher   Run a local game with separate client and server processes (default)");
            println!("    local      Run a local game with client and server in one process (experimental)");
            println!("    client     Run only the game client");
            println!("    server     Run only the dedicated game server");
            println!();
            println!("Cvars (optional):");
            println!("    You can specify cvars in key value pairs separated by space.");
            println!("    Example: rustcycles cl_camera_fov 100 m_sensitivity 0.8");
            println!();
            println!("    Cvars can be changed at runtime using the console but some of them");
            println!("    are only read at startup so the value needs to be specified");
            println!("    on the command line to take effect");
            println!();
            // LATER ^ Reloading the map should also work.
            return Ok(());
        }
        Some("--version") => {
            // LATER Would be nice to print git hash and dirty status here.
            // Find a way to do that without increasing compile times or only do that in release builds.
            // Note that it's especially annoying when dirty status changes and forces a rebuild.
            // Maybe also include time of build.
            // https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-crates
            println!("RustCycles {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(arg) if arg.starts_with('-') => {
            panic!("Unknown option: {}", arg);
        }
        _ => {}
    }
    // Anything else, we assume it's a cvar.
    // Some games require cvars to be prefixed by `+` which allows more specific error messages
    // because they know it's meant to be a cvar and not a malformed command line option.
    // We might wanna require that too but this is slightly less typing for now.
    opts.cvar_args = args.collect();

    match opts.endpoint {
        // LATER None should launch client and offer choice in menu
        None => {
            init_global_state("launcher");
            client_server_main(opts);
        }
        Some(Endpoint::Local) => {
            init_global_state("lo");
            let cvars = args_to_cvars(&opts.cvar_args)?;
            client_main(cvars, true);
        }
        Some(Endpoint::Client) => {
            init_global_state("cl");
            let cvars = args_to_cvars(&opts.cvar_args)?;
            client_main(cvars, false);
        }
        Some(Endpoint::Server) => {
            init_global_state("sv");
            let cvars = args_to_cvars(&opts.cvar_args)?;
            server_main(cvars);
        }
    }

    Ok(())
}

fn init_global_state(endpoint_name: &'static str) {
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        dbg_logf!("panicking");
        prev_hook(panic_info);
    }));

    debug::details::set_endpoint(endpoint_name);

    // LATER Switch fyrox to a more standard logger
    // or at least add a level below INFO so load times can remain as INFO
    // and the other messages are hidden by default.
    Log::set_verbosity(MessageKind::Warning);
}

fn args_to_cvars(cvar_args: &[String]) -> Result<Cvars, String> {
    let mut cvars = Cvars::default();

    let mut cvars_iter = cvar_args.iter();
    while let Some(cvar_name) = cvars_iter.next() {
        // Cvar names can optionally be prefixed by '+'.
        let mut cvar_name = cvar_name.as_str();
        if cvar_name.starts_with('+') {
            cvar_name = &cvar_name[1..];
        }

        let str_value = cvars_iter.next().ok_or_else(|| {
            format!("missing value for cvar `{}` or incorrect command line option", cvar_name)
        })?;
        let res = cvars.set_str(cvar_name, str_value);
        match res.as_ref() {
            Ok(_) => {
                // Intentionally getting the new value from cvars, not just printing the input
                // so the user can check it was parsed correctly.
                dbg_logf!("{} = {}", cvar_name, cvars.get_string(cvar_name).unwrap());
            }
            Err(msg) => {
                if cvars.d_exit_on_unknown_cvar {
                    return Err(format!(
                        "failed to set cvar {} to value {}: {}",
                        cvar_name, str_value, msg
                    ));
                } else {
                    dbg_logf!("failed to set cvar {} to value {}: {}", cvar_name, str_value, msg);
                }
            }
        }
    }

    Ok(cvars)
}

/// Run both client and server.
///
/// This is just a convenience for quicker testing.
/// It spawns 2 processes to make sure the other is killed if one crashes.
///
/// LATER It should do that explicitly, right now it only kills the server
/// because client quits without a server anyway.
fn client_server_main(opts: Opts) {
    // This is broken - most input gets ignored (on Kubuntu):
    // thread::spawn(|| {
    //     // LATER EventLoop::new_any_thread is Unix only, what happens on Windows?
    //     server_main(EventLoop::new_any_thread());
    // });
    // thread::sleep(Duration::from_secs(1));
    // client_main();

    let path = env::args().next().unwrap();

    let mut server_cmd = Command::new(&path);
    let mut client_cmd = Command::new(&path);

    server_cmd.arg("server");
    client_cmd.arg("client");

    for arg in &opts.cvar_args {
        server_cmd.arg(arg);
        client_cmd.arg(arg);
    }

    let mut server = server_cmd.spawn().unwrap();
    // Sleep so the client window appears later and gets focus.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let mut client = client_cmd.spawn().unwrap();

    // We wanna close just the client and automatically close the server that way.
    client.wait().unwrap();
    dbg_logf!("Client exited, killing server");
    server.kill().unwrap();
}

/// LATER Do we want a shared game state or just running both
/// client and server in one thread? Update docs on Endpoint or wherever.
fn client_main(cvars: Cvars, local_server: bool) {
    let event_loop = EventLoop::new();
    let engine = init_engine_client(&event_loop, &cvars);

    let mut client = executor::block_on(ClientProcess::new(cvars, engine, local_server));
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
                if let Some(os_event) = translate_event(&event) {
                    client.engine.user_interface.process_os_event(&os_event);
                }

                match event {
                    WindowEvent::Resized(size) => {
                        client.resized(size);
                    }
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Focused(focus) => {
                        client.focused(focus);
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        client.keyboard_input(input);
                    }
                    WindowEvent::MouseWheel { delta, phase, .. } => {
                        client.mouse_wheel(delta, phase);
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        client.mouse_input(state, button);
                    }
                    _ => {}
                }
            }
            // Using device event for mouse motion because
            // - it reports delta, not position
            // - it doesn't care whether we're at the edge of the screen
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    client.mouse_motion(delta);
                }
                _ => {}
            },
            Event::UserEvent(_) => {}
            // LATER test suspend/resume
            Event::Suspended => {}
            Event::Resumed => {}
            Event::MainEventsCleared => {
                while let Some(msg) = client.engine.user_interface.poll_message() {
                    client.ui_message(&msg);
                }
                client.update();
            }
            Event::RedrawRequested(_) => {
                client.engine.render().unwrap(); // LATER only crash if failed multiple times
            }
            Event::RedrawEventsCleared => {
                if client.exit {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::LoopDestroyed => {
                client.loop_destroyed();
            }
        }
    });
}

fn server_main(cvars: Cvars) {
    let event_loop = EventLoop::new();
    let engine = init_engine_server(&event_loop);

    let mut server = executor::block_on(ServerProcess::new(cvars, engine));
    event_loop.run(move |event, _, control_flow| {
        // Default control_flow is ControllFlow::Poll but let's be explicit in case it changes.
        *control_flow = ControlFlow::Poll;

        // Exhaustively match all variants so we notice if the enum changes.
        #[allow(clippy::single_match)]
        match event {
            Event::NewEvents(_) => {}
            Event::WindowEvent { event, .. } => {
                if let Some(os_event) = translate_event(&event) {
                    server.engine.user_interface.process_os_event(&os_event);
                }

                match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    _ => {}
                }
            }
            Event::DeviceEvent { .. } => {}
            Event::UserEvent(_) => {}
            Event::Suspended => {}
            Event::Resumed => {}
            Event::MainEventsCleared => {
                server.update();
                while let Some(_msg) = server.engine.user_interface.poll_message() {}
            }
            Event::RedrawRequested(_) => {}
            Event::RedrawEventsCleared => {}
            Event::LoopDestroyed => dbg_logf!("bye"),
        }
    });
}

fn init_engine_client(event_loop: &EventLoop<()>, cvars: &Cvars) -> Engine {
    let mut window_builder = WindowBuilder::new().with_title("RustCycles");
    if cvars.cl_fullscreen {
        // Borderless is preferred on macOS.
        window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    } else {
        let width = cvars.cl_window_width;
        let height = cvars.cl_window_height;
        // Using PhysicalSize seems more ... logical, if we let users configure it in pixels.
        window_builder = window_builder.with_inner_size(PhysicalSize::new(width, height));
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
        headless: cvars.cl_headless,
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
        headless: true,
    })
    .unwrap()
}
