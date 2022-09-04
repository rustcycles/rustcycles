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
    core::instant::Instant,
    dpi::PhysicalSize,
    error::ExternalError,
    event::{ElementState, KeyboardInput, MouseButton},
    gui::{
        brush::Brush,
        formatted_text::WrapMode,
        message::{MessageDirection, UiMessage},
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
    renderer::QualitySettings,
    window::CursorGrabMode,
};

use crate::{
    client::{console::FyroxConsole, game::ClientGame},
    common::net::{LocalConnection, LocalListener, TcpConnection},
    cvars::Cvars,
    prelude::*,
    server::game::ServerGame,
};

/// The process that runs a player's game client.
pub(crate) struct ClientProcess {
    cvars: Cvars,
    clock: Instant,
    mouse_grabbed: bool,
    shift_pressed: bool,
    pub(crate) engine: Engine,
    console: FyroxConsole,
    debug_text: Handle<UiNode>,
    sg: Option<ServerGame>,
    cg: ClientGame,
}

impl ClientProcess {
    pub(crate) async fn new(cvars: Cvars, mut engine: Engine, local_game: bool) -> Self {
        let quality = match cvars.r_quality {
            0 => QualitySettings::low(),
            1 => QualitySettings::medium(),
            2 => QualitySettings::high(),
            _ => {
                dbg_logf!("Invalid r_quality value: {}", cvars.r_quality);
                QualitySettings::low()
            }
        };
        // LATER Allow changing quality at runtime
        engine.renderer.set_quality_settings(&quality).unwrap();

        let debug_text =
            TextBuilder::new(WidgetBuilder::new().with_foreground(Brush::Solid(Color::RED)))
                // Word wrap doesn't work if there's an extremely long word.
                .with_wrap(WrapMode::Letter)
                .build(&mut engine.user_interface.build_ctx());

        // Z index doesn't work, console has to be created after debug_text (and any other UI):
        // https://github.com/FyroxEngine/Fyrox/issues/356
        let console = FyroxConsole::new(&mut engine);

        let (sg, cg) = if local_game {
            // LATER Multithreading would be sweet but we can't use threads in WASM.

            let (tx1, rx1) = mpsc::channel();
            let (tx2, rx2) = mpsc::channel();
            let conn1 = LocalConnection::new(tx1, rx2);
            let conn2 = LocalConnection::new(tx2, rx1);

            // Init server first, otherwise the client has nothing to connect to.
            let listener = LocalListener::new(conn1);
            let mut sg = ServerGame::new(&mut engine, Box::new(listener)).await;

            // Make the server accept the local connection
            // and send init data into it so the client can read it during creation.
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
                // LATER Don't block the main thread - async?
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
            cvars,
            clock: Instant::now(),
            mouse_grabbed: false,
            shift_pressed: false,
            engine,
            console,
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

        self.console.resized(&mut self.engine, size);
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
        // This is problematic in other ways so here's a bunch of issues to follow:
        // https://github.com/rust-windowing/winit/issues/732:
        //  From my testing on Windows and Linux the letter/punctuation keys, number keys,
        //  function keys, and escape, space, shift, enter, tab, backspace, and caps keys
        //  are all identical across both platforms, but various other keys
        //  (such as arrow keys) weren't consistent.
        // https://github.com/rust-windowing/winit/issues/2436:
        //  Expose ScanCode to VirtualKeyCode mapping
        // https://github.com/bevyengine/bevy/discussions/2386:
        //  Naming problems: KeyboardInput Scancodes/Virtual Scancodes
        // https://github.com/bevyengine/bevy/issues/2052
        //  Improve keyboard input with Input<ScanCode>

        self.client_input(input);
        if self.console.is_open() {
            self.console.keyboard_input(&mut self.cvars, &mut self.engine, input);
        } else {
            self.game_input(input);
        }
    }

    /// Input that is handled regardless of whether we're in menu/console/game.
    fn client_input(&mut self, input: KeyboardInput) {
        use scan_codes::*;

        let pressed = input.state == ElementState::Pressed;

        match input.scancode {
            ESC if pressed => {
                if self.console.is_open() {
                    self.console.close(&mut self.engine);
                    self.set_mouse_grab(true);
                } else if self.shift_pressed {
                    self.console.open(&mut self.engine);
                    self.set_mouse_grab(false);
                }
            }
            BACKTICK if pressed => {
                if !self.console.is_open() {
                    self.console.open(&mut self.engine);
                    self.set_mouse_grab(false);
                }
            }
            SHIFT => self.shift_pressed = pressed,
            _ => (),
        }
    }

    /// Input that is handdled only when we're in game.
    fn game_input(&mut self, input: KeyboardInput) {
        use scan_codes::*;

        let pressed = input.state == ElementState::Pressed;

        match input.scancode {
            W => self.cg.lp.input.forward = pressed,
            A => self.cg.lp.input.left = pressed,
            S => self.cg.lp.input.backward = pressed,
            D => self.cg.lp.input.right = pressed,
            SPACE => self.cg.lp.input.up = pressed,
            SHIFT => self.cg.lp.input.down = pressed,

            // Don't print anything for these, it just spams stdout.
            ESC | TAB | CTRL | ALT | BACKSLASH | Z => {}
            F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => {}

            c => {
                // LATER This is for easier debugging, allow disabling via cvars
                if pressed {
                    dbg_logf!(
                        "pressed unhandled scancode: {} (virtual key code: {:?})",
                        c,
                        input.virtual_keycode
                    );
                }
            }
        }

        self.cg.lp.input.real_time = self.real_time();
        self.cg.lp.input.game_time = self.cg.gs.game_time;
        self.cg.send_input();
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if self.console.is_open() {
            return;
        }

        self.set_mouse_grab(true);

        let pressed = state == ElementState::Pressed;
        match button {
            MouseButton::Left => self.cg.lp.input.fire1 = pressed,
            MouseButton::Right => self.cg.lp.input.fire2 = pressed,
            MouseButton::Middle => self.cg.lp.input.zoom = pressed,
            MouseButton::Other(_) => {}
        }

        self.cg.lp.input.real_time = self.real_time();
        self.cg.lp.input.game_time = self.cg.gs.game_time;
        self.cg.send_input();
    }

    /// Either grab mouse and hide cursor
    /// or ungrab mouse and show cursor.
    fn set_mouse_grab(&mut self, grab: bool) {
        // LATER Don't hide cursor in menu.
        if grab != self.mouse_grabbed {
            let window = self.engine.get_window();
            let res = window.set_cursor_grab(CursorGrabMode::Confined);
            match res {
                Ok(_) | Err(ExternalError::NotSupported(_)) => {}
                Err(_) => res.unwrap(),
            }
            window.set_cursor_visible(!grab);
            self.mouse_grabbed = grab;
        }
    }

    pub(crate) fn mouse_motion(&mut self, delta: (f64, f64)) {
        if self.console.is_open() {
            return;
        }

        if !self.mouse_grabbed {
            // LATER (privacy) Recheck we're not handling mouse movement when minimized
            //  (and especially not sending to server)
            return;
        }

        // Events don't come at a constant rate, they often seem to bunch up.
        // We don't know the time when they were generated, only when we handle them here.
        // So there's no point trying to calculate things like mouse speed
        // based on real time from last event. Instead, save the cumulative delta
        // and update angles/speeds once per frame.
        //
        // LATER Might be because the main thread is blocked running game logic.
        //  Update this comment after separating things to threads.

        // LATER cvars
        let mouse_sensitivity_horizontal = 0.5;
        let mouse_sensitivity_vertical = 0.5;
        let zoom_factor = if self.cg.lp.input.zoom { 4.0 } else { 1.0 };

        // Subtract, don't add the delta - nalgebra rotations are counterclockwise.
        let delta_yaw = -delta.0 as f32 * mouse_sensitivity_horizontal / zoom_factor;
        let delta_pitch = delta.1 as f32 * mouse_sensitivity_vertical / zoom_factor;

        self.cg.lp.delta_yaw += delta_yaw;
        self.cg.lp.delta_pitch += delta_pitch;
    }

    pub(crate) fn ui_message(&mut self, ui_message: UiMessage) {
        if ui_message.direction == MessageDirection::ToWidget {
            return;
        }

        if let Some(
            WidgetMessage::MouseMove { .. } | WidgetMessage::MouseEnter | WidgetMessage::MouseLeave,
        ) = ui_message.data()
        {
            return;
        }

        if self.cvars.d_ui_messages {
            // LATER dbg_logdp for pretty printing
            dbg!(&ui_message);
        }

        // LATER This is_open() is a hack around the fact fyrox can't force the prompt to be focused.
        // When the console is open, all input should go to it.
        if self.console.is_open() {
            self.console.ui_message(&mut self.engine, &mut self.cvars, ui_message);
        }
    }

    pub(crate) fn update(&mut self) {
        let target = self.real_time();
        self.cg.update(&self.cvars, &mut self.engine, target);
        let target = self.real_time(); // Borrowck dance
        if let Some(sg) = &mut self.sg {
            sg.update(&mut self.engine, target)
        }
    }

    pub(crate) fn real_time(&self) -> f32 {
        self.clock.elapsed().as_secs_f32()
    }
}

mod scan_codes {
    #![allow(dead_code)]

    use fyrox::event::ScanCode;

    // Apparently there are different numbering schemes all called "scancodes".
    // This image is mostly accurate for the one in winit:
    // https://forum.thegamecreators.com/thread/145420
    // Note that some keys are different (e.g. R_ALT, KP_ENTER)

    pub(crate) const ESC: ScanCode = 1;
    pub(crate) const TAB: ScanCode = 15;
    pub(crate) const W: ScanCode = 17;
    pub(crate) const ENTER: ScanCode = 28;
    pub(crate) const CTRL: ScanCode = 29;
    pub(crate) const A: ScanCode = 30;
    pub(crate) const S: ScanCode = 31;
    pub(crate) const D: ScanCode = 32;
    pub(crate) const BACKTICK: ScanCode = 41;
    pub(crate) const SHIFT: ScanCode = 42;
    pub(crate) const Z: ScanCode = 44;
    pub(crate) const ALT: ScanCode = 56;
    pub(crate) const SPACE: ScanCode = 57;
    pub(crate) const F1: ScanCode = 59;
    pub(crate) const F2: ScanCode = 60;
    pub(crate) const F3: ScanCode = 61;
    pub(crate) const F4: ScanCode = 62;
    pub(crate) const F5: ScanCode = 63;
    pub(crate) const F6: ScanCode = 64;
    pub(crate) const F7: ScanCode = 65;
    pub(crate) const F8: ScanCode = 66;
    pub(crate) const F9: ScanCode = 67;
    pub(crate) const F10: ScanCode = 68;
    pub(crate) const F11: ScanCode = 69;
    pub(crate) const F12: ScanCode = 70;
    pub(crate) const BACKSLASH: ScanCode = 86;
    pub(crate) const KP_ENTER: ScanCode = 96;
    pub(crate) const UP_ARROW: ScanCode = 103;
    pub(crate) const PG_UP: ScanCode = 104;
    pub(crate) const DOWN_ARROW: ScanCode = 108;
    pub(crate) const PG_DOWN: ScanCode = 109;
}
