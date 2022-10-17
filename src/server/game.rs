//! Server-side gamelogic.

use std::io::ErrorKind;

use crate::{
    common::{
        entities::{Player, PlayerState},
        messages::{
            AddPlayer, ClientMessage, CyclePhysics, Init, PlayerCycle, PlayerInput, ServerMessage,
            Update,
        },
        net::{self, Connection, Listener},
        GameState,
    },
    debug::details::{DEBUG_SHAPES, DEBUG_TEXTS},
    prelude::*,
};

/// A game server. Could be dedicated or a listen server.
///
/// Lets clients connect to play. Contains the authoritative copy of the game state.
pub(crate) struct ServerGame {
    pub(crate) gs: GameState,
    listener: Box<dyn Listener>,
    clients: Pool<RemoteClient>,
}

impl ServerGame {
    pub(crate) async fn new(engine: &mut Engine, listener: Box<dyn Listener>) -> Self {
        let gs = GameState::new(engine).await;

        Self {
            gs,
            listener,
            clients: Pool::new(),
        }
    }

    pub(crate) fn update(&mut self, engine: &mut Engine, game_time_target: f32) {
        // This is similar to Client::update,
        // see that for more information.

        let dt = 1.0 / 60.0;
        while self.gs.game_time + dt < game_time_target {
            self.gs.game_time_prev = self.gs.game_time;
            self.gs.game_time += dt;
            self.gs.frame_number += 1;

            self.tick_begin_frame(engine);

            self.gs.tick_before_physics(engine, dt);

            // There's currently no need to split this into pre_ and post_update like on the client.
            // Dummy control flow and lag since we don't use fyrox plugins.
            let mut cf = fyrox::event_loop::ControlFlow::Poll;
            let mut lag = 0.0;
            engine.update(dt, &mut cf, &mut lag);
            // Sanity check - if the engine starts doing something with these, we'll know.
            assert_eq!(cf, fyrox::event_loop::ControlFlow::Poll);
            assert_eq!(lag, 0.0);

            // `sys_send_update` sends debug shapes and text to client.
            // Any debug calls after it will show up next frame.
            self.gs.debug_engine_updates(v!(-5 5 3), 4);
            self.sys_send_update(engine);
            self.gs.debug_engine_updates(v!(-6 5 3), 4);
        }
    }

    fn tick_begin_frame(&mut self, engine: &mut Engine) {
        self.accept_new_connections(engine);
        self.sys_receive(engine);
    }

    pub(crate) fn accept_new_connections(&mut self, engine: &mut Engine) {
        loop {
            match self.listener.accept_conn() {
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
                    self.network_send(engine, msg, SendDest::All);

                    // Create client
                    // This is after adding the player so that we can send the new client
                    // its own player index.
                    let client = RemoteClient::new(conn, player_handle);
                    let client_handle = self.clients.spawn(client);
                    self.send_init(engine, client_handle);

                    // Spawn cycle
                    let scene = &mut engine.scenes[self.gs.scene];
                    let cycle_handle = self.gs.spawn_cycle(scene, player_handle, None);

                    // Tell all players
                    let player_cycle = PlayerCycle {
                        player_index: player_handle.index(),
                        cycle_index: cycle_handle.index(),
                    };
                    let msg = ServerMessage::SpawnCycle(player_cycle);
                    self.network_send(engine, msg, SendDest::All);
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

    fn sys_receive(&mut self, engine: &mut Engine) {
        let mut disconnected = Vec::new();
        let mut msgs_to_all = Vec::new();
        for (client_handle, client) in self.clients.pair_iter_mut() {
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
            self.disconnect(engine, client_handle);
        }
        for msg in msgs_to_all {
            self.network_send(engine, msg, SendDest::All);
        }
    }

    fn disconnect(&mut self, engine: &mut Engine, client_handle: Handle<RemoteClient>) {
        let scene = &mut engine.scenes[self.gs.scene];
        let client = self.clients.free(client_handle);
        self.gs.free_player(scene, client.player_handle);
        let msg = ServerMessage::RemovePlayer {
            player_index: client.player_handle.index(),
        };
        self.network_send(engine, msg, SendDest::All);
    }

    fn send_init(&mut self, engine: &mut Engine, client_handle: Handle<RemoteClient>) {
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

        let init = Init {
            player_indices,
            local_player_index,
            player_cycles,
            player_projectiles: Vec::new(), // LATER
        };
        let msg = ServerMessage::Init(init);
        self.network_send(engine, msg, SendDest::One(client_handle));
    }

    fn sys_send_update(&mut self, engine: &mut Engine) {
        let scene = &engine.scenes[self.gs.scene];

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
            let body = scene.graph[cycle.body_handle].as_rigid_body();
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
            let ret = texts.clone();
            texts.clear();
            ret
        });
        let debug_shapes = DEBUG_SHAPES.with(|shapes| {
            let mut shapes = shapes.borrow_mut();
            let ret = shapes.clone();
            shapes.clear();
            ret
        });

        let msg = ServerMessage::Update(Update {
            player_inputs,
            cycle_physics,
            debug_texts,
            debug_shapes,
        });
        self.network_send(engine, msg, SendDest::All);
    }

    // LATER This only needs Engine for self.disconnect,
    // but forces all callers to also take Engine.
    fn network_send(&mut self, engine: &mut Engine, msg: ServerMessage, dest: SendDest) {
        // LATER This is incredibly ugly, plus creating the Vec is inafficient.
        //          - Save all streams in a Vec?
        //          - Inline this fn and remove SendDest?
        let mut disconnected = Vec::new();
        let network_msg = net::serialize(msg);
        match dest {
            SendDest::One(handle) => {
                if let Err(e) = self.clients[handle].conn.send(&network_msg) {
                    dbg_logf!("Error in network_send One - index {}: {:?}", handle.index(), e);
                    disconnected.push(handle);
                }
            }
            SendDest::All => {
                for (handle, client) in self.clients.pair_iter_mut() {
                    if let Err(e) = client.conn.send(&network_msg) {
                        dbg_logf!("Error in network_send All - index {}: {:?}", handle.index(), e);
                        disconnected.push(handle);
                    }
                }
            }
        };
        for client_handle in disconnected {
            self.disconnect(engine, client_handle);
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
