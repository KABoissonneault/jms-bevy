//! The hex map. For now this only generates a random test map with holes, to
//! exercise the grid rendering. See `doc/vault/jms/Tech Design/Tiling.md`.

use bevy::prelude::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::components::{HEX_HEIGHT, HexPos, TILE_CELL, Tile};

/// Floor tileset in `assets/`. Each `TILE_CELL`-pixel square cell of the
/// image holds one floor graphic; tile indices traverse from the top-left,
/// left to right, then continue on the next row down.
const FLOOR_TILESET_PATH: &str = "sprites/tilemaps/test_tile.png";

/// Size of the generated test map. Must be at least 20 tiles in each
/// direction to be a useful test of the grid.
const TEST_MAP_COLS: i32 = 24;
const TEST_MAP_ROWS: i32 = 24;
/// Chance that a test map tile is skipped, leaving a hole in the grid.
const TEST_MAP_GAP_CHANCE: f64 = 0.15;
/// Fixed seed, so the generated test map is stable across runs. Spells "JMS".
const TEST_MAP_SEED: u64 = 0x4A4D53;
/// Number of distinct floor graphics to pick from (tile indices 0..=3).
const FLOOR_SPRITE_COUNT: u32 = 4;

/// Vertical offset from a tile's position (its hexagon center) to the center
/// of its sprite, in pixels. The sprite is one `TILE_CELL` square: the
/// hexagon occupies the top `HEX_HEIGHT` pixels and the foundation the rest,
/// so the cell center sits half the foundation below the hexagon center.
const FLOOR_SPRITE_Y_OFFSET: f32 = -((TILE_CELL as f32 - HEX_HEIGHT) / 2.0);
/// Base z of the map layer, above other 2d content such as the title text.
const TILE_Z: f32 = 1.0;
/// Tiles are ordered within the layer by screen y, so a tile's top pixels
/// draw over the bottom pixels of the tile above it, as required by
/// `Tiling.md` to keep overlapping sprites deterministic.
const TILE_ORDER_Y_SCALE: f32 = 0.001;

/// The floor tileset, sliced into `TILE_CELL`-pixel cells once the image has
/// loaded.
#[derive(Resource)]
struct FloorTileset {
    image: Handle<Image>,
    layout: Option<Handle<TextureAtlasLayout>>,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_tileset)
        .add_systems(Update, spawn_test_map);
}

fn load_tileset(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(FloorTileset {
        image: assets.load(FLOOR_TILESET_PATH),
        layout: None,
    });
}

/// Waits for the tileset image to load, then slices it into `TILE_CELL`
/// cells and spawns the test map.
fn spawn_test_map(
    mut commands: Commands,
    mut tileset: ResMut<FloorTileset>,
    images: Res<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    if tileset.layout.is_some() {
        return;
    }
    let Some(image) = images.get(&tileset.image) else {
        return; // Still loading.
    };

    // Slice the loaded image into tile cells based on its actual dimensions.
    let columns = (image.size().x / TILE_CELL).max(1);
    let rows = (image.size().y / TILE_CELL).max(1);
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_CELL),
        columns,
        rows,
        None,
        None,
    ));
    tileset.layout = Some(layout.clone());

    let sprite_count = FLOOR_SPRITE_COUNT.min(columns * rows);
    let mut rng = StdRng::seed_from_u64(TEST_MAP_SEED);

    // The map spans from the origin tile to the opposite corner; center
    // that extent on the camera.
    let corner = HexPos::from_grid(TEST_MAP_COLS - 1, TEST_MAP_ROWS - 1).pixel_pos();
    let center = corner / 2.0;

    for row in 0..TEST_MAP_ROWS {
        for col in 0..TEST_MAP_COLS {
            if rng.random_bool(TEST_MAP_GAP_CHANCE) {
                continue;
            }
            let pos = HexPos::from_grid(col, row);
            let px = pos.pixel_pos();
            let floor = rng.random_range(0..sprite_count);
            commands.spawn((
                Name::new("Tile"),
                pos,
                Tile { floor },
                Sprite {
                    image: tileset.image.clone(),
                    texture_atlas: Some(TextureAtlas {
                        layout: layout.clone(),
                        index: floor as usize,
                    }),
                    ..default()
                },
                Transform::from_xyz(
                    px.x - center.x,
                    center.y - px.y + FLOOR_SPRITE_Y_OFFSET,
                    TILE_Z + px.y * TILE_ORDER_Y_SCALE,
                ),
            ));
        }
    }
}
