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

use cvars_console_fyrox::FyroxConsole;
use fyrox::{
    core::instant::Instant,
    dpi::PhysicalSize,
    engine::GraphicsContext,
    event::{ElementState, KeyboardInput, MouseButton, MouseScrollDelta, TouchPhase},
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
    client::game::{ClientFrameData, ClientGame},
    common::net::{LocalConnection, LocalListener, TcpConnection},
    prelude::*,
    server::game::ServerGame,
};

/// The process that runs a player's game client.
pub struct ClientProcess {
    cvars: Cvars,
    clock: Instant,
    mouse_grabbed: bool,
    shift_pressed: bool,
    pub engine: Engine,
    r_quality: i32,
    console: FyroxConsole,
    debug_text: Handle<UiNode>,
    gs: GameState,
    cg: ClientGame,
    /// Optional server-side game data when playing in local mode (with shared or LATER separate game state).
    sg: Option<ServerGame>,
    // LATER Optional server-side game state when playing in local mode with separate game states.
    //  This will likely also require separate scenes.
    //sgs: Option<GameState>,
    pub exit: bool,
}

impl ClientProcess {
    pub async fn new(cvars: Cvars, mut engine: Engine, local_game: bool) -> Self {
        let clock = Instant::now();

        let debug_text =
            TextBuilder::new(WidgetBuilder::new().with_foreground(Brush::Solid(Color::RED)))
                // LATER react to changes at runtime
                .with_shadow(cvars.d_draw_text_shadow)
                .with_shadow_dilation(cvars.d_draw_text_shadow_dilation)
                .with_shadow_offset(Vector2::new(
                    cvars.d_draw_text_shadow_offset_x,
                    cvars.d_draw_text_shadow_offset_y,
                ))
                // Word wrap doesn't work if there's an extremely long word.
                .with_wrap(WrapMode::Letter)
                .build(&mut engine.user_interface.build_ctx());

        // Z index doesn't work, console has to be created after debug_text (and any other UI):
        // https://github.com/FyroxEngine/Fyrox/issues/356
        let console = FyroxConsole::new(&mut engine.user_interface);

        let gs_type = if local_game {
            GameStateType::Shared
        } else {
            GameStateType::Client
        };
        let mut gs = GameState::new(&cvars, &mut engine, gs_type).await;

        let (sg, cg) = if local_game {
            // LATER Multithreading would be sweet but we can't use threads in WASM.

            let (tx1, rx1) = mpsc::channel();
            let (tx2, rx2) = mpsc::channel();
            let conn1 = LocalConnection::new(tx1, rx2);
            let conn2 = LocalConnection::new(tx2, rx1);

            // Init server first, otherwise the client has nothing to connect to.
            let listener = LocalListener::new(conn1);
            let mut sg = ServerGame::new(Box::new(listener)).await;

            // Make the server accept the local connection
            // and send init data into it so the client can read it during creation.
            // Otherwise the client would remain stuck.
            // Yes, this is really ugly.
            let mut data = ServerFrameData {
                cvars: &cvars,
                scene: &mut engine.scenes[gs.scene_handle],
                gs: &mut gs,
                sg: &mut sg,
            };
            data.accept_new_connections();

            let cg =
                ClientGame::new(&cvars, &mut engine, debug_text, Box::new(conn2), &mut gs).await;

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
            let cg =
                ClientGame::new(&cvars, &mut engine, debug_text, Box::new(conn), &mut gs).await;

            (None, cg)
        };

        let exit = cvars.d_exit_after_one_frame;

        let elapsed = clock.elapsed();
        dbg_logf!("ClientProcess::new() took {} ms", elapsed.as_millis());

        Self {
            cvars,
            clock,
            mouse_grabbed: false,
            shift_pressed: false,
            engine,
            r_quality: -1, // Initialize this on the first frame, after graphics_context
            console,
            debug_text,
            gs,
            cg,
            sg,
            exit,
        }
    }

    pub fn resized(&mut self, size: PhysicalSize<u32>) {
        // This is also called when the window is first created.

        if self.cvars.d_events && self.cvars.d_events_resized {
            dbg_logf!("{} resized: {:?}", self.real_time(), size);
        }

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

        self.console.resized(
            &mut self.engine.user_interface,
            size.width as f32,
            size.height as f32,
        );
    }

    pub fn focused(&mut self, focus: bool) {
        if self.cvars.d_events && self.cvars.d_events_focused {
            dbg_logf!("{} focused: {:?}", self.real_time(), focus);
        }

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

    pub fn keyboard_input(&mut self, input: KeyboardInput) {
        // NOTE: This event is repeated if the key is held, that means
        // there can be more `state: Pressed` events before a `state: Released`.

        if self.cvars.d_events && self.cvars.d_events_keyboard_input {
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
                    self.close_console();
                } else if self.shift_pressed {
                    // Shift + ESC is a common shortcut to open the console in games.
                    // This shortcut should not be configurable so it works for all players
                    // no matter how much they break their config.
                    self.open_console();
                } else {
                    // ESC anywhere else just ungrabs the mouse.
                    self.set_mouse_grab(false);
                }
            }
            BACKTICK if pressed => {
                // LATER Configurable console bind.
                if !self.console.is_open() {
                    self.open_console();
                }
            }
            L_SHIFT => self.shift_pressed = pressed,
            _ => (),
        }
    }

    fn open_console(&mut self) {
        self.console.open(&mut self.engine.user_interface, self.mouse_grabbed);
        self.cg.input.release_all_keys();
        self.set_mouse_grab(false);
    }

    fn close_console(&mut self) {
        let grab = self.console.close(&mut self.engine.user_interface);
        self.set_mouse_grab(grab);
    }

    /// Input that is handdled only when we're in game.
    fn game_input(&mut self, input: KeyboardInput) {
        use scan_codes::*;

        let pressed = input.state == ElementState::Pressed;

        match input.scancode {
            W => self.cg.input.forward = pressed,
            A => self.cg.input.left = pressed,
            S => self.cg.input.backward = pressed,
            D => self.cg.input.right = pressed,
            SPACE => self.cg.input.up = pressed,
            L_SHIFT => self.cg.input.down = pressed,
            Q => self.cg.input.prev_weapon = pressed,
            E => self.cg.input.next_weapon = pressed,
            R => self.cg.input.reload = pressed,
            F => self.cg.input.flag = pressed,
            G => self.cg.input.grenade = pressed,
            K => self.cg.input.kill = pressed,
            M => self.cg.input.map = pressed,
            TAB => self.cg.input.score = pressed,
            ENTER => self.cg.input.chat = pressed,
            PAUSE => self.cg.input.pause = pressed,
            F12 => self.cg.input.screenshot = pressed,
            _ => (),
        }

        self.cg.input.real_time = self.real_time();
        self.cg.input.game_time = self.gs.game_time;
        self.cg.send_input();
    }

    pub fn mouse_wheel(&self, delta: MouseScrollDelta, phase: TouchPhase) {
        if self.cvars.d_events && self.cvars.d_events_mouse_wheel {
            dbg_logf!("{} mouse wheel {:?} {:?}", self.real_time(), delta, phase);
        }

        // LATER After figuring out input: prev/next weap on mouse wheel.
        //       Currently there is no way to do this.
        // if let MouseScrollDelta::LineDelta(_, y) = delta {
        //     // Scrolling seems to always produce only 1 or -1.
        //     if y == 1.0 {
        //         // self.cg.input.next_weapon
        //     } else if y == -1.0 {
        //         // self.cg.input.prev_weapon
        //     }
        // }
    }

    pub fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        if self.cvars.d_events && self.cvars.d_events_mouse_input {
            dbg_logf!("{} mouse_input: {:?} {:?}", self.real_time(), state, button);
        }

        if !self.console.is_open() {
            self.set_mouse_grab(true);

            let pressed = state == ElementState::Pressed;
            match button {
                MouseButton::Left => self.cg.input.fire1 = pressed,
                MouseButton::Right => self.cg.input.fire2 = pressed,
                MouseButton::Middle => self.cg.input.zoom = pressed,
                MouseButton::Other(8) => self.cg.input.marker1 = pressed,
                MouseButton::Other(9) => self.cg.input.marker2 = pressed,
                MouseButton::Other(_) => {}
            }

            self.cg.input.real_time = self.real_time();
            self.cg.input.game_time = self.gs.game_time;
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

        let window = match &self.engine.graphics_context {
            GraphicsContext::Initialized(ctx) => &ctx.window,
            _ => soft_unreachable!(),
        };
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

    pub fn mouse_motion(&mut self, delta: (f64, f64)) {
        if self.cvars.d_events && self.cvars.d_events_mouse_motion {
            // LATER This event normally happens every 4 ms for me when moving the mouse. Print stats.
            // Is it limited by my polling rate? Would it be helpful to teach players how to increase it?
            // Sometimes i get a batch of 4 events every 16 ms. Detect this.
            // https://github.com/martin-t/rustcycles/issues/1
            //
            // Might be because the main thread is blocked running game logic.
            // Update this comment after separating things to threads.

            // LATER This doesn't have enough precision, and neither do the other events.
            // the smallest delta is a whole pixel.
            dbg_logf!("{} mouse_motion: {:?}", self.real_time(), delta);
        }

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

        let zoom_factor = if self.cg.input.zoom {
            self.cvars.cl_zoom_factor
        } else {
            1.0
        };

        let sens_h = self.cvars.m_sensitivity * self.cvars.m_sensitivity_horizontal;
        let sens_v = self.cvars.m_sensitivity * self.cvars.m_sensitivity_vertical;
        // Subtract, don't add the delta - nalgebra rotations are counterclockwise.
        let delta_yaw = -delta.0 as f32 * sens_h / zoom_factor;
        let delta_pitch = delta.1 as f32 * sens_v / zoom_factor;

        self.cg.delta_yaw += delta_yaw;
        self.cg.delta_pitch += delta_pitch;
    }

    pub fn ui_message(&mut self, msg: &UiMessage) {
        self.ui_message_logging(msg);

        self.console.ui_message(&mut self.engine.user_interface, &mut self.cvars, msg);
    }

    fn ui_message_logging(&mut self, msg: &UiMessage) {
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
    }

    pub fn update(&mut self) {
        // LATER read these (again), verify what works best in practise:
        // https://gafferongames.com/post/fix_your_timestep/
        // https://medium.com/@tglaiel/how-to-make-your-game-run-at-60fps-24c61210fe75

        let game_time_target = self.real_time();

        // LATER Abstract game loop logic and merge with server
        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            self.gs.frame_number += 1;

            // LATER Check order of cl and sv stuff for minimum latency.
            // LATER change endpoint name for parts to locl/losv?

            self.cfd().tick_begin_frame();
            self.sfd().map(|mut sfd| sfd.tick_begin_frame());

            self.fd().tick_before_physics(dt);

            self.cfd().tick_before_physics(dt);

            // Update animations, transformations, physics, ...
            // Dummy control flow and lag since we don't use fyrox plugins.
            let mut cf = fyrox::event_loop::ControlFlow::Poll;
            let mut lag = 0.0;
            self.engine.pre_update(dt, &mut cf, &mut lag, FxHashMap::default());
            // Sanity check - if the engine starts doing something with these, we'll know.
            assert_eq!(cf, fyrox::event_loop::ControlFlow::Poll);
            assert_eq!(lag, 0.0);

            // `tick_after_physics` tells the engine to draw debug shapes and text.
            // Any debug calls after it will show up next frame.
            self.fd().debug_engine_updates(v!(-5 3 3));
            self.cfd().tick_after_physics(dt);
            self.fd().debug_engine_updates(v!(-6 3 3));

            // `sys_send_update` sends debug shapes and text to client.
            // Any debug calls after it will show up next frame.
            self.fd().debug_engine_updates(v!(-5 5 3));
            self.sfd().map(|mut sfd| sfd.sys_send_update());
            self.fd().debug_engine_updates(v!(-6 5 3));

            // Update UI
            self.engine.post_update(dt);
        }

        self.update_graphics();
    }

    fn update_graphics(&mut self) {
        let ctx = match &mut self.engine.graphics_context {
            GraphicsContext::Initialized(ctx) => ctx,
            _ => return,
        };

        if self.cvars.r_quality != self.r_quality {
            self.r_quality = self.cvars.r_quality;

            let quality = match self.cvars.r_quality {
                0 => QualitySettings::low(),
                1 => QualitySettings::medium(),
                2 => QualitySettings::high(),
                _ => {
                    dbg_logf!("Invalid r_quality value: {}", self.cvars.r_quality);
                    QualitySettings::low()
                }
            };
            ctx.renderer.set_quality_settings(&quality).unwrap();
        }

        ctx.window.request_redraw();
    }

    fn sfd(&mut self) -> Option<ServerFrameData> {
        self.sg.as_mut().map(|sg| ServerFrameData {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
            sg,
        })
    }

    fn cfd(&mut self) -> ClientFrameData {
        let renderer = match &mut self.engine.graphics_context {
            GraphicsContext::Initialized(ctx) => Some(&mut ctx.renderer),
            _ => None,
        };

        ClientFrameData {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
            cg: &mut self.cg,
            renderer,
            ui: &mut self.engine.user_interface,
        }
    }

    fn fd(&mut self) -> FrameData {
        FrameData {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
        }
    }

    pub fn loop_destroyed(&self) {
        dbg_logf!("{} bye", self.real_time());
    }

    pub fn real_time(&self) -> f32 {
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
///
/// We use scancodes, not virtual key codes, because they don't depend on layout.
/// This is problematic in other ways:
/// - They might not be consistent across platforms and vendors.
/// - We currently have no way to map scan codes to a layout _dependant_ key names
///   for displaying in the UI.
///
/// Here's a bunch of issues to follow:
/// - https://github.com/rust-windowing/winit/issues/732:
///   - Improving Scan Code and General Keyboard Layout Handling
///   - From my testing on Windows and Linux the letter/punctuation keys, number keys,
///     function keys, and escape, space, shift, enter, tab, backspace, and caps keys
///     are all identical across both platforms, but various other keys
///     (such as arrow keys) weren't consistent.
/// - https://github.com/rust-windowing/winit/issues/2436:
///   - Expose ScanCode to VirtualKeyCode mapping
/// - https://github.com/rust-windowing/winit/issues/1806
///   - Meta Issue: Keyboard input
/// - https://github.com/bevyengine/bevy/discussions/2386:
///   - Naming problems: KeyboardInput Scancodes/Virtual Scancodes
#[rustfmt::skip]
// ...and also so I can stop rustfmt from mangling it.
// Seriously, remove #[rustfmt::skip] and see what it does, I dare you.
// I've never seen anybody *ever* format comments like that
// and rustfmt does it *by default* without a way to disable it.
// I. Just. Hate. It.
mod scan_codes {
    #![allow(dead_code)]

    use fyrox::event::ScanCode;

    // Apparently there are different numbering schemes all called "scancodes".
    // This image is the least inaccurate for the one in winit (on Kubuntu 22.04 if that matters):
    // https://forum.thegamecreators.com/thread/145420
    // Note that many keys are different (e.g. R_ALT, KP_ENTER, arrows, ...),
    // this is just to get a vague idea how it looks.

    pub const ESC: ScanCode = 1;
    pub const NUM1: ScanCode = 2;
    pub const NUM2: ScanCode = 3;
    pub const NUM3: ScanCode = 4;
    pub const NUM4: ScanCode = 5;
    pub const NUM5: ScanCode = 6;
    pub const NUM6: ScanCode = 7;
    pub const NUM7: ScanCode = 8;
    pub const NUM8: ScanCode = 9;
    pub const NUM9: ScanCode = 10;
    pub const NUM0: ScanCode = 11;
    pub const MINUS: ScanCode = 12;
    pub const EQUALS: ScanCode = 13;
    pub const BACKSPACE: ScanCode = 14;
    pub const TAB: ScanCode = 15;
    pub const Q: ScanCode = 16;
    pub const W: ScanCode = 17;
    pub const E: ScanCode = 18;
    pub const R: ScanCode = 19;
    pub const T: ScanCode = 20;
    pub const Y: ScanCode = 21;
    pub const U: ScanCode = 22;
    pub const I: ScanCode = 23;
    pub const O: ScanCode = 24;
    pub const P: ScanCode = 25;
    pub const LBRACKET: ScanCode = 26;
    pub const RBRACKET: ScanCode = 27;
    pub const ENTER: ScanCode = 28;
    pub const L_CTRL: ScanCode = 29;
    pub const A: ScanCode = 30;
    pub const S: ScanCode = 31;
    pub const D: ScanCode = 32;
    pub const F: ScanCode = 33;
    pub const G: ScanCode = 34;
    pub const H: ScanCode = 35;
    pub const J: ScanCode = 36;
    pub const K: ScanCode = 37;
    pub const L: ScanCode = 38;
    pub const SEMICOLON: ScanCode = 39;
    pub const APOSTROPHE: ScanCode = 40;
    pub const BACKTICK: ScanCode = 41;
    pub const L_SHIFT: ScanCode = 42;
    pub const BACKSLASH: ScanCode = 43;
    pub const Z: ScanCode = 44;
    pub const X: ScanCode = 45;
    pub const C: ScanCode = 46;
    pub const V: ScanCode = 47;
    pub const B: ScanCode = 48;
    pub const N: ScanCode = 49;
    pub const M: ScanCode = 50;
    pub const COMMA: ScanCode = 51;
    pub const PERIOD: ScanCode = 52;
    pub const SLASH: ScanCode = 53;
    pub const R_SHIFT: ScanCode = 54;
    pub const KP_MULTIPLY: ScanCode = 55;
    pub const L_ALT: ScanCode = 56;
    pub const SPACE: ScanCode = 57;
    pub const CAPS_LOCK: ScanCode = 58;
    pub const F1: ScanCode = 59;
    pub const F2: ScanCode = 60;
    pub const F3: ScanCode = 61;
    pub const F4: ScanCode = 62;
    pub const F5: ScanCode = 63;
    pub const F6: ScanCode = 64;
    pub const F7: ScanCode = 65;
    pub const F8: ScanCode = 66;
    pub const F9: ScanCode = 67;
    pub const F10: ScanCode = 68;
    pub const F11: ScanCode = 69;
    pub const F12: ScanCode = 70;
    pub const KP7: ScanCode = 71;
    pub const KP8: ScanCode = 72;
    pub const KP9: ScanCode = 73;
    pub const KP_MINUS: ScanCode = 74;
    pub const KP4: ScanCode = 75;
    pub const KP5: ScanCode = 76;
    pub const KP6: ScanCode = 77;
    pub const KP_PLUS: ScanCode = 78;
    pub const KP1: ScanCode = 79;
    pub const KP2: ScanCode = 80;
    pub const KP3: ScanCode = 81;
    pub const KP0: ScanCode = 82;
    pub const KP_PERIOD: ScanCode = 83;
    // 84
    // 85
    pub const BACKSLASH2: ScanCode = 86; // Between LSHIFT and Z, not on all keyboards
    // 87
    // 88
    // 89
    // 90
    // 91
    // 92
    // 93
    // 94
    // 95
    pub const KP_ENTER: ScanCode = 96;
    pub const R_CTRL: ScanCode = 97;
    pub const KP_DIVIDE: ScanCode = 98;
    pub const PRINT_SCREEN: ScanCode = 99;
    pub const R_ALT: ScanCode = 100;
    // 101
    pub const HOME: ScanCode = 102;
    pub const UP_ARROW: ScanCode = 103;
    pub const PG_UP: ScanCode = 104;
    pub const LEFT_ARROW: ScanCode = 105;
    pub const RIGHT_ARROW: ScanCode = 106;
    pub const END: ScanCode = 107;
    pub const DOWN_ARROW: ScanCode = 108;
    pub const PG_DOWN: ScanCode = 109;
    pub const INSERT: ScanCode = 110;
    pub const DELETE: ScanCode = 111;
    // 112
    // 113
    // 114
    // 115
    // 116
    // 117
    // 118
    pub const PAUSE: ScanCode = 119;
    // 120
    // 121
    // 122
    // 123
    // 124
    pub const L_SUPER: ScanCode = 125;
    pub const R_SUPER: ScanCode = 126;
    pub const MENU: ScanCode = 127;
}
