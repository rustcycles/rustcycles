//! This file contains entry points and event loops for the native client and server.
//! If you're looking for the main _game_ loops, it's in ClientProcess and ServerProcess.

#![allow(clippy::option_map_unit_fn)] // Map is sometimes more readable.

// Keep this first so the macros are available everywhere without having to import them.
#[macro_use]
pub mod debug;

mod client;
mod common;
mod cvars;
mod prelude;
mod server;

use std::{env, error::Error, panic, process::Command, sync::Arc};

use fyrox::{
    asset::manager::ResourceManager,
    core::{
        futures::executor,
        log::{Log, MessageKind},
        task::TaskPool,
    },
    dpi::{LogicalSize, PhysicalSize},
    engine::{EngineInitParams, GraphicsContextParams, SerializationContext},
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    utils::translate_event,
    window::{Fullscreen, WindowBuilder},
};

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
//      - [x] Check all paths are lowercase (or we might have issues on windows)
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

#[derive(Debug)]
enum Endpoint {
    /// Run a local game (client and server in one process)
    Local,
    /// Run only the game client
    Client,
    /// Run only the game server
    Server,
}

fn main() -> Result<(), Box<dyn Error>> {
    // Sometimes people clone the repo and try `cargo run` without reading the instructions.
    // It has already happened back when using LFS, now it's the same with submodules.
    // Try to detect it and provide a nice error message.
    // LATER Would be nice to show a window with this message for people not using the terminal.
    //  This might also happen on MacOS due to their treatement of unsigned programs
    //  but we'll cross that bridge when we get there.
    if std::fs::read_dir("data").unwrap().count() == 0 {
        println!("The data directory is empty, this usually happens when the repository is cloned without submodules.");
        println!("Make sure to initialize the git submodule after cloning - run `git submodule update --init --recursive`.");
        println!("See README.md for details. Exiting...");
        std::process::exit(1);
    }

    // We are not using a derive-based library (anymore)
    // because they add a couple hundred ms to incremental debug builds.
    //
    // If hand parsing gets too complex, might wanna consider one of the libs here:
    // https://github.com/rosetta-rs/argparse-rosetta-rs
    let mut args = env::args().skip(1).peekable(); // Skip path to self
    let endpoint = match args.peek().map(String::as_str) {
        Some("launcher") => {
            args.next();
            None
        }
        Some("local") => {
            args.next();
            Some(Endpoint::Local)
        }
        Some("client") => {
            args.next();
            Some(Endpoint::Client)
        }
        Some("server") => {
            args.next();
            Some(Endpoint::Server)
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
        _ => None,
    };
    // Anything else, we assume it's a cvar.
    // Some games require cvars/commands to be prefixed by `+` which allows more specific error messages
    // because they know it's meant to be a cvar/command and not a malformed command line option.
    // We might wanna require that too but this is slightly less typing for now.
    let cvar_args = args.collect();

    match endpoint {
        // LATER None should launch client and offer choice in menu
        None => {
            init_global_state("launcher");
            client_server_main(cvar_args);
        }
        Some(Endpoint::Local) => {
            init_global_state("lo");
            let cvars = args_to_cvars(&cvar_args)?;
            client_main(cvars, true);
        }
        Some(Endpoint::Client) => {
            init_global_state("cl");
            let cvars = args_to_cvars(&cvar_args)?;
            client_main(cvars, false);
        }
        Some(Endpoint::Server) => {
            init_global_state("sv");
            let cvars = args_to_cvars(&cvar_args)?;
            server_main(cvars);
        }
    }

    Ok(())
}

fn init_global_state(endpoint_name: &'static str) {
    debug::set_endpoint(endpoint_name);

    // LATER Switch fyrox to a more standard logger
    // or at least add a level below INFO so load times can remain as INFO
    // and the other messages are hidden by default.
    Log::set_verbosity(MessageKind::Warning);

    // Log which endpoint panicked.
    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        dbg_logf!("panicking"); // No need to print panic_info here, it'll be printed later anyway.
        prev_hook(panic_info);
    }));
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
            format!("missing value for cvar `{cvar_name}` or incorrect command line option")
        })?;
        let res = cvars.set_str(cvar_name, str_value);
        match res.as_ref() {
            Ok(_) => {
                // Intentionally getting the new value from cvars, not just printing the input
                // so the user can check it was parsed correctly.
                dbg_logf!("{} = {}", cvar_name, cvars.get_string(cvar_name).unwrap());
            }
            Err(e) => {
                let msg = format!("failed to set cvar {cvar_name} to value {str_value}: {e}");
                if cvars.d_exit_on_unknown_cvar {
                    return Err(msg);
                } else {
                    dbg_logf!("WARNING {msg}");
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
fn client_server_main(cvar_args: Vec<String>) {
    let path = env::args().next().unwrap();

    let mut server_cmd = Command::new(&path);
    let mut client_cmd = Command::new(&path);

    server_cmd.arg("server");
    client_cmd.arg("client");

    for arg in &cvar_args {
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
fn client_main(cvars: Cvars, local_game: bool) {
    let engine = init_engine_client(&cvars);
    let mut client = executor::block_on(ClientProcess::new(cvars, engine, local_game));

    let event_loop = EventLoop::new().unwrap();
    // We have to use Poll instead of the default Wait because we need the main "loop" (i.e. this event handler)
    // to run as fast as possible so gamelogic updates run as soon as they should and don't lag behind real time.
    // Additionally, with Wait a headless process or a window which doesn't redraw wouldn't receive any events
    // unless the user moved the mouse or presses a key.
    // So the dedicated server and probably headless clients on CI need polling anyway.
    //
    // Polling gives events 70-80 times each *milli*second when doing nothing else beside printing their times.
    // With it, we can use AboutToWait to run updates as soon as needed.
    // The downside is we occupy a full CPU core (or 2 when there's also a server process).
    // LATER Offload gamelogic and rendering to another thread so input can be received at any time and sent to server immediately.
    // LATER Is there a way to not waste CPU cycles so much? WaitUntil and calculate how much time till next frame?
    //
    // This comment also applies to server_main.
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
        .run(move |event, window_target| {
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
                            window_target.exit();
                        }
                        WindowEvent::Focused(focus) => {
                            client.focused(focus);
                        }
                        WindowEvent::KeyboardInput { event, .. } => {
                            client.keyboard_input(&event);
                        }
                        WindowEvent::MouseWheel { delta, phase, .. } => {
                            client.mouse_wheel(delta, phase);
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            client.mouse_input(state, button);
                        }
                        WindowEvent::RedrawRequested => {
                            // This event never happens in headless mode.
                            // So don't put anything here except rendering (duh).

                            client.engine.render().unwrap();
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
                Event::Suspended => {
                    if !client.cvars.cl_headless {
                        client.engine.destroy_graphics_context().unwrap();
                    }
                }
                Event::Resumed => {
                    if !client.cvars.cl_headless {
                        client.engine.initialize_graphics_context(window_target).unwrap();
                    }
                }
                Event::AboutToWait => {
                    while let Some(msg) = client.engine.user_interface.poll_message() {
                        client.ui_message(&msg);
                    }
                    client.update(window_target);
                    if client.exit {
                        window_target.exit();
                    }
                }
                Event::LoopExiting => {
                    client.loop_exiting();
                }
                Event::MemoryWarning => {}
            }
        })
        .unwrap();
}

fn server_main(cvars: Cvars) {
    let engine = init_engine_server();
    let mut server = executor::block_on(ServerProcess::new(cvars, engine));

    let event_loop = EventLoop::new().unwrap();
    // See client_main for explanation.
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
        .run(move |event, window_target| {
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
                            window_target.exit();
                        }
                        _ => {}
                    }
                }
                Event::DeviceEvent { .. } => {}
                Event::UserEvent(_) => {}
                Event::Suspended => {
                    if !server.cvars.cl_headless {
                        server.engine.destroy_graphics_context().unwrap();
                    }
                }
                Event::Resumed => {
                    if !server.cvars.cl_headless {
                        server.engine.initialize_graphics_context(window_target).unwrap();
                    }
                }
                Event::AboutToWait => {
                    while let Some(_msg) = server.engine.user_interface.poll_message() {}
                    server.update(window_target);
                }
                Event::LoopExiting => dbg_logf!("bye"),
                Event::MemoryWarning => {}
            }
        })
        .unwrap();
}

fn init_engine_client(cvars: &Cvars) -> Engine {
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

    // LATER no vsync
    let task_pool = Arc::new(TaskPool::new());
    Engine::new(EngineInitParams {
        graphics_context_params: GraphicsContextParams {
            window_attributes: window_builder.window_attributes().clone(),
            vsync: cvars.cl_vsync,
        },
        serialization_context: Arc::new(SerializationContext::new()),
        resource_manager: ResourceManager::new(task_pool.clone()),
        task_pool,
    })
    .unwrap()
}

fn init_engine_server() -> Engine {
    let window_builder = WindowBuilder::new()
        .with_title("RustCycles server")
        .with_inner_size(LogicalSize::new(400, 100));

    let task_pool = Arc::new(TaskPool::new());
    Engine::new(EngineInitParams {
        graphics_context_params: GraphicsContextParams {
            window_attributes: window_builder.window_attributes().clone(),
            vsync: false, // Must be off when headless or weird things happen.
        },
        serialization_context: Arc::new(SerializationContext::new()),
        resource_manager: ResourceManager::new(task_pool.clone()),
        task_pool,
    })
    .unwrap()
}
