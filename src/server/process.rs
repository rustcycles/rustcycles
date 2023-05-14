//! The process that runs a dedicated server.

use std::net::TcpListener;

use fyrox::core::instant::Instant;

use crate::{
    prelude::*,
    server::game::{ServerFrameData, ServerGame},
};

/// The process that runs a dedicated server.
pub(crate) struct ServerProcess {
    cvars: Cvars,
    pub(crate) clock: Instant,
    pub(crate) engine: Engine,
    gs: GameState,
    sg: ServerGame,
}

impl ServerProcess {
    pub(crate) async fn new(cvars: Cvars, mut engine: Engine) -> Self {
        let clock = Instant::now();

        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        listener.set_nonblocking(true).unwrap();

        let gs_type = GameStateType::Server;
        let gs = GameState::new(&cvars, &mut engine, gs_type).await;
        let sg = ServerGame::new(Box::new(listener)).await;

        let elapsed = clock.elapsed();
        dbg_logf!("ServerProcess::new() took {} ms", elapsed.as_millis());

        Self {
            cvars,
            clock,
            engine,
            gs,
            sg,
        }
    }

    /// This is similar to Client::update,
    /// see that for more information.
    pub(crate) fn update(&mut self) {
        let game_time_target = self.real_time();

        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            self.gs.frame_number += 1;

            self.sfd().tick_begin_frame();

            self.fd().tick_before_physics(dt);

            // There's currently no need to split this into pre_ and post_update like on the client.
            // Dummy control flow and lag since we don't use fyrox plugins.
            let mut cf = fyrox::event_loop::ControlFlow::Poll;
            let mut lag = 0.0;
            self.engine.update(dt, &mut cf, &mut lag, FxHashMap::default());
            // Sanity check - if the engine starts doing something with these, we'll know.
            assert_eq!(cf, fyrox::event_loop::ControlFlow::Poll);
            assert_eq!(lag, 0.0);

            // `sys_send_update` sends debug shapes and text to client.
            // Any debug calls after it will show up next frame.
            self.fd().debug_engine_updates(v!(-5 5 3));
            self.sfd().sys_send_update();
            self.fd().debug_engine_updates(v!(-6 5 3));
        }
    }

    fn sfd(&mut self) -> ServerFrameData {
        ServerFrameData {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
            sg: &mut self.sg,
        }
    }

    fn fd(&mut self) -> FrameData {
        FrameData {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
        }
    }

    pub(crate) fn real_time(&self) -> f32 {
        self.clock.elapsed().as_secs_f32()
    }
}
