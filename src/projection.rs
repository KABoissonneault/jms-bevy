#![allow(unused)]

use crate::prelude::*;
use bevy::prelude::*;

/// Pixel length of one HexCoord dx
pub const DX_PX: f32 = 8.0;
/// Pixel length of one HexCoord dy
pub const DY_PX: f32 = 5.0;

/// Horizontal pixel length of one hex tile.
pub const HEX_WIDTH_PX: f32 = 4.0 * DX_PX;
/// Vertical pixel length of one hex tile.
pub const HEX_HEIGHT_PX: f32 = 4.0 * DY_PX;
/// Vertical pixel offset for one z-layer
pub const HEX_Z_HEIGHT_OFFSET_PX: f32 = 11.0;

/// Pixel position of this coordinate in Bevy world space, with x
/// increasing to the right, y increasing upward (matching the
/// convention of `Transform`), and each z layer rising
/// [`HEX_Z_OFFSET_PX`] pixels.
pub fn pixel_delta(coord: GamePos) -> Vec2 {
    Vec2::new(
        DX_PX * coord.x as f32,
        DY_PX * coord.y as f32 + HEX_Z_HEIGHT_OFFSET_PX * coord.z as f32,
    )
}

/// How much hex layers affect render depth
const HEX_Z_LAYER_DEPTH: f32 = 1.0;

/// Tiles are ordered within a layer so that tiles lower on the screen draw
/// over tiles above them, making a tile's top pixels draw over the bottom
/// pixels of the tile above it, as required by `Tiling.md` to keep
/// overlapping sprites deterministic. Grid y maps to screen-up, so the
/// order term is negated in the transform below.
const HEX_Y_DEPTH: f32 = 0.001;

pub fn get_coord_depth(coord: GamePos) -> f32 {
    HEX_Z_LAYER_DEPTH * coord.z as f32 - HEX_Y_DEPTH * coord.y as f32
}

#[test]
fn pixel_pos_scales_by_pixel_units() {
    assert_eq!(pixel_delta(GamePos::new(0, 0, 0)), Vec2::ZERO);
    assert_eq!(pixel_delta(GamePos::new(1, 0, 0)), Vec2::new(DX_PX, 0.0));
    assert_eq!(
        pixel_delta(GamePos::new(4, 0, 0)),
        Vec2::new(HEX_WIDTH_PX, 0.0)
    );
    assert_eq!(
        pixel_delta(GamePos::new(0, 4, 0)),
        Vec2::new(0.0, HEX_HEIGHT_PX)
    );
    assert_eq!(
        pixel_delta(GamePos::new(0, 0, 1)),
        Vec2::new(0.0, HEX_Z_HEIGHT_OFFSET_PX)
    );
}
