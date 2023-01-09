//! Server-side gamelogic.

use std::{io::ErrorKind, mem};

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{
            AddPlayer, ClientMessage, CyclePhysics, Init, PlayerCycle, PlayerInput, ServerMessage,
            Update,
        },
        net::{self, Connection, Listener},
    },
    debug::details::{DEBUG_SHAPES, DEBUG_TEXTS},
    prelude::*,
};

/// A game server. Could be a dedicated or a listen server.
///
/// Lets clients connect to play.
pub(crate) struct ServerGame {
    // LATER Connections and the listener should probably be persistent across matches.
    listener: Box<dyn Listener>,
    clients: Pool<RemoteClient>,
}

/// All data necessary to run a frame of server-side gamelogic in one convenient package.
///
/// See also `ClientFrameData` and `FrameData`.
///
/// Note that this struct can't just _contain_ FrameData and deref into it
/// because Deref borrows self as a whole so it would be impossible
/// to access multiple fields mutably at the same time.
///
/// LATER Unsafe Deref? Same on client.
pub(crate) struct ServerFrameData<'a> {
    pub(crate) cvars: &'a Cvars,
    pub(crate) scene: &'a mut Scene,
    pub(crate) gs: &'a mut GameState,
    pub(crate) sg: &'a mut ServerGame,
}

impl ServerGame {
    pub(crate) async fn new(listener: Box<dyn Listener>) -> Self {
        Self {
            listener,
            clients: Pool::new(),
        }
    }
}

impl ServerFrameData<'_> {
    pub(crate) fn fd(&mut self) -> FrameData<'_> {
        FrameData {
            cvars: self.cvars,
            scene: self.scene,
            gs: self.gs,
        }
    }

    pub(crate) fn tick_begin_frame(&mut self) {
        self.accept_new_connections();
        self.sys_receive();
    }

    pub(crate) fn accept_new_connections(&mut self) {
        loop {
            match self.sg.listener.accept_conn() {
                Ok(conn) => {
                    dbg_logf!("accept {}", conn.addr());

                    // Add player
                    // This is sent to all clients except the new one.
                    let player = Player::new(None);
                    let player_handle = self.gs.players.spawn(player);
                    let add_player = AddPlayer {
                        name: "Player".to_owned(), // LATER from client
                        player_index: player_handle.index(),
                    };
                    let msg = ServerMessage::AddPlayer(add_player);
                    self.network_send(msg, SendDest::All);

                    // Create client
                    // This is after adding the player so that we can send the new client
                    // its own player index.
                    let client = RemoteClient::new(conn, player_handle);
                    let client_handle = self.sg.clients.spawn(client);
                    self.send_init(client_handle);

                    // Spawn cycle
                    let cycle_handle = self.fd().spawn_cycle(player_handle, None);

                    // Tell all players
                    let player_cycle = PlayerCycle {
                        player_index: player_handle.index(),
                        cycle_index: cycle_handle.index(),
                    };
                    let msg = ServerMessage::SpawnCycle(player_cycle);
                    self.network_send(msg, SendDest::All);
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
        let mut disconnected = Vec::new();
        let mut msgs_to_all = Vec::new();
        for (client_handle, client) in self.sg.clients.pair_iter_mut() {
            let (msgs, closed) = client.conn.receive_cm();
            // We might have received valid messages before the stream was closed - handle them
            // even though for some, such as player input, it doesn't affect anything.
            for msg in msgs {
                match msg {
                    ClientMessage::Input(input) => {
                        // LATER (server reconciliation) handle more inputs arriving in one frame
                        self.gs.players[client.player_handle].input = input;
                    }
                    ClientMessage::Chat(chat) => {
                        // LATER Show chat in-game
                        dbg_logd!(chat);
                    }
                    ClientMessage::Join => {
                        self.gs.players[client.player_handle].ps = PlayerState::Playing;
                        let player_index = client.player_handle.index();
                        dbg_logf!("player {} is now playing", player_index);
                        let msg = ServerMessage::Join { player_index };
                        msgs_to_all.push(msg);
                    }
                    ClientMessage::Observe => {
                        self.gs.players[client.player_handle].ps = PlayerState::Observing;
                        let player_index = client.player_handle.index();
                        dbg_logf!("player {} is now observing", player_index);
                        let msg = ServerMessage::Observe { player_index };
                        msgs_to_all.push(msg);
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
        for msg in msgs_to_all {
            self.network_send(msg, SendDest::All);
        }
    }

    fn disconnect(&mut self, client_handle: Handle<RemoteClient>) {
        let client = self.sg.clients.free(client_handle);
        self.fd().free_player(client.player_handle);
        let msg = ServerMessage::RemovePlayer {
            player_index: client.player_handle.index(),
        };
        self.network_send(msg, SendDest::All);
    }

    fn send_init(&mut self, client_handle: Handle<RemoteClient>) {
        let mut player_indices = Vec::new();
        for (player_handle, _) in self.gs.players.pair_iter() {
            player_indices.push(player_handle.index());
        }
        let local_player_index = self.sg.clients[client_handle].player_handle.index();

        let mut player_cycles = Vec::new();
        for (cycle_handle, cycle) in self.gs.cycles.pair_iter() {
            let init_player = PlayerCycle {
                player_index: cycle.player_handle.index(),
                cycle_index: cycle_handle.index(),
            };
            player_cycles.push(init_player);
        }

        let init = Init {
            player_indices,
            local_player_index,
            player_cycles,
            player_projectiles: Vec::new(), // LATER
        };
        let msg = ServerMessage::Init(init);
        self.network_send(msg, SendDest::One(client_handle));
    }

    pub(crate) fn sys_send_update(&mut self) {
        let mut player_inputs = Vec::new();
        for (player_handle, player) in self.gs.players.pair_iter() {
            let pi = PlayerInput {
                player_index: player_handle.index(),
                input: player.input,
            };
            player_inputs.push(pi);
        }

        let mut cycle_physics = Vec::new();
        for (cycle_handle, cycle) in self.gs.cycles.pair_iter() {
            let body = self.scene.graph[cycle.body_handle].as_rigid_body();
            let cp = CyclePhysics {
                cycle_index: cycle_handle.index(),
                translation: **body.local_transform().position(),
                rotation: **body.local_transform().rotation(),
                velocity: body.lin_vel(),
            };
            cycle_physics.push(cp);
        }

        // Send debug items, then clear everything on the server
        // so it doesn't get sent again next frame.
        // Calling debug::details::cleanup() would only clear expired.
        let debug_texts = DEBUG_TEXTS.with(|texts| {
            let mut texts = texts.borrow_mut();
            mem::take(&mut *texts)
        });
        let debug_shapes = DEBUG_SHAPES.with(|shapes| {
            let mut shapes = shapes.borrow_mut();
            mem::take(&mut *shapes)
        });

        let msg = ServerMessage::Update(Update {
            player_inputs,
            cycle_physics,
            debug_texts,
            debug_shapes,
        });
        self.network_send(msg, SendDest::All);
    }

    // LATER This only needs Engine for self.disconnect,
    // but forces all callers to also take Engine.
    fn network_send(&mut self, msg: ServerMessage, dest: SendDest) {
        // LATER This is incredibly ugly, plus creating the Vec is inafficient.
        //          - Save all streams in a Vec?
        //          - Inline this fn and remove SendDest?
        let mut disconnected = Vec::new();
        let network_msg = net::serialize(msg);
        match dest {
            SendDest::One(handle) => {
                if let Err(e) = self.sg.clients[handle].conn.send(&network_msg) {
                    dbg_logf!("Error in network_send One - index {}: {:?}", handle.index(), e);
                    disconnected.push(handle);
                }
            }
            SendDest::All => {
                for (handle, client) in self.sg.clients.pair_iter_mut() {
                    if let Err(e) = client.conn.send(&network_msg) {
                        dbg_logf!("Error in network_send All - index {}: {:?}", handle.index(), e);
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
    conn: Box<dyn Connection>,
    player_handle: Handle<Player>,
}

impl RemoteClient {
    fn new(conn: Box<dyn Connection>, player_handle: Handle<Player>) -> Self {
        Self {
            conn,
            player_handle,
        }
    }
}
