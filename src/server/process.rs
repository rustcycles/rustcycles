//! The process that runs a dedicated server.

use crate::{prelude::*, server::game::ServerGame};

/// The process that runs a dedicated server.
pub(crate) struct ServerProcess {
    pub(crate) engine: Engine,
    sg: ServerGame,
}

impl ServerProcess {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let sg = ServerGame::new(&mut engine).await;

        Self { engine, sg }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        self.sg.update(&mut self.engine, game_time_target);
    }
}
