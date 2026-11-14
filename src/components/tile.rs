#![allow(unused)]

use bevy::prelude::*;

// Hex geometry, in pixels. See `doc/vault/jms/Tech Design/Tiling.md`.

/// Side length of one tile cell in the floor tileset.
pub const TILE_CELL: u32 = 32;

/// Width of a hexagon (point-to-point), spanning 4 dx.
pub const HEX_WIDTH: f32 = 32.0;
/// Height of a hexagon (flat-to-flat), spanning 4 dy.
pub const HEX_HEIGHT: f32 = 20.0;
/// The dx unit: smallest x delta, a quarter of [`HEX_WIDTH`].
pub const DX_PX: f32 = HEX_WIDTH / 4.0;
/// The dy unit: smallest y delta, a quarter of [`HEX_HEIGHT`].
pub const DY_PX: f32 = HEX_HEIGHT / 4.0;
/// Pixel distance between hex grid layers (`S`).
pub const LAYER_STEP_Z: f32 = 11.0;

// Steps between neighboring grid positions, in dx/dy units.

/// x delta between adjacent tile columns: `3 dx`.
pub const COLUMN_DX: i32 = 3;
/// y stagger between adjacent tile columns: `2 dy`.
pub const COLUMN_DY: i32 = 2;
/// y delta between adjacent tile rows: `4 dy`.
pub const ROW_DY: i32 = 4;

/// Position of a gameplay entity on the hex world, in the game coordinates
/// of `Tiling.md`: (x, y) are in quarter-hex units (dx and dy) on the
/// horizontal slice of the world, and z is the hex grid layer index.
/// Hexagon centers and vertices are always aligned to an even y.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl HexPos {
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Global position of the tile grid cell `(col, row)` on layer 0.
    ///
    /// Adjacent columns are [`COLUMN_DX`] apart and staggered [`COLUMN_DY`],
    /// rows are [`ROW_DY`] apart. This keeps every tile on a hex neighbor
    /// offset: (`COLUMN_DX`, `COLUMN_DY`) to the next column, `ROW_DY`
    /// straight to the next row.
    pub const fn from_grid(col: i32, row: i32) -> Self {
        Self::new(COLUMN_DX * col, ROW_DY * row + COLUMN_DY * (col & 1), 0)
    }

    /// Returns this position moved to the hex grid layer `z`.
    pub const fn at_layer(self, z: i32) -> Self {
        Self::new(self.x, self.y, z)
    }

    /// Pixel position of this coordinate in the world, with x increasing to
    /// the right, y increasing downward, and each z layer rising
    /// [`LAYER_STEP_Z`] pixels.
    pub fn pixel_pos(self) -> Vec2 {
        Vec2::new(
            DX_PX * self.x as f32,
            DY_PX * self.y as f32 - LAYER_STEP_Z * self.z as f32,
        )
    }
}

/// A tile occupying the hexagon at its [`HexPos`].
#[derive(Component, Debug, Clone, Copy)]
pub struct Tile {
    /// Index of the floor graphic in the floor tileset.
    pub floor: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_steps_match_hex_neighbor_offsets() {
        // Adjacent columns are COLUMN_DX x COLUMN_DY apart, adjacent rows
        // ROW_DY apart, so every grid step lands on a neighboring hexagon.
        assert_eq!(
            HexPos::from_grid(1, 0),
            HexPos::new(COLUMN_DX, COLUMN_DY, 0)
        );
        assert_eq!(HexPos::from_grid(0, 1), HexPos::new(0, ROW_DY, 0));
        // The convention holds on the whole global grid: negative odd
        // columns (`& 1` is never negative) are staggered down as well.
        assert_eq!(
            HexPos::from_grid(-1, 0),
            HexPos::new(-COLUMN_DX, COLUMN_DY, 0)
        );
        // Odd columns sit COLUMN_DY below the row line.
        assert_eq!(HexPos::from_grid(2, 0), HexPos::new(2 * COLUMN_DX, 0, 0));
        assert_eq!(
            HexPos::from_grid(2, 1),
            HexPos::new(2 * COLUMN_DX, ROW_DY, 0)
        );
    }

    #[test]
    fn at_layer_moves_between_layers() {
        assert_eq!(HexPos::new(1, 2, 0).at_layer(-1), HexPos::new(1, 2, -1));
    }

    #[test]
    fn pixel_pos_scales_by_pixel_units() {
        assert_eq!(HexPos::new(0, 0, 0).pixel_pos(), Vec2::ZERO);
        // One x unit is one dx to the right.
        assert_eq!(HexPos::new(1, 0, 0).pixel_pos(), Vec2::new(DX_PX, 0.0));
        // A full hexagon spans 4 dy.
        assert_eq!(HexPos::new(0, 4, 0).pixel_pos(), Vec2::new(0.0, HEX_HEIGHT));
        // Higher z layers rise LAYER_STEP_Z pixels.
        assert_eq!(
            HexPos::new(0, 0, 1).pixel_pos(),
            Vec2::new(0.0, -LAYER_STEP_Z)
        );
    }
}
