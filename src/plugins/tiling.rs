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
//! - Tiled's y axis points down; rows are flipped and the map is offset so
//!   its middle tile is the world origin (see [`tile_grid_pos`]).
//!
//! Tile layers spawn one tile entity each, and object layers spawn one entity
//! per point object sitting on a hex center, named as in Tiled. Image and
//! group layers, and objects of other shapes, are ignored.

use bevy::{asset::AssetEvent, prelude::*};

use crate::prelude::*;
use crate::third_party::tiled::TiledMap;

/// The Tiled map spawned as the hex map.
const TEST_MAP_PATH: &str = "tiled/Test.tmx";

/// How far a Tiled point may sit from a hex center and still snap to it.
const POINT_SNAP_PX: f32 = 1.0;

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

/// A named point object of a Tiled map, placed on the grid.
struct MapPoint {
    name: String,
    game_pos: GamePos,
}

/// Marks everything spawned from the map asset, so a reload can replace it
/// all without touching entities the game spawned itself (the player).
#[derive(Component, Default)]
struct MapSpawned;

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
/// Rows are flipped, since Tiled's y points down and world +y is screen-up,
/// and the map is offset so its middle tile is the world origin. When the
/// middle falls between two tiles (even width or height), the left column
/// and the bottom row are the origin. The column origin then stays on an
/// even column: the stagger has to keep matching Tiled's (odd columns
/// shifted down), which an odd shift would flip.
fn tile_grid_pos(x: i32, y: i32, z: i32, width: u32, height: u32) -> GridPos {
    let half_cols = (width as i32 / 2) & !1;
    GridPos::from_offset(x - half_cols, height as i32 / 2 - y, z)
}

/// Game position of a Tiled map point at pixel `(x, y)` on z layer `z`, when
/// it sits on a hex center (within [`POINT_SNAP_PX`]).
///
/// Tile centers of the 32x20 tiles of `Tiling.md` sit `2*DX_PX +
/// 3*DX_PX*column` px right and `2*DY_PX + 4*DY_PX*row + 2*DY_PX` px down for
/// odd columns, from the map's top-left.
fn point_game_pos(x: f32, y: f32, z: i32, width: u32, height: u32) -> Option<GamePos> {
    let column = snap(x, 2.0 * DX_PX, 3.0 * DX_PX)?;
    let row = snap(y, 2.0 * DY_PX * (1 + (column & 1)) as f32, 4.0 * DY_PX)?;
    Some(GamePos::from(tile_grid_pos(column, row, z, width, height)))
}

/// The tile index at `value` px along one axis, when it sits on a center at
/// `offset + index * spacing` within [`POINT_SNAP_PX`] pixels.
fn snap(value: f32, offset: f32, spacing: f32) -> Option<i32> {
    let index = ((value - offset) / spacing).round();
    ((value - offset - index * spacing).abs() <= POINT_SNAP_PX).then_some(index as i32)
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

/// Every point object of `map`'s visible object layers that sits on a hex
/// center, named as in Tiled.
fn map_points(map: &tiled::Map) -> impl Iterator<Item = MapPoint> + '_ {
    map.layers()
        .filter_map(move |layer| {
            let Some(z) = layer_z(layer.offset_y) else {
                warn!(
                    "Skipping Tiled layer {:?}: offsety {} px is not a multiple of {} px",
                    layer.name, layer.offset_y, HEX_Z_HEIGHT_OFFSET_PX
                );
                return None;
            };
            if layer.offset_x != 0.0 {
                warn!(
                    "Skipping Tiled layer {:?}: an offsetx of {} px cannot land on the hex lattice",
                    layer.name, layer.offset_x
                );
                return None;
            }
            let objects = layer.as_object_layer()?;
            layer.visible.then(|| {
                objects.objects().filter_map(move |object| {
                    if !matches!(object.shape, tiled::ObjectShape::Point(..)) {
                        warn!(
                            "Skipping Tiled object {:?}: only point objects are supported",
                            object.name
                        );
                        return None;
                    }
                    let Some(game_pos) =
                        point_game_pos(object.x, object.y, z, map.width, map.height)
                    else {
                        warn!(
                            "Skipping Tiled point {:?}: not aligned with a hex center",
                            object.name
                        );
                        return None;
                    };
                    Some(MapPoint {
                        name: object.name.clone(),
                        game_pos,
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

    commands.despawn_all::<With<MapSpawned>>();
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
            MapSpawned,
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

    for point in map_points(map) {
        commands.spawn((Name::new(point.name), point.game_pos, MapSpawned));
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
    fn tiled_tiles_land_on_hex_centers() {
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
    fn the_middle_tile_is_the_world_origin() {
        // For the even-sized test map, the origin is the left column and the
        // bottom row of the middle: tile 14 of 0..29, row 10 of 0..19.
        assert_eq!(
            GamePos::from(tile_grid_pos(14, 10, 0, 30, 20)),
            GamePos::new(0, 0, 0)
        );
        // The tile right of it steps one column right and, being an odd
        // column in Tiled, half a tile down.
        assert_eq!(
            GamePos::from(tile_grid_pos(15, 10, 0, 30, 20)),
            GamePos::new(3, -2, 0)
        );
        // Odd widths have a single middle column, but the column origin
        // stays even to keep the stagger matching Tiled, so it centers one
        // column left of the middle.
        assert_eq!(
            GamePos::from(tile_grid_pos(14, 10, 0, 31, 20)),
            GamePos::new(0, 0, 0)
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

    #[test]
    fn map_points_snap_to_hex_centers() {
        // "Lower Left", on the map's bottom-left tile.
        assert_eq!(
            point_game_pos(16.0, 390.0, 0, 30, 20),
            Some(GamePos::new(-42, -36, 0))
        );
        // Odd columns sit half a tile lower in Tiled: the tile right of the
        // bottom-left one.
        assert_eq!(
            point_game_pos(40.0, 400.0, 0, 30, 20),
            Some(GamePos::new(-39, -38, 0))
        );
        // One hex north of "Lower Left": north is up on screen, and world
        // +y is up.
        assert_eq!(
            point_game_pos(16.0, 370.0, 0, 30, 20),
            Some(GamePos::new(-42, -32, 0))
        );
        // A raised layer keeps the tile indexes; only z changes.
        assert_eq!(
            point_game_pos(328.0, 120.0, 1, 30, 20),
            Some(GamePos::new(-3, 18, 1))
        );
        // Off the lattice: no position.
        assert_eq!(point_game_pos(20.0, 390.0, 0, 30, 20), None);
        assert_eq!(point_game_pos(16.0, 385.0, 0, 30, 20), None);
    }

    /// The tiles `TEST_MAP_PATH` should put on the grid, once it is loaded.
    fn expected_tiles(world: &World) -> Option<Vec<MapTile>> {
        let source = &world.resource::<HexMap>().source;
        world
            .resource::<Assets<TiledMap>>()
            .get(source)
            .map(|map| map_tiles(map.map()).collect())
    }

    /// The points `TEST_MAP_PATH` should place, once it is loaded.
    fn expected_points(world: &World) -> Option<Vec<MapPoint>> {
        let source = &world.resource::<HexMap>().source;
        world
            .resource::<Assets<TiledMap>>()
            .get(source)
            .map(|map| map_points(map.map()).collect())
    }

    /// The game positions the map has placed so far.
    fn placed_points(world: &mut World) -> Vec<GamePos> {
        world
            .query_filtered::<&GamePos, With<MapSpawned>>()
            .iter(world)
            .copied()
            .collect()
    }

    /// Runs the app headless, loading and spawning the map, and hands it to
    /// `check` once `done` holds.
    fn run_until(mut done: impl FnMut(&mut World) -> bool, check: impl FnOnce(&mut World)) {
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
            if done(app.world_mut()) {
                check(app.world_mut());
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let tiles = expected_tiles(app.world());
        let points = expected_points(app.world());
        panic!(
            "{TEST_MAP_PATH} did not fully spawn: {} tiles on the grid ({:?} expected), {} points placed ({:?} expected)",
            app.world().resource::<HexStorage>().tile_count(),
            tiles.as_ref().map(|tiles| tiles.len()),
            placed_points(app.world_mut()).len(),
            points.as_ref().map(|points| points.len()),
        );
    }

    #[test]
    fn spawns_every_tiled_tile_and_point_onto_the_grid() {
        run_until(
            |world| {
                expected_tiles(world)
                    .is_some_and(|tiles| world.resource::<HexStorage>().tile_count() == tiles.len())
                    && expected_points(world).is_some_and(|points| {
                        let placed = placed_points(world);
                        points.iter().all(|point| placed.contains(&point.game_pos))
                    })
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

                // Every map point is placed, exactly once, at its position.
                let points = expected_points(world).unwrap();
                let placed = placed_points(world);
                assert_eq!(placed.len(), points.len());
                for point in &points {
                    assert!(
                        placed.contains(&point.game_pos),
                        "no map point at {:?}",
                        point.game_pos
                    );
                }
            },
        );
    }
}
