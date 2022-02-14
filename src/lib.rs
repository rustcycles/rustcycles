//! Entry point for WASM builds.
//! Native builds live in main.rs.

#![cfg(target_arch = "wasm32")] //FIXME needed?

// Keep this first so the macros are available everywhere without having to import them.
#[macro_use]
pub(crate) mod debug;

mod client;
mod common;
mod prelude;
mod server;

use std::{
    panic::{self, PanicInfo},
    todo,
};

use fyrox::{
    core::{
        instant::Instant,
        wasm_bindgen::{self, prelude::*},
    },
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    utils::log::{Log, MessageKind},
    window::{Fullscreen, WindowBuilder},
};

use crate::{
    client::GameClient,
    debug::details::{DebugEndpoint, DEBUG_ENDPOINT},
    prelude::*,
};

// FIXME what is this
// https://docs.rs/console_error_panic_hook/0.1.6/src/console_error_panic_hook/lib.rs.html
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn error(msg: String);

    type Error;

    #[wasm_bindgen(constructor)]
    fn new() -> Error;

    #[wasm_bindgen(structural, method, getter)]
    fn stack(error: &Error) -> String;
}

fn hook_impl(info: &PanicInfo) {
    let mut msg = info.to_string();

    // Add the error stack to our message.
    //
    // This ensures that even if the `console` implementation doesn't
    // include stacks for `console.error`, the stack is still available
    // for the user. Additionally, Firefox's console tries to clean up
    // stack traces, and ruins Rust symbols in the process
    // (https://bugzilla.mozilla.org/show_bug.cgi?id=1519569) but since
    // it only touches the logged message's associated stack, and not
    // the message's contents, by including the stack in the message
    // contents we make sure it is available to the user.
    msg.push_str("\n\nStack:\n\n");
    let e = Error::new();
    let stack = e.stack();
    msg.push_str(&stack);

    // Safari's devtools, on the other hand, _do_ mess with logged
    // messages' contents, so we attempt to break their heuristics for
    // doing that by appending some whitespace.
    // https://github.com/rustwasm/console_error_panic_hook/issues/7
    msg.push_str("\n\n");

    // Finally, log the panic with `console.error`!
    error(msg);
}

/// A panic hook for use with
/// [`std::panic::set_hook`](https://doc.rust-lang.org/nightly/std/panic/fn.set_hook.html)
/// that logs panics into
/// [`console.error`](https://developer.mozilla.org/en-US/docs/Web/API/Console/error).
///
/// On non-wasm targets, prints the panic to `stderr`.
pub fn hook(info: &PanicInfo) {
    hook_impl(info);
}

/// Set the `console.error` panic hook the first time this is called. Subsequent
/// invocations do nothing.
#[inline]
pub fn set_once() {
    use std::sync::Once;
    static SET_HOOK: Once = Once::new();
    SET_HOOK.call_once(|| {
        panic::set_hook(Box::new(hook));
    });
}

#[wasm_bindgen]
pub fn client_main() {
    set_once();

    DEBUG_ENDPOINT.with(|endpoint| {
        *endpoint.borrow_mut() = DebugEndpoint {
            name: "cl",
            default_color: Color::RED,
        }
    });

    // LATER Switch rg3d to a more standard logger
    // or at least add a level below INFO so load times can remain as INFO
    // and the other messages are hidden by default.
    // Also used in server_main().
    Log::set_verbosity(MessageKind::Warning);

    // FIXME copied verbatim:
    //  Same for server.
    //  Need 2 game states and messages between them.
    //      - Alternative is just dumping messages into replay file but maybe later.
    //      - For now 2 game states much easier.
    //      - Maybe split GameClient and ClientProgram? Then SharedProgram?
    //      - Wanna keep stuff like input in one place though.
    let mut window_builder = WindowBuilder::new().with_title("RustCycles");
    //if !opts.windowed {
    //    window_builder = window_builder.with_fullscreen(Some(Fullscreen::Borderless(None)));
    //}
    let event_loop = EventLoop::new();
    // LATER no vsync
    let engine = Engine::new(window_builder, &event_loop, true).unwrap();
    let mut client = fyrox::core::futures::executor::block_on(GameClient::new(engine));

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
                        // Sometimes i get 4 events every 16 ms. Detect this.
                        // https://github.com/martin-t/rustcycles/issues/1
                        // dbg_logf!(
                        //     "{} DeviceEvent::MouseMotion {:?}",
                        //     clock.elapsed().as_secs_f32(),
                        //     delta
                        // );

                        // LATER This doesn't have enough precision,
                        //  and neither do the other events.
                        dbg_logd!(delta);

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

#[wasm_bindgen]
fn server_main() {
    set_once();

    DEBUG_ENDPOINT.with(|endpoint| {
        *endpoint.borrow_mut() = DebugEndpoint {
            name: "sv",
            default_color: Color::GREEN,
        }
    });

    // See note in client_main().
    Log::set_verbosity(MessageKind::Warning);

    // LATER Headless - do all this without creating a window.
    let window_builder = WindowBuilder::new()
        .with_title("RustCycles server")
        .with_inner_size(LogicalSize::new(400, 100));
    let event_loop = EventLoop::new();
    // LATER Does vsync have any effect here?
    let engine = Engine::new(window_builder, &event_loop, false).unwrap();
    let mut server = fyrox::core::futures::executor::block_on(GameServer::new(engine));

    // Render pure black just once so the window doesn't look broken.
    server.engine.render().unwrap();

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
