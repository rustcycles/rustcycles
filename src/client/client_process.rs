// FIXME: The idea: have Clientwindow or ClientProcess with field GameClient.
// Abstract main loop into a trait.

use fyrox::{
    dpi::PhysicalSize,
    error::ExternalError,
    event::{ElementState, KeyboardInput, MouseButton, ScanCode},
    gui::{message::MessageDirection, widget::WidgetMessage},
};

use crate::client::GameClient;

impl GameClient {
    pub(crate) fn resized(&mut self, size: PhysicalSize<u32>) {
        // This is also called when the window is first created.

        self.engine.set_frame_size(size.into()).unwrap();

        // mrDIMAS on discord:
        // The root element of the UI is Canvas,
        // it has infinite constraints so it does not stretch its contents.
        // If you'll have some complex UI, I'd advise you to create either
        // a window-sized Border or Grid and attach all your ui elements to it,
        // instead of root canvas.
        self.engine
            .user_interface
            .send_message(WidgetMessage::width(
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
            W => self.lp.input.forward = pressed,
            A => self.lp.input.left = pressed,
            S => self.lp.input.backward = pressed,
            D => self.lp.input.right = pressed,
            TAB | SHIFT | CTRL | ALT | BACKSLASH | Z => {
                // Don't print anything, it just spams stdout when switching windows.
            }
            c => {
                if pressed {
                    dbg_logf!("pressed unhandled scancode: {}", c);
                }
            }
        }

        self.send_input();
    }

    pub(crate) fn mouse_input(&mut self, state: ElementState, button: MouseButton) {
        self.set_mouse_grab(true);

        let pressed = state == ElementState::Pressed;
        match button {
            fyrox::event::MouseButton::Left => self.lp.input.fire1 = pressed,
            fyrox::event::MouseButton::Right => self.lp.input.fire2 = pressed,
            fyrox::event::MouseButton::Middle => self.lp.input.zoom = pressed,
            fyrox::event::MouseButton::Other(_) => {}
        }

        self.send_input();
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
        let zoom_factor = if self.lp.input.zoom { 0.25 } else { 1.0 };
        let delta_yaw = delta.0 as f32 * mouse_sensitivity_horizontal * zoom_factor;
        let delta_pitch = delta.1 as f32 * mouse_sensitivity_vertical * zoom_factor;

        // Subtract, don't add the delta X.
        // Nalgebra rotations follow the right hand rule,
        // thumb points in +Z, the curl of fingers shows direction.
        self.lp.input.yaw.0 -= delta_yaw; // LATER Normalize to [0, 360°) or something
        self.lp.input.pitch.0 = (self.lp.input.pitch.0 + delta_pitch).clamp(-90.0, 90.0);
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
}
