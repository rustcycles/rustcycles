//! The E and C in ECS
//!
//! We're using the ECS design pattern (decoupling behavior from data),
//! just without the ECS data structure (we use generational arenas / pools instead to retain static typing).
//! Most game data goes here - entities are structs, components are fields.
//!
//! Some entities have pure member functions.
//! This is not a violation of the ECS pattern,
//! because they don't modify game state - they're not behavior.

use rg3d::{core::pool::Handle, scene::node::Node};

use crate::common::Input;

/// A client connected to a server. Can be observing, spectating or playing.
#[derive(Debug)]
pub(crate) struct Player {
    pub(crate) input: Input,
    pub(crate) ps: PlayerState,
    pub(crate) cycle_handle: Option<Handle<Cycle>>,
}

impl Player {
    pub(crate) fn new(cycle_handle: Option<Handle<Cycle>>) -> Self {
        Self {
            input: Input::default(),
            ps: PlayerState::Observing,
            cycle_handle,
        }
    }
}

/// How the player is participating in the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerState {
    /// The player is a freely floating camera observing the game.
    Observing,
    /// The player is watching another player's POV - LATER
    #[allow(dead_code)]
    Spectating { spectatee_handle: Handle<Player> },
    /// The player is playing
    Playing,
}

#[derive(Debug)]
pub(crate) struct Cycle {
    pub(crate) node_handle: Handle<Node>, // TODO needed?
    pub(crate) body_handle: Handle<Node>,
    pub(crate) player_handle: Handle<Player>,
}