#![allow(unused)]

use crate::prelude::*;
use bevy::prelude::*;

// Hex geometry. See `doc/vault/jms/Tech Design/Tiling.md`.

/// x delta between columns
const COLUMN_DX: i32 = 3;
/// y delta between rows
const ROW_DY: i32 = 4;
/// y offset for odd columns
const ODD_COLUMN_DY: i32 = 2;
/// y delta between drow values
const DROW_DY: i32 = 2;

dev_reflect_component! {
    /// Position of a gameplay entity on the hex world, in the game coordinates
    /// of `Tiling.md`: (x, y) are in quarter-hex units (dx and dy) on the
    /// horizontal slice of the world, and z is the hex grid layer index.
    /// Hexagon centers and vertices are always aligned to an even y.
    #[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GamePos {
        pub x: i32,
        pub y: i32,
        pub z: i32,
    }
}

impl GamePos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Game position of the tile grid cell `(col, row)` on layer 0.
    pub const fn from(grid_pos: GridPos) -> Self {
        Self::new(
            COLUMN_DX * grid_pos.col,
            DROW_DY * grid_pos.drow,
            grid_pos.layer,
        )
    }

    pub const fn from_grid_offset(col: i32, row: i32, layer: i32) -> Self {
        Self::from(GridPos::from_offset(col, row, layer))
    }

    /// Returns this position moved to the hex grid layer `z`.
    pub const fn at_layer(self, z: i32) -> Self {
        Self::new(self.x, self.y, z)
    }

    /// Returns whether the position could represent a tile center on the grid
    pub const fn is_grid_pos(self) -> bool {
        (2 * self.x + 3 * self.y).rem_euclid(12) == 0
    }

    /// Returns the equivalent GridPos if valid, none otherwise
    pub const fn to_grid(self) -> Option<GridPos> {
        if !self.is_grid_pos() {
            return None;
        }
        let col = self.x / COLUMN_DX;
        let drow = self.y / DROW_DY;
        Some(GridPos::new(col, drow, self.z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_positions_are_tiles() {
        // Every grid cell is a canonical center.
        for col in -4..=4 {
            for row in -4..=4 {
                let center = GamePos::from_grid_offset(col, row, 0);
                assert!(center.is_grid_pos());
            }
        }

        // Between the sites lie positions that are neither centers nor
        // vertices — e.g. (0, 2), halfway up the edge shared by vertically
        // adjacent hexes — and they must not pass as canonical.
        for pos in [
            GamePos::new(0, 2, 0),
            GamePos::new(3, 0, 0),
            GamePos::new(2, 2, 0),
            GamePos::new(1, 0, 0),
            GamePos::new(-1, 0, 0),
            GamePos::new(-2, 2, 0),
            GamePos::new(1, 1, 0),
        ] {
            assert!(!pos.is_grid_pos());
        }
        assert!(GamePos::new(-COLUMN_DX, ODD_COLUMN_DY, 0).is_grid_pos());
    }

    #[test]
    fn grid_steps_match_hex_neighbor_offsets() {
        // Adjacent columns are COLUMN_DX x COLUMN_DY apart, adjacent rows
        // ROW_DY apart, so every grid step lands on a neighboring hexagon.
        assert_eq!(
            GamePos::from_grid_offset(1, 0, 0),
            GamePos::new(COLUMN_DX, -ODD_COLUMN_DY, 0)
        );
        assert_eq!(
            GamePos::from_grid_offset(0, 1, 0),
            GamePos::new(0, ROW_DY, 0)
        );
        // The convention holds on the whole global grid: negative odd
        // columns (`& 1` is never negative) are staggered down as well.
        assert_eq!(
            GamePos::from_grid_offset(-1, 0, 0),
            GamePos::new(-COLUMN_DX, -ODD_COLUMN_DY, 0)
        );
        // Odd columns sit COLUMN_DY below the row line, like in Tiled.
        assert_eq!(
            GamePos::from_grid_offset(2, 0, 0),
            GamePos::new(2 * COLUMN_DX, 0, 0)
        );
        assert_eq!(
            GamePos::from_grid_offset(2, 1, 0),
            GamePos::new(2 * COLUMN_DX, ROW_DY, 0)
        );
    }

    #[test]
    fn at_layer_moves_between_layers() {
        assert_eq!(GamePos::new(1, 2, 0).at_layer(-1), GamePos::new(1, 2, -1));
    }

    #[test]
    fn to_grid_inverts_from_grid() {
        for col in -4..=4 {
            for row in -4..=4 {
                assert_eq!(
                    GamePos::from_grid_offset(col, row, 0).to_grid(),
                    Some(GridPos::from_offset(col, row, 0)),
                    "cell ({col}, {row})"
                );
            }
        }
        // Vertices and non-canonical positions have no grid cell.
        assert_eq!(GamePos::new(1, 2, 0).to_grid(), None);
        assert_eq!(GamePos::new(2, 0, 0).to_grid(), None);
        assert_eq!(GamePos::new(0, 2, 0).to_grid(), None);
        assert_eq!(GamePos::new(1, 1, 7).to_grid(), None);
    }
}
