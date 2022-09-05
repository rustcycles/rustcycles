//! Console variables - configuration options for anything and everything.

use cvars::SetGet;

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
    pub cl_window_height: i32,
    pub cl_window_width: i32,

    /// "Temporary" cvar for quick testing. Normally unused but kept here
    /// so I don't have to add a cvar each time I want a quick toggle.
    pub d_dbg: bool,

    /// Master switch for debug output - the d_draw_* group.
    pub d_draw: bool,
    pub d_draw_frame_timings: bool,
    /// This ruins perf in debug builds: https://github.com/FyroxEngine/Fyrox/issues/237
    pub d_draw_physics: bool,
    pub d_draw_text: bool,

    pub d_ui_messages: bool,

    pub r_quality: i32,
}

impl Default for Cvars {
    fn default() -> Self {
        Self {
            cl_fullscreen: true,
            cl_window_height: 540,
            cl_window_width: 960,

            d_dbg: false,

            d_draw: true,
            d_draw_frame_timings: true,
            d_draw_physics: true,
            d_draw_text: true,

            d_ui_messages: false,

            r_quality: 0,
        }
    }
}
