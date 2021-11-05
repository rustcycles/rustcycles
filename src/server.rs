use std::{
    io::{ErrorKind, Read},
    net::{TcpListener, TcpStream},
};

use crate::{
    common::{GameState, Input},
    GameEngine,
};

pub(crate) struct Server {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) input: Input,
    stream: TcpStream,
}

impl Server {
    pub(crate) async fn new(mut engine: GameEngine) -> Self {
        let gs = GameState::new(&mut engine).await;

        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        //listener.set_nonblocking(true).unwrap();
        // TODO set_nodelay ?
        let (stream, addr) = listener.accept().unwrap();
        stream.set_nonblocking(true).unwrap();
        println!("S accept {}", addr);

        Self {
            engine,
            gs,
            input: Input::default(),
            stream,
        }
    }

    pub(crate) fn update(&mut self, game_time_target: f32) {
        // This is similar to Client::update,
        // see that for more information.

        let dt = 1.0 / 60.0;
        let game_time_target = game_time_target;

        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time += dt;

            self.network_receive();

            // TODO input
            self.gs.tick(&mut self.engine, dt, self.input);

            self.engine.update(dt);
        }

        // TODO Send updates to clients
    }

    fn network_receive(&mut self) {
        let mut buf = [0; 16];
        let res = self.stream.read_exact(&mut buf);
        match res {
            Ok(_) => {
                let s = String::from_utf8(buf.to_vec()).unwrap();
                println!("S received: {:?}", s);
            }
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => {}
                _ => panic!("network error: {}", err),
            },
        }
    }
}
