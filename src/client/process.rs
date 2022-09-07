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
        if focus {
            if self.cvars.cl_mouse_grab_on_focus && !self.console.is_open() {
                self.set_mouse_grab(true);
            }
        } else {
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

        if self.cvars.d_keyboard_input {
            dbg_logf!("{} keyboard_input: {:?}", self.real_time(), input);
        }

        self.client_input(input);
        if !self.console.is_open() {
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
                    // With shift or without, ESC closes an open console.
                    let grab = self.console.close(&mut self.engine);
                    self.set_mouse_grab(grab);
                } else if self.shift_pressed {
                    // Shift + ESC is a common shortcut to open the console in games.
                    self.console.open(&mut self.engine, self.mouse_grabbed);
                    self.set_mouse_grab(false);

                    // Kinda hacky but at least this is the only 2-key shortcut
                    // so shift is the only such special case.
                    self.cg.lp.input.down = false;
                } else {
                    // ESC anywhere else just ungrabs the mouse.
                    self.set_mouse_grab(false);
                }
            }
            BACKTICK if pressed => {
                if !self.console.is_open() {
                    self.console.open(&mut self.engine, self.mouse_grabbed);
                    self.set_mouse_grab(false);
                }
            }
            L_SHIFT => self.shift_pressed = pressed,
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
            L_SHIFT => self.cg.lp.input.down = pressed, // LATER Unhardcode release on shift+ESC
            Q => self.cg.lp.input.prev_weapon = pressed,
            E => self.cg.lp.input.next_weapon = pressed,
            R => self.cg.lp.input.reload = pressed,
            F => self.cg.lp.input.flag = pressed,
            G => self.cg.lp.input.grenade = pressed,
            M => self.cg.lp.input.map = pressed,
            TAB => self.cg.lp.input.score = pressed,
            ENTER => self.cg.lp.input.chat = pressed,
            PAUSE => self.cg.lp.input.pause = pressed,
            F12 => self.cg.lp.input.screenshot = pressed,
            _ => (),
        }

        self.cg.lp.input.real_time = self.real_time();
        self.cg.lp.input.game_time = self.cg.gs.game_time;
        self.cg.send_input();
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if self.cvars.d_mouse_input {
            dbg_logf!("{} mouse_input: {:?} {:?}", self.real_time(), state, button);
        }

        if !self.console.is_open() {
            self.set_mouse_grab(true);

            let pressed = state == ElementState::Pressed;
            match button {
                MouseButton::Left => self.cg.lp.input.fire1 = pressed,
                MouseButton::Right => self.cg.lp.input.fire2 = pressed,
                MouseButton::Middle => self.cg.lp.input.zoom = pressed,
                MouseButton::Other(8) => self.cg.lp.input.marker1 = pressed,
                MouseButton::Other(9) => self.cg.lp.input.marker2 = pressed,
                MouseButton::Other(_) => {}
            }

            self.cg.lp.input.real_time = self.real_time();
            self.cg.lp.input.game_time = self.cg.gs.game_time;
            self.cg.send_input();
        }
    }

    /// Either grab mouse and hide cursor
    /// or ungrab mouse and show cursor.
    fn set_mouse_grab(&mut self, grab: bool) {
        // LATER Don't hide cursor in menu.

        // Don't exit early if grab == self.mouse_grabbed here.
        // It's possible to get into weird states (e.g. when opening KDE's Klipper tool by a shortcut)
        // where self.mouse_grabbed is incorrect and we'd need to press ESC and then click to regrab.

        let window = self.engine.get_window();
        if grab {
            #[cfg(target_os = "macos")]
            let mode = CursorGrabMode::Locked;

            #[cfg(not(target_os = "macos"))]
            let mode = CursorGrabMode::Confined;

            let res = window.set_cursor_grab(mode);
            if let Err(e) = res {
                // This happens when opening KDE's Klipper using Ctrl+Alt+V while mouse is *not* grabbed.
                // It seems that we first lose focus, then gain it, then lose it again.
                // I don't know why and I don't care, not my bug, just ignore it.
                dbg_logf!("Failed to grab mouse (mode {:?}): {}", mode, e);
            }
        } else {
            window.set_cursor_grab(CursorGrabMode::None).unwrap();
        }

        window.set_cursor_visible(!grab);
        self.mouse_grabbed = grab;
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

    pub(crate) fn ui_message(&mut self, msg: UiMessage) {
        let mut print = self.cvars.d_ui_msgs;

        match msg.direction {
            MessageDirection::ToWidget if !self.cvars.d_ui_msgs_direction_to => print = false,
            MessageDirection::FromWidget if !self.cvars.d_ui_msgs_direction_from => print = false,
            _ => (),
        }

        match msg.data() {
            Some(
                WidgetMessage::MouseDown { .. }
                | WidgetMessage::MouseUp { .. }
                | WidgetMessage::MouseMove { .. }
                | WidgetMessage::MouseWheel { .. }
                | WidgetMessage::MouseLeave
                | WidgetMessage::MouseEnter
                | WidgetMessage::DoubleClick { .. },
            ) if !self.cvars.d_ui_msgs_mouse => print = false,
            _ => (),
        }

        if print {
            // LATER dbg_logdp for pretty printing
            dbg!(&msg);
        }

        if self.console.is_open() {
            self.console.ui_message(&mut self.engine, &mut self.cvars, msg);
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
        // LATER How to handle time in logging code? Real or frame time?
        // Should be OK to create one instant as 0 and clone it to a global/client/server.
        // Elapsed is guaranteed to be monotonic even across instances
        // because it uses Instant::now() internally.
        self.clock.elapsed().as_secs_f32()
    }
}

/// Layout independant scancodes.
///
/// This is a separate mod so you can glob-import it.
#[rustfmt::skip]
// ...and also so I can stop rustfmt from mangling it.
// Seriously, remove #[rustfmt::skip] and see what it does, I dare you.
// I've never seen anybody ever format comments like that
// and rustfmt does it by default without a way to disable it.
// I. Just. Hate. It.
mod scan_codes {
    #![allow(dead_code)]

    use fyrox::event::ScanCode;

    // Apparently there are different numbering schemes all called "scancodes".
    // This image is the least inaccurate for the one in winit (on Kubuntu 22.04):
    // https://forum.thegamecreators.com/thread/145420
    // Note that many keys are different (e.g. R_ALT, KP_ENTER, arrows, ...).

    pub(crate) const ESC: ScanCode = 1;
    pub(crate) const NUM1: ScanCode = 2;
    pub(crate) const NUM2: ScanCode = 3;
    pub(crate) const NUM3: ScanCode = 4;
    pub(crate) const NUM4: ScanCode = 5;
    pub(crate) const NUM5: ScanCode = 6;
    pub(crate) const NUM6: ScanCode = 7;
    pub(crate) const NUM7: ScanCode = 8;
    pub(crate) const NUM8: ScanCode = 9;
    pub(crate) const NUM9: ScanCode = 10;
    pub(crate) const NUM0: ScanCode = 11;
    pub(crate) const MINUS: ScanCode = 12;
    pub(crate) const EQUALS: ScanCode = 13;
    pub(crate) const BACKSPACE: ScanCode = 14;
    pub(crate) const TAB: ScanCode = 15;
    pub(crate) const Q: ScanCode = 16;
    pub(crate) const W: ScanCode = 17;
    pub(crate) const E: ScanCode = 18;
    pub(crate) const R: ScanCode = 19;
    pub(crate) const T: ScanCode = 20;
    pub(crate) const Y: ScanCode = 21;
    pub(crate) const U: ScanCode = 22;
    pub(crate) const I: ScanCode = 23;
    pub(crate) const O: ScanCode = 24;
    pub(crate) const P: ScanCode = 25;
    pub(crate) const LBRACKET: ScanCode = 26;
    pub(crate) const RBRACKET: ScanCode = 27;
    pub(crate) const ENTER: ScanCode = 28;
    pub(crate) const L_CTRL: ScanCode = 29;
    pub(crate) const A: ScanCode = 30;
    pub(crate) const S: ScanCode = 31;
    pub(crate) const D: ScanCode = 32;
    pub(crate) const F: ScanCode = 33;
    pub(crate) const G: ScanCode = 34;
    pub(crate) const H: ScanCode = 35;
    pub(crate) const J: ScanCode = 36;
    pub(crate) const K: ScanCode = 37;
    pub(crate) const L: ScanCode = 38;
    pub(crate) const SEMICOLON: ScanCode = 39;
    pub(crate) const APOSTROPHE: ScanCode = 40;
    pub(crate) const BACKTICK: ScanCode = 41;
    pub(crate) const L_SHIFT: ScanCode = 42;
    pub(crate) const BACKSLASH: ScanCode = 43;
    pub(crate) const Z: ScanCode = 44;
    pub(crate) const X: ScanCode = 45;
    pub(crate) const C: ScanCode = 46;
    pub(crate) const V: ScanCode = 47;
    pub(crate) const B: ScanCode = 48;
    pub(crate) const N: ScanCode = 49;
    pub(crate) const M: ScanCode = 50;
    pub(crate) const COMMA: ScanCode = 51;
    pub(crate) const PERIOD: ScanCode = 52;
    pub(crate) const SLASH: ScanCode = 53;
    pub(crate) const R_SHIFT: ScanCode = 54;
    pub(crate) const KP_MULTIPLY: ScanCode = 55;
    pub(crate) const L_ALT: ScanCode = 56;
    pub(crate) const SPACE: ScanCode = 57;
    pub(crate) const CAPS_LOCK: ScanCode = 58;
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
    pub(crate) const KP7: ScanCode = 71;
    pub(crate) const KP8: ScanCode = 72;
    pub(crate) const KP9: ScanCode = 73;
    pub(crate) const KP_MINUS: ScanCode = 74;
    pub(crate) const KP4: ScanCode = 75;
    pub(crate) const KP5: ScanCode = 76;
    pub(crate) const KP6: ScanCode = 77;
    pub(crate) const KP_PLUS: ScanCode = 78;
    pub(crate) const KP1: ScanCode = 79;
    pub(crate) const KP2: ScanCode = 80;
    pub(crate) const KP3: ScanCode = 81;
    pub(crate) const KP0: ScanCode = 82;
    pub(crate) const KP_PERIOD: ScanCode = 83;
    // 84
    // 85
    pub(crate) const BACKSLASH2: ScanCode = 86; // Between LSHIFT and Z, not on all keyboards
    // 87
    // 88
    // 89
    // 90
    // 91
    // 92
    // 93
    // 94
    // 95
    pub(crate) const KP_ENTER: ScanCode = 96;
    pub(crate) const R_CTRL: ScanCode = 97;
    pub(crate) const KP_DIVIDE: ScanCode = 98;
    pub(crate) const PRINT_SCREEN: ScanCode = 99;
    pub(crate) const R_ALT: ScanCode = 100;
    // 101
    pub(crate) const HOME: ScanCode = 102;
    pub(crate) const UP_ARROW: ScanCode = 103;
    pub(crate) const PG_UP: ScanCode = 104;
    pub(crate) const LEFT_ARROW: ScanCode = 105;
    pub(crate) const RIGHT_ARROW: ScanCode = 106;
    pub(crate) const END: ScanCode = 107;
    pub(crate) const DOWN_ARROW: ScanCode = 108;
    pub(crate) const PG_DOWN: ScanCode = 109;
    pub(crate) const INSERT: ScanCode = 110;
    pub(crate) const DELETE: ScanCode = 111;
    // 112
    // 113
    // 114
    // 115
    // 116
    // 117
    // 118
    pub(crate) const PAUSE: ScanCode = 119;
    // 120
    // 121
    // 122
    // 123
    // 124
    pub(crate) const L_SUPER: ScanCode = 125;
    pub(crate) const R_SUPER: ScanCode = 126;
    pub(crate) const MENU: ScanCode = 127;
}
