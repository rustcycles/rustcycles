use std::{
    io::{ErrorKind, Read, Write},
    mem,
    net::{SocketAddr, TcpListener, TcpStream},
};

use rg3d::{
    core::pool::{Handle, Pool},
    engine::Engine,
};

use crate::common::{
    AddPlayer, CyclePhysics, GameState, InitData, Input, Player, PlayerCycle, ServerMessage,
    SpawnCycle, UpdatePhysics,
};

pub(crate) struct Server {
    pub(crate) engine: Engine,
    pub(crate) gs: GameState,
    listener: TcpListener,
    clients: Pool<RemoteClient>,
}

impl Server {
    pub(crate) async fn new(mut engine: Engine) -> Self {
        let gs = GameState::new(&mut engine).await;

        let listener = TcpListener::bind("127.0.0.1:26000").unwrap();
        listener.set_nonblocking(true).unwrap();

        Self {
            engine,
            gs,
            listener,
            clients: Pool::new(),
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
            self.gs.tick(&mut self.engine, dt);

            self.engine.update(dt);

            self.send_update();
        }
    }

    fn accept_new_connections(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    // LATER set_nodelay to disable Nagle'a algo? (also on Client)
                    stream.set_nonblocking(true).unwrap();
                    println!("S accept {}", addr);

                    // Create client
                    let client = RemoteClient::new(stream, addr, Handle::NONE);
                    let client_handle = self.clients.spawn(client);
                    // This is intentionally before spawning entities
                    // 1) we can differentiate between syncing existing entities to new clients
                    //      and spawning new entities - there might be spawn effects/sounds.
                    // 2) we can add support for players without cycles - observers/spectators.
                    self.send_init(client_handle);

                    // Add player
                    let player = Player::new(None);
                    let player_handle = self.gs.players.spawn(player);
                    self.clients[client_handle].player_handle = player_handle;
                    let add_player = AddPlayer {
                        player_index: player_handle.index(),
                    };
                    let packet = ServerMessage::AddPlayer(add_player);
                    self.network_send(packet, SendDest::All);

                    // Spawn cycle
                    let scene = &mut self.engine.scenes[self.gs.scene];
                    let cycle_handle = self.gs.spawn_cycle(scene, player_handle, None);
                    self.gs.players[player_handle].cycle_handle = Some(cycle_handle);
                    dbg!(cycle_handle);

                    // Tell all players
                    let player_cycle = PlayerCycle {
                        player_index: player_handle.index(),
                        cycle_index: Some(cycle_handle.index()),
                    };
                    let spawn_cycle = SpawnCycle { player_cycle };
                    let packet = ServerMessage::SpawnCycle(spawn_cycle);
                    self.network_send(packet, SendDest::All);
                }
                Err(err) => match err.kind() {
                    ErrorKind::WouldBlock => {
                        break;
                    }
                    _ => panic!("network error (accept): {}", err),
                },
            }
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
                        //println!("S received from {}: {:?}", client.addr, input);
                        self.gs.players[client.player_handle].input = input;
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

    fn send_init(&mut self, client_handle: Handle<RemoteClient>) {
        let mut player_cycles = Vec::new();
        for (player_handle, player) in self.gs.players.pair_iter() {
            let init_player = PlayerCycle {
                player_index: player_handle.index(),
                cycle_index: player.cycle_handle.map(|handle| handle.index()),
            };
            player_cycles.push(init_player);
        }

        let init_data = InitData { player_cycles };
        let packet = ServerMessage::InitData(init_data);
        self.network_send(packet, SendDest::One(client_handle));
    }

    fn send_update(&mut self) {
        let scene = &self.engine.scenes[self.gs.scene];
        let mut cycle_physics = Vec::new();
        for (cycle_handle, cycle) in self.gs.cycles.pair_iter() {
            let body = scene.physics.bodies.get(&cycle.body_handle).unwrap();
            let update = CyclePhysics {
                cycle_index: cycle_handle.index(),
                translation: *body.translation(),
                velocity: *body.linvel(),
            };
            cycle_physics.push(update);
        }
        let update_physics = UpdatePhysics { cycle_physics };
        let packet = ServerMessage::UpdatePhysics(update_physics);
        self.network_send(packet, SendDest::All);
    }

    fn network_send(&mut self, packet: ServerMessage, dest: SendDest) {
        // LATER Measure network usage.
        // LATER Try to minimize network usage.
        //       General purpose compression could help a bit,
        //       but using what we know about the data should give much better results.

        let buf = bincode::serialize(&packet).unwrap();
        let len = u16::try_from(buf.len()).unwrap().to_le_bytes();
        match dest {
            SendDest::One(handle) => {
                let client = &mut self.clients[handle];
                Self::send_bytes(&buf, len, client);
            }
            SendDest::All => {
                for client in &mut self.clients {
                    Self::send_bytes(&buf, len, client);
                }
            }
        }
    }

    fn send_bytes(buf: &[u8], len: [u8; 2], client: &mut RemoteClient) {
        // Prefix data by length so it's easy to parse on the other side.
        client.stream.write_all(&len).unwrap();
        client.stream.write_all(buf).unwrap();
    }
}

enum SendDest {
    One(Handle<RemoteClient>),
    All,
}

struct RemoteClient {
    stream: TcpStream,
    #[allow(dead_code)]
    addr: SocketAddr,
    player_handle: Handle<Player>,
}

impl RemoteClient {
    fn new(stream: TcpStream, addr: SocketAddr, player_handle: Handle<Player>) -> Self {
        Self {
            stream,
            addr,
            player_handle,
        }
    }
}
