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
    pub cl_camera_3rd_person_back: f32,
    pub cl_camera_3rd_person_up: f32,
    /// Vertical field of view in degrees.
    ///
    /// LATER What do other games use? Horiz/vert, what values?
    pub cl_camera_fov: f32,
    pub cl_camera_speed: f32,
    pub cl_camera_z_near: f32,
    pub cl_camera_z_far: f32,

    pub cl_fullscreen: bool,
    pub cl_mouse_grab_on_focus: bool,
    pub cl_window_height: i32,
    pub cl_window_width: i32,

    pub cl_zoom_factor: f32,

    /// A "temporary" cvar for quick testing. Normally unused but kept here
    /// so I don't have to add a cvar each time I want a quick toggle.
    pub d_dbg: bool,
    /// Same as d_dbg but for floats.
    pub d_dbgf: f32,
    /// Same as d_dbg but for ints.
    pub d_dbgi: i32,

    // TODO A lot of these cvars need to be synced to server when playing locally.
    /// Master switch for debug output - the d_draw_* group.
    pub d_draw: bool,
    pub d_draw_arrows: bool,
    pub d_draw_crosses: bool,
    pub d_draw_crosses_half_len: f32,
    pub d_draw_crosses_line_from_origin: bool,
    pub d_draw_frame_timings: bool,
    pub d_draw_frame_timings_steps: usize,
    pub d_draw_frame_timings_text: bool,
    pub d_draw_lines: bool,
    /// This ruins perf in debug builds: https://github.com/FyroxEngine/Fyrox/issues/237
    pub d_draw_physics: bool,
    pub d_draw_rots: bool,
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
    pub d_events_mouse_wheel: bool,
    pub d_events_resized: bool,

    /// Run init, gamelogic, and rendering once, then exit. Useful for crude testing/benchmarking.
    pub d_exit_after_one_frame: bool,
    pub d_exit_on_unknown_cvar: bool,

    /// Print UI messages or a subset of them.
    pub d_ui_msgs: bool,
    pub d_ui_msgs_direction_from: bool,
    pub d_ui_msgs_direction_to: bool,
    pub d_ui_msgs_mouse: bool,

    /// This is needed because the default 1 causes the wheel to randomly stutter/stop
    /// when passing between poles - they use a single trimesh collider.
    /// 2 is very noticeable, 5 is better, 10 is only noticeable at high speeds.
    /// It never completely goes away, even with 100.
    pub g_physics_max_ccd_substeps: u32,
    pub g_physics_nudge: f32,

    pub g_projectile_lifetime: f32,
    pub g_projectile_speed: f32,

    /// Mouse sensitivity.
    pub m_sensitivity: f32,
    /// Additional coefficient for horizontal sensitivity.
    pub m_sensitivity_horizontal: f32,
    /// Additional coefficient for vertical sensitivity.
    pub m_sensitivity_vertical: f32,

    pub r_quality: i32,
}

impl Default for Cvars {
    fn default() -> Self {
        Self {
            cl_camera_3rd_person_back: 2.0,
            cl_camera_3rd_person_up: 0.5,
            cl_camera_fov: 75.0,
            cl_camera_speed: 10.0,
            cl_camera_z_near: 0.001,
            cl_camera_z_far: 2048.0,

            cl_fullscreen: true,
            cl_mouse_grab_on_focus: true,
            cl_window_height: 540,
            cl_window_width: 960,

            cl_zoom_factor: 4.0,

            d_dbg: false,
            d_dbgf: 0.0,
            d_dbgi: 0,

            d_draw: true,
            d_draw_arrows: true,
            d_draw_crosses: true,
            d_draw_crosses_half_len: 0.5,
            d_draw_crosses_line_from_origin: false,
            d_draw_frame_timings: true,
            d_draw_frame_timings_steps: 4,
            d_draw_frame_timings_text: false,
            d_draw_lines: true,
            d_draw_physics: true,
            d_draw_rots: true,
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
            d_events_mouse_wheel: false,
            d_events_resized: false,

            d_exit_after_one_frame: false,
            d_exit_on_unknown_cvar: true,

            d_ui_msgs: false,
            d_ui_msgs_direction_from: true,
            d_ui_msgs_direction_to: false,
            d_ui_msgs_mouse: false,

            g_physics_max_ccd_substeps: 100,
            g_physics_nudge: 0.01,

            g_projectile_lifetime: 60.0,
            g_projectile_speed: 50.0,

            m_sensitivity: 0.5,
            m_sensitivity_horizontal: 1.0,
            m_sensitivity_vertical: 1.0,

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
