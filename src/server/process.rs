//! The process that runs a dedicated server.

use std::net::TcpListener;

use fyrox::core::instant::Instant;

use crate::{prelude::*, server::game::ServerGame};

/// The process that runs a dedicated server.
pub(crate) struct ServerProcess {
    cvars: Cvars,
    pub(crate) clock: Instant,
    pub(crate) engine: Engine,
    sg: ServerGame,
}

impl ServerProcess {
    pub(crate) async fn new(cvars: Cvars, mut engine: Engine) -> Self {
        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        listener.set_nonblocking(true).unwrap();

        let sg = ServerGame::new(&mut engine, Box::new(listener)).await;

        Self {
            cvars,
            clock: Instant::now(),
            engine,
            sg,
        }
    }

    pub(crate) fn update(&mut self) {
        let target = self.real_time();
        self.sg.update(&self.cvars, &mut self.engine, target);
    }

    pub(crate) fn real_time(&self) -> f32 {
        self.clock.elapsed().as_secs_f32()
    }
}
