//! The process that runs a dedicated server.

use std::net::TcpListener;

use fyrox::core::instant::Instant;

use crate::{
    debug,
    prelude::*,
    server::game::{ServerFrameCtx, ServerGame},
};

/// The process that runs a dedicated server.
pub struct ServerProcess {
    cvars: Cvars,
    pub clock: Instant,
    pub engine: Engine,
    gs: GameState,
    sg: ServerGame,
}

impl ServerProcess {
    pub async fn new(cvars: Cvars, mut engine: Engine) -> Self {
        let clock = Instant::now();

        let listener = TcpListener::bind(&cvars.sv_net_listen_addr).unwrap();
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
    pub fn update(&mut self) {
        let game_time_target = self.real_time();

        let dt_update = game_time_target - self.gs.game_time;
        if dt_update > 5.0 {
            dbg_logf!("large dt_update: {dt_update}");
        }

        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.frame_num += 1;
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            debug::set_game_time(self.gs.game_time);

            self.sv_ctx().tick_begin_frame();

            self.ctx().tick_before_physics(dt);

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
            self.ctx().debug_engine_updates(v!(-5 5 3));
            self.sv_ctx().sys_send_update();
            self.ctx().debug_engine_updates(v!(-6 5 3));
        }
    }

    fn sv_ctx(&mut self) -> ServerFrameCtx {
        ServerFrameCtx {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
            sg: &mut self.sg,
        }
    }

    fn ctx(&mut self) -> FrameCtx {
        FrameCtx {
            cvars: &self.cvars,
            scene: &mut self.engine.scenes[self.gs.scene_handle],
            gs: &mut self.gs,
        }
    }

    pub fn real_time(&self) -> f32 {
        self.clock.elapsed().as_secs_f32()
    }
}
