//! The process that runs a dedicated server.

use std::net::TcpListener;

use crate::{prelude::*, server::game::ServerGame};

/// The process that runs a dedicated server.
pub(crate) struct ServerProcess {
    pub(crate) engine: Engine,
    sg: ServerGame,
}

impl ServerProcess {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        listener.set_nonblocking(true).unwrap();

        let sg = ServerGame::new(&mut engine, Box::new(listener)).await;

        Self { engine, sg }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        self.sg.update(&mut self.engine, game_time_target);
    }
}
