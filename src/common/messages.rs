//! Messages sent between the client and server, usually over the network.
//!
//! LATER These will form the basis of demo recording and replay.

use crate::common::Input;

use rg3d::core::algebra::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ClientMessage {
    Input(Input),
    Chat(String), // LATER Allow sending this
    Join,
    Observe,
}

/// Message sent from server to client
#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ServerMessage {
    /// Initial game state that is sent to a new player upon connecting.
    ///
    /// This is intentionally separate from messages such as AddPlayer or SpawnCycle
    /// because eventually those might trigger additional effects
    /// such as info messages, sounds, particles, etc.
    InitData(InitData),
    /// Add a new player to the game.
    AddPlayer(AddPlayer),
    /// Remove the player and all data associated with him, for example when he disconnects.
    RemovePlayer { player_index: u32 },
    /// Spawn a new cycle for an existing player.
    SpawnCycle(PlayerCycle),
    /// Remove the cycle from game state, for example when the player switches to observer mode.
    DespawnCycle { cycle_index: u32 },
    /// Update the translations, rotations, velocities, etc. of everything.
    UpdatePhysics(UpdatePhysics),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InitData {
    pub(crate) player_indices: Vec<u32>,
    pub(crate) local_player_index: u32,
    pub(crate) player_cycles: Vec<PlayerCycle>,
    pub(crate) player_projectiles: Vec<PlayerProjectile>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct AddPlayer {
    pub(crate) player_index: u32,
    pub(crate) name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DespawnCycle {
    pub(crate) cycle_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PlayerCycle {
    pub(crate) player_index: u32,
    pub(crate) cycle_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PlayerProjectile {
    pub(crate) player_index: u32,
    pub(crate) projectile_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct UpdatePhysics {
    pub(crate) cycle_physics: Vec<CyclePhysics>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct CyclePhysics {
    pub(crate) cycle_index: u32,
    pub(crate) translation: Vector3<f32>,
    pub(crate) velocity: Vector3<f32>,
}
