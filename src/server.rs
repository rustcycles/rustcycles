use std::{
    io::{ErrorKind, Read},
    net::{SocketAddr, TcpListener, TcpStream},
};

use crate::{
    common::{GameState, Input},
    GameEngine,
};

pub(crate) struct Server {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) input: Input,
    listener: TcpListener,
    clients: Vec<RemoteClient>,
}

struct RemoteClient {
    stream: TcpStream,
    addr: SocketAddr,
}

impl Server {
    pub(crate) async fn new(mut engine: GameEngine) -> Self {
        let gs = GameState::new(&mut engine).await;

        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        listener.set_nonblocking(true).unwrap();

        Self {
            engine,
            gs,
            input: Input::default(),
            listener,
            clients: Vec::new(),
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

            self.network_send();
        }
    }

    fn network_receive(&mut self) {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                // TODO set_nodelay to disable Nagle'a algo? (also on Client)
                stream.set_nonblocking(true).unwrap(); // TODO needed?
                println!("S accept {}", addr);
                let client = RemoteClient { stream, addr };
                self.clients.push(client);
            }
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => {}
                _ => panic!("network error (accept): {}", err),
            },
        }

        for client in &mut self.clients {
            let mut buf = [0; 6];
            loop {
                // We're reading in a loop in case more packets arrive in one frame.
                let res = client.stream.read_exact(&mut buf);
                match res {
                    Ok(_) => {
                        let input: Input = bincode::deserialize(&buf).unwrap();
                        println!("S received from {}: {:?}", client.addr, input);
                    }
                    Err(err) => match err.kind() {
                        ErrorKind::WouldBlock => {
                            break;
                        }
                        _ => panic!("network error (read): {}", err),
                    },
                }
            }
        }
    }

    fn network_send(&mut self) {}
}
