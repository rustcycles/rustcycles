//! The process that runs a player's game client.
//!
//! Handles stuff like windowing, input, etc. but not game logic.
//! When connected to a remote server, contains a game client.
//! When playing locally, contains both a client and a server.

use std::{
    net::{SocketAddr, TcpStream},
    str::FromStr,
    sync::mpsc,
    thread,
    time::Duration,
};

use fyrox::{
    dpi::PhysicalSize,
    error::ExternalError,
    event::{ElementState, KeyboardInput, MouseButton, ScanCode},
    gui::{
        brush::Brush,
        formatted_text::WrapMode,
        message::MessageDirection,
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
};

use crate::{
    client::game::ClientGame,
    common::net::{LocalConnection, LocalListener, TcpConnection},
    prelude::*,
    server::game::ServerGame,
};

/// The process that runs a player's game client.
pub(crate) struct ClientProcess {
    pub(crate) mouse_grabbed: bool,
    pub(crate) engine: Engine,
    debug_text: Handle<UiNode>,
    sg: Option<ServerGame>,
    cg: ClientGame,
}

impl ClientProcess {
    pub(crate) async fn new(mut engine: Engine, local_game: bool) -> Self {
        let debug_text =
            TextBuilder::new(WidgetBuilder::new().with_foreground(Brush::Solid(Color::RED)))
                // Word wrap doesn't work if there's an extremely long word.
                .with_wrap(WrapMode::Letter)
                .build(&mut engine.user_interface.build_ctx());

        let (sg, cg) = if local_game {
            // LATER Multithreading would be sweet, would it help perf on WASM?

            let (tx1, rx1) = mpsc::channel();
            let (tx2, rx2) = mpsc::channel();
            let conn1 = LocalConnection::new(tx1, rx2);
            let conn2 = LocalConnection::new(tx2, rx1);

            // Init server first, otherwise the client has nothing to connect to.
            let listener = LocalListener::new(conn1);
            let mut sg = ServerGame::new(&mut engine, Box::new(listener)).await;

            // Make the server accept the local connection
            // and push init data into it so the client can read it during creation.
            // Otherwise the client would remain stuck.
            // Yes, this is really ugly.
            sg.accept_new_connections(&mut engine);

            let cg = ClientGame::new(&mut engine, debug_text, Box::new(conn2)).await;

            (Some(sg), cg)
        } else {
            let addr = SocketAddr::from_str("127.0.0.1:26000").unwrap();

            let mut connect_attempts = 0;
            let stream = loop {
                connect_attempts += 1;
                // LATER Don't block the main thread - no sleep in async
                // LATER Limit the number of attempts.
                if let Ok(stream) = TcpStream::connect(addr) {
                    dbg_logf!("connect attempts: {}", connect_attempts);
                    break stream;
                }
                if connect_attempts % 100 == 0 {
                    dbg_logf!("connect attempts: {}", connect_attempts);
                }
                thread::sleep(Duration::from_millis(10));
            };
            stream.set_nodelay(true).unwrap();
            stream.set_nonblocking(true).unwrap();

            let conn = TcpConnection::new(stream, addr);
            let cg = ClientGame::new(&mut engine, debug_text, Box::new(conn)).await;

            (None, cg)
        };

        Self {
            mouse_grabbed: false,
            engine,
            debug_text,
            sg,
            cg,
        }
    }

    pub(crate) fn resized(&mut self, size: PhysicalSize<u32>) {
        // This is also called when the window is first created.

        self.engine.set_frame_size(size.into()).unwrap();

        // mrDIMAS on discord:
        // The root element of the UI is Canvas,
        // it has infinite constraints so it does not stretch its contents.
        // If you'll have some complex UI, I'd advise you to create either
        // a window-sized Border or Grid and attach all your ui elements to it,
        // instead of root canvas.
        self.engine.user_interface.send_message(WidgetMessage::width(
            self.debug_text,
            MessageDirection::ToWidget,
            size.width as f32,
        ));
    }

    pub(crate) fn focused(&mut self, focus: bool) {
        // Ungrab here is needed in addition to ESC,
        // otherwise the mouse stays grabbed when alt+tabbing to other windows.
        // However, don't automatically grab it when gaining focus,
        // the game can get stuck in a loop (bugs like this are most common on startup)
        // and it would never ungrab.
        if !focus {
            self.set_mouse_grab(false);
        }

        // LATER pause/unpause
    }

    pub(crate) fn keyboard_input(&mut self, input: KeyboardInput) {
        // Use scancodes, not virtual keys, because they don't depend on layout.
        const ESC: ScanCode = 1;
        const TAB: ScanCode = 15;
        const W: ScanCode = 17;
        const CTRL: ScanCode = 29;
        const A: ScanCode = 30;
        const S: ScanCode = 31;
        const D: ScanCode = 32;
        const SHIFT: ScanCode = 42;
        const Z: ScanCode = 44;
        const ALT: ScanCode = 56;
        const BACKSLASH: ScanCode = 86;
        let pressed = input.state == ElementState::Pressed;
        match input.scancode {
            ESC => self.set_mouse_grab(false),
            W => self.cg.lp.input.forward = pressed,
            A => self.cg.lp.input.left = pressed,
            S => self.cg.lp.input.backward = pressed,
            D => self.cg.lp.input.right = pressed,
            TAB | SHIFT | CTRL | ALT | BACKSLASH | Z => {
                // Don't print anything, it just spams stdout when switching windows.
            }
            c => {
                if pressed {
                    dbg_logf!("pressed unhandled scancode: {}", c);
                }
            }
        }

        self.cg.send_input();
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        self.set_mouse_grab(true);

        let pressed = state == ElementState::Pressed;
        match button {
            MouseButton::Left => self.cg.lp.input.fire1 = pressed,
            MouseButton::Right => self.cg.lp.input.fire2 = pressed,
            MouseButton::Middle => self.cg.lp.input.zoom = pressed,
            MouseButton::Other(_) => {}
        }

        self.cg.send_input();
    }

    pub(crate) fn mouse_motion(&mut self, delta: (f64, f64)) {
        if !self.mouse_grabbed {
            // LATER (privacy) Recheck we're not handling mouse movement when minimized
            //  (and especially not sending to server)
            return;
        }

        // LATER cvars
        let mouse_sensitivity_horizontal = 0.5;
        let mouse_sensitivity_vertical = 0.5;
        let zoom_factor = if self.cg.lp.input.zoom { 0.25 } else { 1.0 };
        let delta_yaw = delta.0 as f32 * mouse_sensitivity_horizontal * zoom_factor;
        let delta_pitch = delta.1 as f32 * mouse_sensitivity_vertical * zoom_factor;

        // Subtract, don't add the delta X.
        // Nalgebra rotations follow the right hand rule,
        // thumb points in +Z, the curl of fingers shows direction.
        self.cg.lp.input.yaw.0 -= delta_yaw; // LATER Normalize to [0, 360°) or something
        self.cg.lp.input.pitch.0 = (self.cg.lp.input.pitch.0 + delta_pitch).clamp(-90.0, 90.0);
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

    pub(crate) fn update(&mut self, dt: f32) {
        self.cg.update(&mut self.engine, dt);
        if let Some(sg) = &mut self.sg {
            sg.update(&mut self.engine, dt)
        }
    }
}
