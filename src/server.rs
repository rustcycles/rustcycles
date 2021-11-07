use std::{
    io::{ErrorKind, Read, Write},
    mem,
    net::{SocketAddr, TcpListener, TcpStream},
};

use crate::{
    common::{GameState, Input, ServerPacket},
    GameEngine,
};

pub(crate) struct Server {
    pub(crate) engine: GameEngine,
    pub(crate) gs: GameState,
    pub(crate) input: Input,
    listener: TcpListener,
    clients: Vec<RemoteClient>,
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

            self.accept_new_connections();
            self.network_receive();

            // TODO input
            self.gs.tick(&mut self.engine, dt, self.input);

            self.engine.update(dt);

            self.network_send();
        }
    }

    fn accept_new_connections(&mut self) {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                // TODO set_nodelay to disable Nagle'a algo? (also on Client)
                stream.set_nonblocking(true).unwrap(); // TODO needed?
                println!("S accept {}", addr);
                let client = RemoteClient::new(stream, addr);
                self.clients.push(client);
            }
            Err(err) => match err.kind() {
                ErrorKind::WouldBlock => {}
                _ => panic!("network error (accept): {}", err),
            },
        }
    }

    fn network_receive(&mut self) {
        for client in &mut self.clients {
            // TODO Read from network properly.
            // Using size of Input is also probably wrong,
            // bincode doesn't guarantee same size.
            let mut buf = [0; mem::size_of::<Input>()];
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

    fn network_send(&mut self) {
        // LATER Measure network usage.
        // LATER Try to minimize network usage.
        //       General purpose compression could help a bit,
        //       but using what we know about the data should give much better results.
        let scene = &self.engine.scenes[self.gs.scene];
        let pos1 = scene.graph[self.gs.cycle1.node_handle].global_position();
        let pos2 = scene.graph[self.gs.cycle2.node_handle].global_position();
        let packet = ServerPacket {
            positions: vec![pos1, pos2],
        };
        let buf = bincode::serialize(&packet).unwrap();
        let len = u16::try_from(buf.len()).unwrap().to_le_bytes();
        for client in &mut self.clients {
            // Prefix data by length so it's easy to parse on the other side.
            client.stream.write_all(&len).unwrap();
            client.stream.write_all(&buf).unwrap();
        }
    }
}

struct RemoteClient {
    stream: TcpStream,
    addr: SocketAddr,
}

impl RemoteClient {
    fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        Self { stream, addr }
    }
}
