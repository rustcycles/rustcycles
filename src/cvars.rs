//! Console variables - configuration options for anything and everything.

use cvars::SetGet;
use cvars_console::CvarAccess;

/// Console variables - configuration options for anything and everything.
///
/// Prefix meanings:
/// cl_ is client
/// d_ is debug
/// g_ is gameplay
/// hud_ is the heads-up display
/// r_ is rendering
/// sv_ is server administration + performance
// Normally we use pub(crate) everywhere for when the project is eventually
// split into crates but here we have to use pub:
// https://github.com/martin-t/cvars/issues/13
// Plus then cvars will likely be pub anyway.
#[derive(Debug, Clone, SetGet)]
pub struct Cvars {
    // Long-term this needs some kind of better system to reduce duplication / manual work.
    // Would be nice to keep alphabetically.
    //  |
    //  v
    pub cl_fullscreen: bool,
    pub cl_mouse_grab_on_focus: bool,
    pub cl_window_height: i32,
    pub cl_window_width: i32,

    /// "Temporary" cvar for quick testing. Normally unused but kept here
    /// so I don't have to add a cvar each time I want a quick toggle.
    pub d_dbg: bool,

    // TODO A lot of these cvars need to be synced to server when playing locally.
    /// Master switch for debug output - the d_draw_* group.
    pub d_draw: bool,
    pub d_draw_frame_timings: bool,
    pub d_draw_frame_timings_steps: usize,
    pub d_draw_frame_timings_text: bool,
    /// This ruins perf in debug builds: https://github.com/FyroxEngine/Fyrox/issues/237
    pub d_draw_physics: bool,
    pub d_draw_text: bool,
    pub d_draw_text_shadow: bool,
    pub d_draw_text_shadow_dilation: f32,
    pub d_draw_text_shadow_offset_x: f32,
    pub d_draw_text_shadow_offset_y: f32,

    pub d_events: bool,
    pub d_events_focused: bool,
    pub d_events_keyboard_input: bool,
    pub d_events_mouse_input: bool,
    pub d_events_mouse_motion: bool,
    pub d_events_resized: bool,

    pub d_panic_unknown_cvar: bool,

    /// Print UI messages or a subset of them.
    pub d_ui_msgs: bool,
    pub d_ui_msgs_direction_from: bool,
    pub d_ui_msgs_direction_to: bool,
    pub d_ui_msgs_mouse: bool,

    pub r_quality: i32,
}

impl Default for Cvars {
    fn default() -> Self {
        Self {
            cl_fullscreen: true,
            cl_mouse_grab_on_focus: true,
            cl_window_height: 540,
            cl_window_width: 960,

            d_dbg: false,

            d_draw: true,
            d_draw_frame_timings: true,
            d_draw_frame_timings_steps: 4,
            d_draw_frame_timings_text: false,
            d_draw_physics: true,
            d_draw_text: true,
            d_draw_text_shadow: true,
            d_draw_text_shadow_dilation: 0.0,
            d_draw_text_shadow_offset_x: 1.0,
            d_draw_text_shadow_offset_y: 1.0,

            d_events: true,
            d_events_focused: false,
            d_events_keyboard_input: false,
            d_events_mouse_input: false,
            d_events_mouse_motion: false,
            d_events_resized: false,

            d_panic_unknown_cvar: true,

            d_ui_msgs: false,
            d_ui_msgs_direction_from: true,
            d_ui_msgs_direction_to: false,
            d_ui_msgs_mouse: false,

            r_quality: 0,
        }
    }
}

impl CvarAccess for Cvars {
    fn get_string(&self, cvar_name: &str) -> Result<String, String> {
        self.get_string(cvar_name)
    }

    fn set_str(&mut self, cvar_name: &str, cvar_value: &str) -> Result<(), String> {
        self.set_str(cvar_name, cvar_value)
    }
}
