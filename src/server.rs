//! The authoritative server in a client-server multiplayer game architecture.

use std::{
    collections::VecDeque,
    io::ErrorKind,
    net::{SocketAddr, TcpListener, TcpStream},
};

use rg3d::{
    core::pool::{Handle, Pool},
    engine::Engine,
};

use crate::common::{
    entities::Player,
    messages::{
        AddPlayer, ClientMessage, CyclePhysics, InitData, PlayerCycle, ServerMessage, SpawnCycle,
        UpdatePhysics,
    },
    net, GameState,
};

/// Game server.
///
/// Lets Clients connect to play. Contains the authoritate copy of the game state.
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
            self.sys_receive();

            self.gs.tick(&mut self.engine, dt);

            self.engine.update(dt);

            self.sys_send_update();
        }
    }

    fn accept_new_connections(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    // LATER Measure if nodelay actually makes a difference,
                    // or better yet, replace TCP with something better.
                    // Same on the client.
                    // Also how does it interact with flushing the stram after each write?
                    stream.set_nodelay(true).unwrap();
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
                    let message = ServerMessage::AddPlayer(add_player);
                    self.network_send(message, SendDest::All);

                    // Spawn cycle
                    let scene = &mut self.engine.scenes[self.gs.scene];
                    let cycle_handle = self.gs.spawn_cycle(scene, player_handle, None);

                    // Tell all players
                    let player_cycle = PlayerCycle {
                        player_index: player_handle.index(),
                        cycle_index: Some(cycle_handle.index()),
                    };
                    let spawn_cycle = SpawnCycle { player_cycle };
                    let message = ServerMessage::SpawnCycle(spawn_cycle);
                    self.network_send(message, SendDest::All);
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

    fn sys_receive(&mut self) {
        // TODO Pool::handle_iter()?
        let mut disconnected = Vec::new();
        for (client_handle, client) in self.clients.pair_iter_mut() {
            let mut messages: Vec<ClientMessage> = Vec::new();
            let closed = net::receive(&mut client.stream, &mut client.buffer, &mut messages);
            // We might have received valid messages before the stream was closed - handle them
            // even though for some, such as player input, it doesn't affect anything.
            for message in messages {
                match message {
                    ClientMessage::Input(input) => {
                        // LATER (server reconcilliation) handle more inputs arriving in one frame
                        self.gs.players[client.player_handle].input = input;
                    }
                    ClientMessage::Chat(chat) => {
                        dbg!(chat);
                        todo!();
                    }
                }
            }
            if closed {
                disconnected.push(client_handle);
            }
        }
        for client_handle in disconnected {
            self.disconnect(client_handle);
        }
    }

    fn disconnect(&mut self, client_handle: Handle<RemoteClient>) {
        let scene = &mut self.engine.scenes[self.gs.scene];
        let client = self.clients.free(client_handle);
        self.gs.free_player(scene, client.player_handle);
        let message = ServerMessage::RemovePlayer {
            player_index: client.player_handle.index(),
        };
        self.network_send(message, SendDest::All);
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
        let message = ServerMessage::InitData(init_data);
        self.network_send(message, SendDest::One(client_handle));
    }

    fn sys_send_update(&mut self) {
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
        let message = ServerMessage::UpdatePhysics(update_physics);
        self.network_send(message, SendDest::All);
    }

    fn network_send(&mut self, message: ServerMessage, dest: SendDest) {
        // LATER This is incredibly ugly, plus creating the Vec is inafficient.
        //          - Save all streams in a Vec?
        //          - Inline this fn and remove SendDest?
        let mut disconnected = Vec::new();
        let network_message = net::serialize(message);
        match dest {
            SendDest::One(handle) => {
                if let Err(e) = net::send(&network_message, &mut self.clients[handle].stream) {
                    println!(
                        "S Error in network_send One - index {}: {:?}",
                        handle.index(),
                        e
                    );
                    disconnected.push(handle);
                }
            }
            SendDest::All => {
                for (handle, client) in self.clients.pair_iter_mut() {
                    if let Err(e) = net::send(&network_message, &mut client.stream) {
                        println!(
                            "S Error in network_send All - index {}: {:?}",
                            handle.index(),
                            e
                        );
                        disconnected.push(handle);
                    }
                }
            }
        };
        for client_handle in disconnected {
            self.disconnect(client_handle);
        }
    }
}

enum SendDest {
    One(Handle<RemoteClient>),
    All,
}

struct RemoteClient {
    stream: TcpStream,
    buffer: VecDeque<u8>,
    #[allow(dead_code)]
    addr: SocketAddr,
    player_handle: Handle<Player>,
}

impl RemoteClient {
    fn new(stream: TcpStream, addr: SocketAddr, player_handle: Handle<Player>) -> Self {
        Self {
            stream,
            buffer: VecDeque::new(),
            addr,
            player_handle,
        }
    }
}
