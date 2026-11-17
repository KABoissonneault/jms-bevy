//! The hex map. For now this only generates a random test map — a full base
//! layer with holes, plus sparse layers above and below — to exercise grid
//! and layer rendering. See `doc/vault/jms/Tech Design/Tiling.md`.

use bevy::prelude::*;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::components::{COLUMN_DY, HexPos, ROW_DY, Tile};

/// Side length of one tile cell in the floor tileset.
const TILE_CELL: u32 = 32;

/// Width of a hexagon (point-to-point), spanning 4 dx.
const HEX_WIDTH_PX: f32 = 32.0;
/// Height of a hexagon (flat-to-flat), spanning 4 dy.
const HEX_HEIGHT_PX: f32 = 20.0;
/// The dx unit: smallest x delta, a quarter of [`HEX_WIDTH_PX`].
const DX_PX: f32 = HEX_WIDTH_PX / 4.0;
/// The dy unit: smallest y delta, a quarter of [`HEX_HEIGHT_PX`].
const DY_PX: f32 = HEX_HEIGHT_PX / 4.0;
/// Pixel distance between hex grid layers (`S`).
const LAYER_STEP_Z_PX: f32 = 11.0;

/// Pixel position of this coordinate in Bevy world space, with x
/// increasing to the right, y increasing upward (matching the
/// convention of `Transform`), and each z layer rising
/// [`LAYER_STEP_Z`] pixels.
pub fn pixel_delta(coord: HexPos) -> Vec2 {
    Vec2::new(
        DX_PX * coord.x as f32,
        DY_PX * coord.y as f32 + LAYER_STEP_Z_PX * coord.z as f32,
    )
}

#[test]
fn pixel_pos_scales_by_pixel_units() {
    assert_eq!(pixel_delta(HexPos::new(0, 0, 0)), Vec2::ZERO);
    // One x unit is one dx to the right.
    assert_eq!(pixel_delta(HexPos::new(1, 0, 0)), Vec2::new(DX_PX, 0.0));
    // A full hexagon spans 4 dy.
    assert_eq!(
        pixel_delta(HexPos::new(0, 4, 0)),
        Vec2::new(0.0, HEX_HEIGHT_PX)
    );
    // Higher z layers rise LAYER_STEP_Z pixels.
    assert_eq!(
        pixel_delta(HexPos::new(0, 0, 1)),
        Vec2::new(0.0, LAYER_STEP_Z_PX)
    );
}

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
/// Render-z gap between adjacent hex layers. Must stay larger than a
/// layer's whole y-order range so tiles of different layers never
/// interleave; asserted below.
const LAYER_ORDER_STEP: f32 = 1.0;
/// Tiles are ordered within a layer so that tiles lower on the screen draw
/// over tiles above them, making a tile's top pixels draw over the bottom
/// pixels of the tile above it, as required by `Tiling.md` to keep
/// overlapping sprites deterministic. Grid y maps to screen-up, so the
/// order term is negated in the transform below.
const TILE_ORDER_Y_SCALE: f32 = 0.001;

/// Total y-order range of one layer across the whole test map.
const TILE_ORDER_Y_RANGE: f32 = TILE_ORDER_Y_SCALE
    * DY_PX
    * (ROW_DY * (TEST_MAP_ROWS - 1) + COLUMN_DY * ((TEST_MAP_COLS - 1) & 1)) as f32;
const _: () = assert!(TILE_ORDER_Y_RANGE < LAYER_ORDER_STEP);

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
    let corner = pixel_delta(HexPos::from_grid(TEST_MAP_COLS - 1, TEST_MAP_ROWS - 1));
    let center = corner / 2.0;

    for &layer in &TEST_MAP_LAYERS {
        let appear_chance = if layer == BASE_LAYER {
            1.0 - TEST_MAP_GAP_CHANCE
        } else {
            TEST_MAP_EXTRA_LAYER_CHANCE
        };
        for row in 0..TEST_MAP_ROWS {
            for col in 0..TEST_MAP_COLS {
                if !rng.random_bool(appear_chance) {
                    continue;
                }
                let pos = HexPos::from_grid(col, row).at_layer(layer);
                let px = pixel_delta(pos);
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
                        px.y - center.y + FLOOR_SPRITE_Y_OFFSET,
                        pos.z as f32 * LAYER_ORDER_STEP - pos.y as f32 * TILE_ORDER_Y_SCALE,
                    ),
                ));
            }
        }
    }
}
