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
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ServerMessage {
    /// Add initial data that is sent to a new player upon connecting.
    InitData(InitData),
    /// Add a new player to the game.
    AddPlayer(AddPlayer),
    /// Remove the player and all data associated with him, for example when he disconnects.
    RemovePlayer { player_index: u32 },
    /// Spawn a new cycle for an existing player.
    SpawnCycle(SpawnCycle),
    /// Remove the cycle from game state, for example when the player switches to observer mode.
    DespawnCycle { cycle_index: u32 },
    /// Update the translations, rotations, velocities, etc. of everything.
    UpdatePhysics(UpdatePhysics),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InitData {
    pub(crate) player_cycles: Vec<PlayerCycle>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct AddPlayer {
    pub(crate) player_index: u32,
    // LATER Name and maybe other fields
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SpawnCycle {
    pub(crate) player_cycle: PlayerCycle,
    // LATER If no fields are added here, might as well remove this struct and use the u32 directly in the enum
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DespawnCycle {
    pub(crate) cycle_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PlayerCycle {
    pub(crate) player_index: u32,
    pub(crate) cycle_index: Option<u32>,
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
