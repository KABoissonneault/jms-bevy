#![allow(unused)]

use crate::prelude::*;
use bevy::prelude::*;

/// Tile on the hex grid
#[derive(Component, Default)]
#[require(GridPos, TileIndex, TileState)]
pub struct Tile;

#[derive(Component, Default, Debug, Clone, Copy)]
pub struct TileIndex {
    /// Index of the floor graphic in the floor tileset.
    pub floor: u32,
}

/// Occupancy state of a whole tile (`Tiling.md` §Collision). Fresh tiles
/// default to [`TileState::Free`]; only exceptional tiles are given
/// another state explicitly.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TileState {
    /// The tile cannot be occupied or overlapped (ex: solid rock).
    Blocked(Entity),
    /// The tile cannot be occupied, but can be partially accessed
    /// (ex: the face of a counter).
    Obstructed,
    /// The tile can be occupied or overlapped.
    #[default]
    Free,
}
