//! Messages sent between the client and server, usually over the network.
//!
//! LATER These will form the basis of demo recording and replay.

use crate::common::Input;

use rg3d::core::algebra::Vector3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ClientMessage {
    Input(Input),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum ServerMessage {
    InitData(InitData),
    AddPlayer(AddPlayer),
    SpawnCycle(SpawnCycle),
    UpdatePhysics(UpdatePhysics),
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct InitData {
    pub(crate) player_cycles: Vec<PlayerCycle>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct AddPlayer {
    pub(crate) player_index: u32,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct SpawnCycle {
    pub(crate) player_cycle: PlayerCycle,
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
