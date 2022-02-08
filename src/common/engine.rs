//! These functions replace fyrox's `Engine::update()`
//! but are more granular.

use fyrox::{
    core::{algebra::Vector2, instant},
    resource::texture::TextureKind,
};

use crate::prelude::*;

pub(crate) fn update_resources(engine: &mut Engine, dt: f32) {
    engine.resource_manager.state().update(dt);
    engine.renderer.update_caches(dt);
    engine.handle_model_events();
}

pub(crate) fn update_physics(engine: &mut Engine, dt: f32) {
    let inner_size = engine.get_window().inner_size();
    let window_size = Vector2::new(inner_size.width as f32, inner_size.height as f32);

    for scene in engine.scenes.iter_mut().filter(|s| s.enabled) {
        let frame_size = scene.render_target.as_ref().map_or(window_size, |rt| {
            if let TextureKind::Rectangle { width, height } = rt.data_ref().kind() {
                Vector2::new(width as f32, height as f32)
            } else {
                panic!("only rectangle textures can be used as render target!");
            }
        });

        scene.update(frame_size, dt);
    }
}

pub(crate) fn update_ui(engine: &mut Engine, dt: f32) {
    let inner_size = engine.get_window().inner_size();
    let window_size = Vector2::new(inner_size.width as f32, inner_size.height as f32);

    let time = instant::Instant::now();
    engine.user_interface.update(window_size, dt);
    engine.ui_time = instant::Instant::now() - time;
}
