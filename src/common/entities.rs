//! The E and C in ECS
//!
//! We're using the ECS design pattern (decoupling behavior from data),
//! just without the ECS data structure (we use generational arenas / pools instead to retain static typing).
//! Most game data goes here - entities are structs, components are fields.
//!
//! Some entities have pure member functions.
//! This is not a violation of the ECS pattern,
//! because they don't modify game state - they're not behavior.

use crate::{common::Input, prelude::*};

/// A client connected to a server. Can be observing, spectating or playing.
#[derive(Debug)]
pub struct Player {
    #[allow(dead_code)]
    pub name: String, // TODO Use, remove allow
    pub state: PlayerState,
    pub input: Input,
    pub cycle_handle: Option<Handle<Cycle>>,
}

impl Player {
    pub fn new(cycle_handle: Option<Handle<Cycle>>) -> Self {
        Self {
            name: "unnamed".to_owned(), // TODO
            state: PlayerState::Observing,
            input: Input::default(),
            cycle_handle,
        }
    }
}
//
/// How the player is participating in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// The player is a freely floating camera observing the game.
    Observing,
    /// The player is watching another player's POV - LATER
    #[allow(dead_code)]
    Spectating { spectatee_handle: Handle<Player> },
    /// The player is playing
    Playing,
}

#[derive(Debug)]
pub struct Cycle {
    pub player_handle: Handle<Player>,
    pub body_handle: Handle<Node>,
    pub collider_handle: Handle<Node>,
    pub time_last_fired: f32,
}

#[derive(Debug)]
pub struct Projectile {
    pub player_handle: Handle<Player>,
    pub pos: Vec3,
    pub vel: Vec3,
    pub time_fired: f32,
}
