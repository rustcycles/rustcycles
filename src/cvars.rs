//! Console variables - configuration options for anything and everything.

use std::{
    fmt::{self, Display, Formatter},
    num::ParseFloatError,
    str::FromStr,
};

use cvars::cvars;

use crate::prelude::*;

// Normally we use pub everywhere for when the project is eventually
// split into crates but here we have to use pub:
// https://github.com/martin-t/cvars/issues/13
// Plus then cvars will likely be pub anyway.
cvars! {
    //! Console variables - configuration options for anything and everything.
    //!
    //! Prefix meanings:
    //! cl_ is client
    //! d_ is debug
    //! g_ is gameplay
    //! hud_ is the heads-up display
    //! r_ is rendering
    //! sv_ is server administration + performance

    // Would be nice to keep alphabetically.

    // LATER move back depending on speed? change fov too?
    cl_camera_3rd_person_back: f32 = 2.0,
    cl_camera_3rd_person_up: f32 = 1.0,
    /// Vertical field of view in degrees.
    ///
    /// LATER What do other games use? Horiz/vert, what values?
    cl_camera_fov: f32 = 75.0,
    cl_camera_initial_position: CVec3 = v!(0 5 -15).into(),
    cl_camera_speed: f32 = 10.0,
    cl_camera_z_near: f32 = 0.001,
    cl_camera_z_far: f32 = 2048.0,

    cl_fullscreen: bool = true,
    /// Run the game without a window. Useful for CI.
    cl_headless: bool = false,
    cl_mouse_grab_on_focus: bool = true,
    cl_vsync: bool = true,
    cl_window_height: i32 = 540,
    cl_window_width: i32 = 960,

    cl_zoom_factor: f32 = 4.0,

    /// A "temporary" cvar for quick testing. Normally unused but kept here
    /// so I don't have to add a cvar each time I want a quick toggle.
    d_dbg: bool = false,
    /// Same as d_dbg but for floats.
    d_dbgf: f32 = 0.0,
    /// Same as d_dbg but for ints.
    d_dbgi: i32 = 0,


    // TODO A lot of these cvars need to be synced to server when playing locally.
    /// Master switch for debug output - the d_draw_* group.
    d_draw: bool = true,
    d_draw_arrows: bool = true,
    d_draw_crosses: bool = true,
    d_draw_crosses_half_len: f32 = 0.5,
    /// Sometimes useful if you have trouble finding the crosses.
    d_draw_crosses_line_from_origin: bool = false,
    d_draw_frame_timings: bool = true,
    d_draw_frame_timings_steps: usize = 4,
    d_draw_frame_timings_text: bool = false,
    d_draw_lines: bool = true,
    /// This ruins perf in debug builds: https://github.com/FyroxEngine/Fyrox/issues/237
    d_draw_physics: bool = true,
    d_draw_rots: bool = true,
    d_draw_rots_size: f32 = 1.0,
    d_draw_text: bool = true,
    d_draw_text_shadow: bool = true,
    d_draw_text_shadow_dilation: f32 = 0.0,
    d_draw_text_shadow_offset_x: f32 = 1.0,
    d_draw_text_shadow_offset_y: f32 = 1.0,

    d_engine_stats: bool = true,

    d_events: bool = true,
    d_events_focused: bool = false,
    d_events_keyboard_input: bool = false,
    d_events_mouse_input: bool = false,
    d_events_mouse_motion: bool = false,
    d_events_mouse_wheel: bool = false,
    d_events_resized: bool = false,

    /// Run init, gamelogic, and rendering once, then exit. Useful for crude testing/benchmarking.
    d_exit_after_one_frame: bool = false,
    /// During init. Set this first.
    d_exit_on_unknown_cvar: bool = true,

    d_physics_extra_sync: bool = false,

    /// The seed to initialize the RNG.
    ///
    /// This is not very helpful by itself because by the time you can change cvars in the console,
    /// the seed has already been used. However, in the desktop version, you can set it on the command line.
    ///
    /// LATER If the seed is 0 at match start, the cvar is changed to the current time and that is used as seed.
    /// This means you can look at the cvar's value later and know what seed you need to replay the same game.
    d_seed: u64 = 0,

    /// Enable extra logging useful when testing the game, for example on CI.
    d_testing: bool = false,

    /// Print UI messages or a subset of them.
    d_ui_msgs: bool = false,
    d_ui_msgs_direction_from: bool = true,
    d_ui_msgs_direction_to: bool = false,
    d_ui_msgs_mouse: bool = false,

    /// This is needed because the default 1 causes the wheel to randomly stutter/stop
    /// when passing between poles - they use a single trimesh collider.
    /// 2 is very noticeable, 5 is better, 10 is only noticeable at high speeds.
    /// It never completely goes away, even with 100.
    g_physics_max_ccd_substeps: u32 = 100,
    /// How far to go back when doing raycasts to avoid entering objects
    /// due to floating point errors.
    ///
    /// Nudging should enabled by default because it's easy to forget
    /// and usually this is what we want for most traces anyway.
    g_physics_nudge: f32 = 0.01,

    /// If fewer human players are connected, bots will join.
    g_players_min: u32 = 4, // TODO

    g_projectile_lifetime: f32 = 60.0,
    g_projectile_refire: f32 = 0.05,
    g_projectile_speed: f32 = 75.0,
    g_projectile_spread: f32 = 0.2,

    g_wheel_acceleration: f32 = 20.0,

    m_pitch_max: f32 = 90.0,
    m_pitch_min: f32 = -90.0,

    /// Mouse sensitivity.
    m_sensitivity: f32 = 0.15,
    /// Additional coefficient for horizontal sensitivity.
    m_sensitivity_horizontal: f32 = 1.0,
    /// Additional coefficient for vertical sensitivity.
    m_sensitivity_vertical: f32 = 1.0,

    net_tcp_connect_retry_delay_ms: u64 = 10,
    net_tcp_connect_retry_print_every_n: u64 = 100,

    r_quality: i32 = 0,

    /// Run the dedicated server without a window.
    ///
    /// Currently off by default because it seems to cause weird stuttering.
    sv_headless: bool = false,
}

/// Vec3 with support for cvars. Should be converted to Vec3 before use in gamecode.
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
    let x = parts.next().ok_or(None)?.parse().map_err(Some)?;
    let y = parts.next().ok_or(None)?.parse().map_err(Some)?;
    let z = parts.next().ok_or(None)?.parse().map_err(Some)?;
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
