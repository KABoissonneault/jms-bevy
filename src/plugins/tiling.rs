//! The hex map, loaded from a Tiled map and spawned as one entity per tile to
//! exercise grid and layer rendering, and registers the `HexStorage`
//! observers that make the spawned grid queryable. A reference occupant from
//! the tiling plan then stands on the topmost tile at one spot, and `QWEASD`
//! steps it one hex per press through the [`MoveEntity`] command. See
//! `doc/vault/jms/Tech Design/Tiling.md`.
//!
//! Maps are authored in Tiled as "Hexagonal (Staggered)" maps whose odd
//! columns are offset by half a hex, matching [`GridPos::from_offset`], with
//! square tiles in each tileset image. Two conventions bridge Tiled to the
//! game's grid:
//!
//! - Tiled has no elevation, so a tile layer's vertical offset stands in for
//!   the z layer it belongs to (see [`layer_z`]).
//! - Tiled's y axis points down; the map is flipped and centered onto the
//!   world origin (see [`tile_grid_pos`]).
//!
//! Only tile layers are spawned; object, image and group layers are ignored.

use bevy::{asset::AssetEvent, prelude::*};

use crate::prelude::*;
use crate::third_party::tiled::TiledMap;

/// The Tiled map spawned as the hex map.
const TEST_MAP_PATH: &str = "tiled/Test.tmx";

/// One tileset of the loaded map, its image sliced into tile cells.
struct TilesetAtlas {
    layout: Handle<TextureAtlasLayout>,
    /// Vertical offset from a tile's position (its hexagon center) to the
    /// center of its sprite, in pixels. The sprite is one square tile cell:
    /// the hexagon occupies the top `HEX_HEIGHT` pixels and the foundation
    /// the rest, so the cell center sits half the foundation below the
    /// hexagon center.
    sprite_y_offset: f32,
}

/// The map being shown: its asset, and what rendering its tiles needs.
#[derive(Resource)]
struct HexMap {
    /// Keeps the map asset loaded, and identifies it in asset events.
    source: Handle<TiledMap>,
    /// Atlas of each tileset of the map, in the same order.
    tilesets: Vec<TilesetAtlas>,
}

/// A tile of a Tiled map, placed on the grid.
struct MapTile {
    grid_pos: GridPos,
    /// The tile's tileset: its index among the map's tilesets, and so among
    /// [`HexMap::tilesets`] and the map asset's tileset images.
    tileset: usize,
    /// The tile's graphic, as an index in its tileset.
    floor: u32,
}

pub(crate) fn plugin(app: &mut App) {
    app.insert_resource(HexStorage::default())
        .add_observer(on_insert_tile)
        .add_observer(on_remove_tile)
        .add_systems(Startup, load_map)
        .add_systems(Update, spawn_map);
}

fn load_map(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(HexMap {
        source: assets.load(TEST_MAP_PATH),
        tilesets: Vec::new(),
    });
}

/// z layer of a Tiled tile layer with vertical offset `offset_y`: whole layer
/// steps only, with negative (up on screen, since Tiled's y points down)
/// rising in z. `None` when the offset falls between two layers.
fn layer_z(offset_y: f32) -> Option<i32> {
    let steps = -offset_y / HEX_Z_HEIGHT_OFFSET_PX;
    let steps = steps.round() as i32;
    (-offset_y == HEX_Z_HEIGHT_OFFSET_PX * steps as f32).then_some(steps)
}

/// Grid cell of the tile at Tiled `(x, y)` in a map `width` by `height`
/// tiles, on layer `z`.
///
/// Rows are flipped, since Tiled's y points down and world +y is screen-up.
/// Both axes are then shifted to center the map on the world origin. The
/// column shift is even: hex centers only exist where `col + drow` is even,
/// and an odd shift would move every tile off the lattice.
fn tile_grid_pos(x: i32, y: i32, z: i32, width: u32, height: u32) -> GridPos {
    let half_cols = (width as i32 / 2) & !1;
    let half_rows = height as i32 / 2;
    GridPos::from_offset(x - half_cols, half_rows - 1 - y, z)
}

/// Every tile of `map` that gets spawned, layer by layer, top-left first:
/// the tiles of each visible tile layer sitting on a whole z layer.
fn map_tiles(map: &tiled::Map) -> impl Iterator<Item = MapTile> + '_ {
    map.layers()
        .filter_map(move |layer| {
            let Some(z) = layer_z(layer.offset_y) else {
                warn!(
                    "Skipping Tiled layer {:?}: offsety {} px is not a multiple of {} px",
                    layer.name, layer.offset_y, HEX_Z_HEIGHT_OFFSET_PX
                );
                return None;
            };
            // Infinite maps are rejected when loading, so layers are finite.
            let tiled::TileLayer::Finite(tiles) = layer.as_tile_layer()? else {
                return None;
            };
            layer.visible.then(|| {
                (0..tiles.height()).flat_map(move |y| {
                    (0..tiles.width()).filter_map(move |x| {
                        let tile = tiles.get_tile(x as i32, y as i32)?;
                        Some(MapTile {
                            grid_pos: tile_grid_pos(x as i32, y as i32, z, map.width, map.height),
                            tileset: tile.tileset_index(),
                            floor: tile.id(),
                        })
                    })
                })
            })
        })
        .flatten()
}

/// Spawns the map's tiles once its asset reports loaded, and again whenever
/// the map (or a tileset it reads) changes on disk, replacing the previous
/// tiles. Scheduled on `Update` because the asset arrives a few frames after
/// `Startup`; the asset-event reader keeps every frame without a relevant
/// event a no-op.
fn spawn_map(
    mut commands: Commands,
    maps: Res<Assets<TiledMap>>,
    mut asset_events: MessageReader<AssetEvent<TiledMap>>,
    mut hex_map: ResMut<HexMap>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let source = hex_map.source.clone();
    let Some(tiled_map) = asset_events.read().find_map(|event| match event {
        // `LoadedWithDependencies`: the tileset images are decoded too.
        AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id }
            if *id == source.id() =>
        {
            maps.get(&source)
        }
        _ => None,
    }) else {
        return;
    };

    commands.despawn_all::<With<Tile>>();
    for tileset in &hex_map.tilesets {
        layouts.remove(tileset.layout.id());
    }

    let map = tiled_map.map();
    hex_map.tilesets = map
        .tilesets()
        .iter()
        .map(|tileset| {
            let columns = tileset.columns.max(1);
            let rows = tileset.tilecount.div_ceil(columns).max(1);
            TilesetAtlas {
                layout: layouts.add(TextureAtlasLayout::from_grid(
                    UVec2::new(tileset.tile_width, tileset.tile_height),
                    columns,
                    rows,
                    Some(UVec2::splat(tileset.spacing)),
                    Some(UVec2::splat(tileset.margin)),
                )),
                sprite_y_offset: -((tileset.tile_height as f32 - HEX_HEIGHT_PX) / 2.0),
            }
        })
        .collect();

    for tile in map_tiles(map) {
        let atlas = &hex_map.tilesets[tile.tileset];
        let game_pos = GamePos::from(tile.grid_pos);
        let px = pixel_delta(game_pos);
        commands.spawn((
            Name::new("Tile"),
            tile.grid_pos,
            Tile,
            TileIndex { floor: tile.floor },
            Sprite {
                image: tiled_map.tileset_image(tile.tileset).clone(),
                texture_atlas: Some(TextureAtlas {
                    layout: atlas.layout.clone(),
                    index: tile.floor as usize,
                }),
                ..default()
            },
            Transform::from_xyz(
                px.x,
                px.y + atlas.sprite_y_offset,
                get_coord_depth(game_pos),
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use bevy::image::{CompressedImageFormats, ImageLoader};

    /// Map sizes covering even and odd widths and heights.
    const MAP_SIZES: [(u32, u32); 4] = [(30, 20), (31, 20), (30, 21), (1, 1)];

    #[test]
    fn tiled_tiles_land_on_hex_centers_whatever_the_map_size() {
        for (width, height) in MAP_SIZES {
            for y in 0..height as i32 {
                for x in 0..width as i32 {
                    assert!(
                        GamePos::from(tile_grid_pos(x, y, 0, width, height)).is_grid_pos(),
                        "tile ({x}, {y}) of a {width}x{height} map is not on a hex center"
                    );
                }
            }
        }
    }

    #[test]
    fn tiled_tiles_keep_their_neighbors() {
        let (width, height) = MAP_SIZES[0];
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                let tile = tile_grid_pos(x, y, 0, width, height);
                let is_neighbor = |other: GridPos| tile.neighbors().any(|(_, n)| n == other);
                // The Tiled tiles to the right and below are hex neighbors.
                if x + 1 < width as i32 {
                    assert!(is_neighbor(tile_grid_pos(x + 1, y, 0, width, height)));
                }
                if y + 1 < height as i32 {
                    assert!(is_neighbor(tile_grid_pos(x, y + 1, 0, width, height)));
                }
            }
        }
    }

    #[test]
    fn tiled_top_rows_stand_further_north() {
        let (width, height) = MAP_SIZES[0];
        // Tiled's y points down, so its first row is the top of the map.
        assert!(
            GamePos::from(tile_grid_pos(0, 0, 0, width, height)).y
                > GamePos::from(tile_grid_pos(0, height as i32 - 1, 0, width, height)).y
        );
    }

    #[test]
    fn layers_rise_with_negative_offsets() {
        assert_eq!(layer_z(0.0), Some(0));
        assert_eq!(layer_z(-HEX_Z_HEIGHT_OFFSET_PX), Some(1));
        assert_eq!(layer_z(-2.0 * HEX_Z_HEIGHT_OFFSET_PX), Some(2));
        assert_eq!(layer_z(HEX_Z_HEIGHT_OFFSET_PX), Some(-1));
        // Between two layers: not a layer of the grid.
        assert_eq!(layer_z(-HEX_Z_HEIGHT_OFFSET_PX / 2.0), None);
    }

    /// The tiles `TEST_MAP_PATH` should put on the grid, once it is loaded.
    fn expected_tiles(world: &World) -> Option<Vec<MapTile>> {
        let source = &world.resource::<HexMap>().source;
        world
            .resource::<Assets<TiledMap>>()
            .get(source)
            .map(|map| map_tiles(map.map()).collect())
    }

    /// Runs the app headless, loading and spawning the map, and hands it to
    /// `check` once `done` holds.
    fn run_until(done: impl Fn(&World) -> bool, check: impl FnOnce(&World)) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        // What the renderer's texture plugin sets up, minus the renderer.
        app.init_asset::<Image>()
            .register_asset_loader(ImageLoader::new(CompressedImageFormats::default()));
        crate::third_party::tiled::plugin(&mut app);
        // Normally registered by the sprite plugin, which needs a renderer.
        app.init_asset::<TextureAtlasLayout>();
        plugin(&mut app);

        // Frames are near-instant headless, so wait on the clock while the
        // asset server's threads do the actual loading.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            app.update();
            if done(app.world()) {
                check(app.world());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!(
            "{TEST_MAP_PATH} did not fully spawn: {} tiles on the grid",
            app.world().resource::<HexStorage>().tile_count()
        );
    }

    #[test]
    fn spawns_every_tiled_tile_onto_the_grid() {
        run_until(
            |world| {
                expected_tiles(world)
                    .is_some_and(|tiles| world.resource::<HexStorage>().tile_count() == tiles.len())
            },
            |world| {
                let storage = world.resource::<HexStorage>();
                let tiles = expected_tiles(world).unwrap();
                let layers: HashSet<i32> = tiles.iter().map(|tile| tile.grid_pos.layer).collect();
                assert_eq!(storage.layer_count(), layers.len());

                for tile in tiles {
                    let Some(entity) = storage.tile_at(tile.grid_pos) else {
                        panic!("no tile at {:?}", tile.grid_pos);
                    };
                    assert_eq!(
                        world
                            .entity(entity)
                            .get::<TileIndex>()
                            .map(|index| index.floor),
                        Some(tile.floor)
                    );
                }
            },
        );
    }
}
