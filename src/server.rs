use std::{io::Read, net::TcpListener};

use crate::{
    common::{GameState, Input},
    GameEngine,
};

pub(crate) struct Server {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) input: Input,
}

impl Server {
    pub(crate) async fn new(mut engine: GameEngine) -> Self {
        let gs = GameState::new(&mut engine).await;

        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        //listener.set_nonblocking(true).unwrap();
        // TODO set_nodelay ?
        let (mut stream, addr) = listener.accept().unwrap();
        println!("S accept {}", addr);
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).unwrap(); // FIXME
        println!("S read_to_end {:?}", buf);

        Self {
            engine,
            gs,
            input: Input::default(),
        }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        // This is similar to Client::update,
        // see that for more information.

        let dt = 1.0 / 60.0;
        let game_time_target = game_time_target;

        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            // TODO input
            self.gs.tick(&mut self.engine, dt, self.input);

            self.engine.update(dt);
        }

        // TODO Send updates to clients
    }
}
