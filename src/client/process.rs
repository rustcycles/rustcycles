//! The process that runs a player's game client.
//!
//! Handles stuff like windowing, input, etc. but not game logic.
//! When connected to a remote server, contains a game client.
//! When playing locally, contains both a client and a server.

use std::sync::mpsc;

use cvars_console_fyrox::FyroxConsole;
use fyrox::{
    core::instant::Instant,
    dpi::PhysicalSize,
    engine::GraphicsContext,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase},
    gui::{
        brush::Brush,
        formatted_text::WrapMode,
        message::{MessageDirection, UiMessage},
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        UiNode,
    },
    keyboard::KeyCode,
    renderer::QualitySettings,
    window::CursorGrabMode,
};

use crate::{
    client::game::{ClientFrameData, ClientGame},
    common::net::{self, LocalConnection, LocalListener},
    debug,
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
            let conn = net::tcp_connect(&cvars, "127.0.0.1:26000");
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

    pub fn keyboard_input(&mut self, event: &KeyEvent) {
        // NOTE: This event is repeated if the key is held, that means
        // there can be more `state: Pressed` events before a `state: Released`.

        if self.cvars.d_events && self.cvars.d_events_keyboard_input {
            dbg_logf!("{} keyboard_input: {:?}", self.real_time(), event);
        }

        self.client_input(event);
        if !self.console.is_open() {
            self.game_input(event);
        }
    }

    /// Input that is handled regardless of whether we're in menu/console/game.
    fn client_input(&mut self, event: &KeyEvent) {
        use KeyCode::*;

        let pressed = event.state == ElementState::Pressed;

        match event.physical_key {
            Escape if pressed => {
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
            Backquote if pressed => {
                // LATER Configurable console bind.
                if !self.console.is_open() {
                    self.open_console();
                }
            }
            ShiftLeft => self.shift_pressed = pressed,
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
    fn game_input(&mut self, event: &KeyEvent) {
        use KeyCode::*;

        let pressed = event.state == ElementState::Pressed;

        match event.physical_key {
            KeyW => self.cg.input.forward = pressed,
            KeyA => self.cg.input.left = pressed,
            KeyS => self.cg.input.backward = pressed,
            KeyD => self.cg.input.right = pressed,
            Space => self.cg.input.up = pressed,
            ShiftLeft => self.cg.input.down = pressed,
            KeyQ => self.cg.input.prev_weapon = pressed,
            KeyE => self.cg.input.next_weapon = pressed,
            KeyR => self.cg.input.reload = pressed,
            KeyF => self.cg.input.flag = pressed,
            KeyG => self.cg.input.grenade = pressed,
            KeyK => self.cg.input.kill = pressed,
            KeyM => self.cg.input.map = pressed,
            Tab => self.cg.input.score = pressed,
            Enter => self.cg.input.chat = pressed,
            Pause => self.cg.input.pause = pressed,
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
                MouseButton::Back => self.cg.input.marker2 = pressed,
                MouseButton::Forward => self.cg.input.marker1 = pressed,
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

        let dt_update = game_time_target - self.gs.game_time;
        if dt_update > 5.0 {
            dbg_logf!("large dt_update: {dt_update}");
        }

        // LATER Abstract game loop logic and merge with server?
        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            self.gs.frame_number += 1;
            debug::set_game_time(self.gs.game_time);

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
