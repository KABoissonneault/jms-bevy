//! The hex map. For now this only generates a random test map — a full base
//! layer with holes, plus sparse layers above and below — to exercise grid
//! and layer rendering, and registers the `HexStorage` observers that make
//! the spawned grid queryable. A reference occupant from the tiling plan
//! then stands on the topmost tile at one spot, and `QWEASD` steps it one
//! hex per press through the [`MoveEntity`] command. See
//! `doc/vault/jms/Tech Design/Tiling.md`.

use bevy::asset::AssetEvent;
use bevy::prelude::*;

use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::prelude::*;

/// Side length of one tile cell in the floor tileset.
const TILE_CELL: u32 = 32;

/// Floor tileset in `assets/`. Each `TILE_CELL`-pixel square cell of the
/// image holds one floor graphic; tile indices traverse from the top-left,
/// left to right, then continue on the next row down.
const FLOOR_TILESET_PATH: &str = "sprites/tilemaps/test_tile.png";

/// Size of the generated test map. Must be at least 20 tiles in each
/// direction to be a useful test of the grid.
const TEST_MAP_COLS: i32 = 24;
const TEST_MAP_ROWS: i32 = 24;
/// z layers generated for the test map. The base layer comes first so its
/// RNG draws — and therefore its layout — stay stable regardless of how the
/// extra layers are configured.
const TEST_MAP_LAYERS: [i32; 3] = [0, -1, 1];
/// The layer holding the full test map.
const BASE_LAYER: i32 = 0;
/// Chance that a base layer tile is skipped, leaving a hole in the grid.
const TEST_MAP_GAP_CHANCE: f64 = 0.15;
/// Chance per tile for each extra layer to have a tile, scattering sparse
/// floors above and below the base layer.
const TEST_MAP_EXTRA_LAYER_CHANCE: f64 = 0.10;
/// Fixed seed, so the generated test map is stable across runs. Spells "JMS".
const TEST_MAP_SEED: u64 = 0x4A4D53;
/// Number of distinct floor graphics to pick from (tile indices 0..=3).
const FLOOR_SPRITE_COUNT: u32 = 4;

/// Vertical offset from a tile's position (its hexagon center) to the center
/// of its sprite, in pixels. The sprite is one `TILE_CELL` square: the
/// hexagon occupies the top `HEX_HEIGHT` pixels and the foundation the rest,
/// so the cell center sits half the foundation below the hexagon center.
const FLOOR_SPRITE_Y_OFFSET: f32 = -((TILE_CELL as f32 - HEX_HEIGHT_PX) / 2.0);

/// The floor tileset, sliced into `TILE_CELL`-pixel cells once the image has
/// loaded.
#[derive(Resource)]
struct FloorTileset {
    image: Handle<Image>,
    layout: Option<Handle<TextureAtlasLayout>>,
}

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(HexStorage::default())
        .add_observer(on_insert_tile)
        .add_observer(on_remove_tile)
        .add_systems(Startup, load_tileset)
        .add_systems(Update, spawn_test_map);
}

fn load_tileset(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(FloorTileset {
        image: assets.load(FLOOR_TILESET_PATH),
        layout: None,
    });
}

/// Slices the tileset image into `TILE_CELL` cells and spawns the test map
/// on the frame the image reports its load complete. Scheduled on `Update`
/// because the asset arrives a few frames after `Startup` — the asset-event
/// reader keeps every earlier frame a no-op, and the `layout` guard below
/// makes every later frame one too.
fn spawn_test_map(
    mut commands: Commands,
    mut asset_events: MessageReader<AssetEvent<Image>>,
    mut tileset: ResMut<FloorTileset>,
    images: Res<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    if tileset.layout.is_some() {
        return;
    }
    let loaded = asset_events.read().any(|event| {
        matches!(event, AssetEvent::LoadedWithDependencies { id } if *id == tileset.image.id())
    });
    if !loaded {
        return; // Still loading.
    }
    let Some(image) = images.get(&tileset.image) else {
        return;
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

    for &layer in &TEST_MAP_LAYERS {
        let appear_chance = if layer == BASE_LAYER {
            1.0 - TEST_MAP_GAP_CHANCE
        } else {
            TEST_MAP_EXTRA_LAYER_CHANCE
        };
        for row in -TEST_MAP_ROWS / 2..TEST_MAP_ROWS / 2 {
            for col in -TEST_MAP_COLS / 2..TEST_MAP_COLS / 2 {
                if !rng.random_bool(appear_chance) {
                    continue;
                }
                let grid_pos = GridPos::from_offset(col, row, layer);
                let game_pos = GamePos::from(grid_pos);
                let px = pixel_delta(game_pos);
                let floor = rng.random_range(0..sprite_count);
                commands.spawn((
                    Name::new("Tile"),
                    grid_pos,
                    Tile,
                    TileIndex { floor },
                    Sprite {
                        image: tileset.image.clone(),
                        texture_atlas: Some(TextureAtlas {
                            layout: layout.clone(),
                            index: floor as usize,
                        }),
                        ..default()
                    },
                    Transform::from_xyz(
                        px.x,
                        px.y + FLOOR_SPRITE_Y_OFFSET,
                        get_coord_depth(game_pos),
                    ),
                ));
            }
        }
    }
}
