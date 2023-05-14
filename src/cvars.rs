//! Console variables - configuration options for anything and everything.

use std::{
    fmt::{self, Display, Formatter},
    num::ParseFloatError,
    str::FromStr,
};

use cvars::SetGet;

use crate::prelude::*;

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
    pub cl_camera_initial_position: CVec3,
    pub cl_camera_speed: f32,
    pub cl_camera_z_near: f32,
    pub cl_camera_z_far: f32,

    pub cl_fullscreen: bool,
    pub cl_headless: bool,
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

    pub d_engine_stats: bool,

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

    pub d_physics_extra_sync: bool,

    /// The seed to initialize the RNG.
    ///
    /// This is not very helpful by itself because by the time you can change cvars in the console,
    /// the seed has already been used. However, in the desktop version, you can set it on the command line.
    ///
    /// LATER If the seed is 0 at match start, the cvar is changed to the current time and that is used as seed.
    /// This means you can look at the cvar's value later and know what seed you need to replay the same game.
    pub d_seed: u64,

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
    /// How far to go back when doing raycasts to avoid entering objects
    /// due to floating point errors.
    ///
    /// Nudging should enabled by default because it's easy to forget
    /// and usually this is what we want for most traces anyway.
    pub g_physics_nudge: f32,

    pub g_projectile_lifetime: f32,
    pub g_projectile_refire: f32,
    pub g_projectile_speed: f32,
    pub g_projectile_spread: f32,

    pub g_wheel_acceleration: f32,

    pub m_pitch_max: f32,
    pub m_pitch_min: f32,

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
            cl_camera_3rd_person_up: 1.0,
            cl_camera_fov: 75.0,
            cl_camera_initial_position: v!(0 5 -15).into(),
            cl_camera_speed: 10.0,
            cl_camera_z_near: 0.001,
            cl_camera_z_far: 2048.0,

            cl_fullscreen: true,
            cl_headless: false,
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

            d_engine_stats: true,

            d_events: true,
            d_events_focused: false,
            d_events_keyboard_input: false,
            d_events_mouse_input: false,
            d_events_mouse_motion: false,
            d_events_mouse_wheel: false,
            d_events_resized: false,

            d_exit_after_one_frame: false,
            d_exit_on_unknown_cvar: true,

            d_physics_extra_sync: false,

            d_seed: 0,

            d_ui_msgs: false,
            d_ui_msgs_direction_from: true,
            d_ui_msgs_direction_to: false,
            d_ui_msgs_mouse: false,

            g_physics_max_ccd_substeps: 100,
            g_physics_nudge: 0.01,

            g_projectile_lifetime: 60.0,
            g_projectile_refire: 0.05,
            g_projectile_speed: 75.0,
            g_projectile_spread: 0.2,

            g_wheel_acceleration: 20.0,

            m_pitch_max: 90.0,
            m_pitch_min: -90.0,

            m_sensitivity: 0.15,
            m_sensitivity_horizontal: 1.0,
            m_sensitivity_vertical: 1.0,

            r_quality: 0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CVec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl CVec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

impl FromStr for CVec3 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_cvec3(s).map_err(|e| {
            if let Some(e) = e {
                format!("Expected format `'x y z'`, got `{}`: {}", s, e)
            } else {
                format!("Expected format `'x y z'`, got `{}`", s)
            }
        })
    }
}

fn parse_cvec3(mut s: &str) -> Result<CVec3, Option<ParseFloatError>> {
    if s.starts_with('\'') && s.ends_with('\'') {
        s = &s[1..s.len() - 1];
    }
    let mut parts = s.split(' ');
    let x = parts.next().ok_or(None)?.parse().map_err(|e| Some(e))?;
    let y = parts.next().ok_or(None)?.parse().map_err(|e| Some(e))?;
    let z = parts.next().ok_or(None)?.parse().map_err(|e| Some(e))?;
    if parts.next().is_some() {
        return Err(None);
    }
    Ok(CVec3::new(x, y, z))
}

impl Display for CVec3 {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "'{:?} {:?} {:?}'", self.x, self.y, self.z)
    }
}

impl From<CVec3> for Vec3 {
    fn from(v: CVec3) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for CVec3 {
    fn from(v: Vec3) -> Self {
        CVec3::new(v.x, v.y, v.z)
    }
}
