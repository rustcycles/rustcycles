//! The authoritative server in a client-server multiplayer game architecture.

use std::{
    collections::VecDeque,
    io::ErrorKind,
    net::{SocketAddr, TcpListener, TcpStream},
};

use rg3d::{core::pool::Pool, engine::Engine};

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{
            AddPlayer, ClientMessage, CyclePhysics, InitData, PlayerCycle, ServerMessage,
            UpdatePhysics,
        },
        net, GameState,
    },
    prelude::*,
};

/// Game server.
///
/// Lets Clients connect to play. Contains the authoritate copy of the game state.
pub(crate) struct GameServer {
    pub(crate) engine: Engine,
    pub(crate) gs: GameState,
    listener: TcpListener,
    clients: Pool<RemoteClient>,
}

impl GameServer {
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
                    dbg_logf!("accept {}", addr);

                    // Add player
                    // This is sent to all clients except the new one.
                    let player = Player::new(None);
                    let player_handle = self.gs.players.spawn(player);
                    let add_player = AddPlayer {
                        name: "Player".to_owned(), // LATER from client
                        player_index: player_handle.index(),
                    };
                    let message = ServerMessage::AddPlayer(add_player);
                    self.network_send(message, SendDest::All);

                    // Create client
                    // This is after adding the player so that we can sent the new client its own player index.
                    let client = RemoteClient::new(stream, addr, player_handle);
                    let client_handle = self.clients.spawn(client);
                    self.send_init(client_handle);

                    // Spawn cycle
                    let scene = &mut self.engine.scenes[self.gs.scene];
                    let cycle_handle = self.gs.spawn_cycle(scene, player_handle, None);

                    // Tell all players
                    let player_cycle = PlayerCycle {
                        player_index: player_handle.index(),
                        cycle_index: cycle_handle.index(),
                    };
                    let message = ServerMessage::SpawnCycle(player_cycle);
                    self.network_send(message, SendDest::All);
                }
                Err(err) => match err.kind() {
                    ErrorKind::WouldBlock => {
                        break;
                    }
                    _ => dbg_panic!("network error (accept): {}", err),
                },
            }
        }
    }

    fn sys_receive(&mut self) {
        let mut disconnected = Vec::new();
        let mut messages_to_all = Vec::new();
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
                        todo!("ClientMessage::Chat");
                    }
                    ClientMessage::Join => {
                        self.gs.players[client.player_handle].ps = PlayerState::Playing;
                        let player_index = client.player_handle.index();
                        dbg_logf!("player {} is now playing", player_index);
                        let msg = ServerMessage::Join { player_index };
                        messages_to_all.push(msg);
                    }
                    ClientMessage::Observe => {
                        self.gs.players[client.player_handle].ps = PlayerState::Observing;
                        let player_index = client.player_handle.index();
                        dbg_logf!("player {} is now observing", player_index);
                        let msg = ServerMessage::Observe { player_index };
                        messages_to_all.push(msg);
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
        for message in messages_to_all {
            self.network_send(message, SendDest::All);
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
        let mut player_indices = Vec::new();
        for (player_handle, _) in self.gs.players.pair_iter() {
            player_indices.push(player_handle.index());
        }
        let local_player_index = self.clients[client_handle].player_handle.index();

        let mut player_cycles = Vec::new();
        for (cycle_handle, cycle) in self.gs.cycles.pair_iter() {
            let init_player = PlayerCycle {
                player_index: cycle.player_handle.index(),
                cycle_index: cycle_handle.index(),
            };
            player_cycles.push(init_player);
        }

        let init_data = InitData {
            player_indices,
            local_player_index,
            player_cycles,
            player_projectiles: Vec::new(), // LATER
        };
        let message = ServerMessage::InitData(init_data);
        self.network_send(message, SendDest::One(client_handle));
    }

    fn sys_send_update(&mut self) {
        let scene = &self.engine.scenes[self.gs.scene];
        let mut cycle_physics = Vec::new();
        for (cycle_handle, cycle) in self.gs.cycles.pair_iter() {
            let body = scene.graph[cycle.body_handle].as_rigid_body();
            let update = CyclePhysics {
                cycle_index: cycle_handle.index(),
                translation: **body.local_transform().position(),
                rotation: **body.local_transform().rotation(),
                velocity: body.lin_vel(),
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
                    dbg_logf!(
                        "Error in network_send One - index {}: {:?}",
                        handle.index(),
                        e
                    );
                    disconnected.push(handle);
                }
            }
            SendDest::All => {
                for (handle, client) in self.clients.pair_iter_mut() {
                    if let Err(e) = net::send(&network_message, &mut client.stream) {
                        dbg_logf!(
                            "Error in network_send All - index {}: {:?}",
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
