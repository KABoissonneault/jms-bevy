#![allow(unused)]

//! Queryable storage over the hex map — `bevy_ecs_tilemap`'s `TileStorage`
//! pattern, adapted to [`HexPos`]'s signed coordinates and z layers.
//!
//! Components stay the source of truth; this resource is a derived lookup
//! kept in sync by the lifecycle observers [`on_insert_hex_pos`] and
//! [`on_remove_hex_pos`], so spawning, moving, and despawning can't forget
//! to update it. All lookups are O(1):
//!
//! - [`HexStorage::tile_at`] — floor tiles (entities with a [`Tile`]
//!   component), in a dense per-z-layer grid whose holes are `None`;
//! - [`HexStorage::occupant_at`] — every other positioned entity (occupants
//!   standing on hexagons, attachments on vertices), in a position map;
//! - [`HexStorage::is_free`] — whether a hexagon can be entered;
//! - [`HexStorage::topmost_tile_at`] — the highest floor sharing an xy,
//!   the layer an occupant belongs on;
//! - [`HexStorage::edge_toward`] — the shared edge between two tiles,
//!   redirecting southern lookups to the neighbor's northern copy.
//!
//! The backing storages are private: dense grid vs. position map stays
//! swappable, and the dense grid covers only the cells seen so far,
//! growing outward as tiles spawn beyond it.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::components::{Direction, EdgeState, HexPos, Tile, TileEdges, TileState};

/// Where every positioned entity currently sits, so gameplay can ask
/// "what is at this hex?" without scanning the world. Mirrors
/// `bevy_ecs_tilemap`'s `TileStorage`, generalized to signed coordinates,
/// z layers, and entities that aren't tiles.
#[derive(Resource, Default)]
pub struct HexStorage {
    /// Floor tiles per z layer, dense over the grid cells seen so far.
    layers: HashMap<i32, TileLayer>,
    /// Non-tile entities keyed by position: occupants on hexagons and
    /// attachments on vertices. Tiles live in [`Self::layers`] instead, so
    /// an occupant and the floor under it can share a position.
    positions: HashMap<HexPos, Entity>,
    /// Reverse map of everything tracked, used to find an entity's
    /// previous slots when it moves or goes away: `Insert` fires on
    /// overwrites (moves) without saying where the entity came from, and
    /// `Remove` doesn't carry the old position either.
    slots: HashMap<Entity, Slot>,
}

/// Which index currently holds an entity.
enum Slot {
    /// A floor tile in the dense grid of its z layer.
    Tile(HexPos),
    /// An entry in the position map.
    Occupant(HexPos),
}

impl HexStorage {
    /// The tile entity at hexagon center `pos`, if a floor tile exists
    /// there.
    pub fn tile_at(&self, pos: HexPos) -> Option<Entity> {
        self.layers.get(&pos.z)?.get(pos)
    }

    /// The non-tile entity at canonical position `pos` — an occupant
    /// standing on a hexagon, or an attachment on a vertex.
    pub fn occupant_at(&self, pos: HexPos) -> Option<Entity> {
        self.positions.get(&pos).copied()
    }

    /// Whether the hexagon center `pos` can be entered: it has a floor
    /// tile and no occupant of its own. Always false for holes, vertices,
    /// and non-canonical positions. This is the raw storage view only —
    /// [`TileState`] and [`TileEdges`] restrictions layer on top in the
    /// movement rules.
    pub fn is_free(&self, pos: HexPos) -> bool {
        self.tile_at(pos).is_some() && self.occupant_at(pos).is_none()
    }

    /// The topmost floor tile covering the hexagon under `(x, y)` — its
    /// z layer and entity. This is the layer something standing at that
    /// spot belongs on: the highest floor sharing its xy, so it neither
    /// sinks under a higher layer's tile nor floats over bare ground.
    pub fn topmost_tile_at(&self, x: i32, y: i32) -> Option<(i32, Entity)> {
        self.layers
            .iter()
            .filter_map(|(&z, layer)| layer.get(HexPos::new(x, y, z)).map(|tile| (z, tile)))
            .max_by_key(|&(z, _)| z)
    }

    /// The traversal state of the shared edge between the tile at `pos`
    /// and its neighbor toward `direction`, read from whichever tile owns
    /// it per [`TileEdges::owner`]: the tile's own northern flag, or — for
    /// southern directions — the neighbor's northern copy. `None` when
    /// either side of the edge is missing: a hole, a missing neighbor, or
    /// a non-canonical position with no edges at all.
    pub fn edge_toward(
        &self,
        pos: HexPos,
        direction: Direction,
        edges: &Query<&TileEdges>,
    ) -> Option<EdgeState> {
        let (owner_pos, owned) = TileEdges::owner(pos, direction)?;
        let owner = self.tile_at(owner_pos)?;
        edges.get(owner).ok()?.owned(owned)
    }

    /// Number of stored floor tiles across all layers.
    pub fn tile_count(&self) -> usize {
        self.layers.values().map(TileLayer::tile_count).sum()
    }

    /// Number of z layers that hold at least one grid so far.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Starts tracking `entity` at `pos`, dropping its previous slots
    /// first. [`Tile`] carriers go into the dense grid, everything else
    /// into the position map.
    fn track(&mut self, entity: Entity, pos: HexPos, is_tile: bool) {
        self.untrack(entity);
        let slot = if is_tile {
            // Off-grid tiles are an upstream bug; `set` skips them and the
            // slot keeps cleanup harmless.
            debug_assert!(pos.to_grid().is_some(), "tile at non-center {pos:?}");
            self.layers.entry(pos.z).or_default().set(pos, entity);
            Slot::Tile(pos)
        } else {
            debug_assert!(pos.is_canonical(), "occupant at non-canonical {pos:?}");
            self.positions.insert(pos, entity);
            Slot::Occupant(pos)
        };
        self.slots.insert(entity, slot);
    }

    /// Drops `entity`'s slots, wherever they are. Every clear is checked
    /// by identity, so a slot that another entity has since overwritten is
    /// left alone.
    fn untrack(&mut self, entity: Entity) {
        let Some(slot) = self.slots.remove(&entity) else {
            return;
        };
        match slot {
            Slot::Tile(pos) => {
                if let Some(layer) = self.layers.get_mut(&pos.z) {
                    if layer.get(pos) == Some(entity) {
                        layer.remove(pos);
                    }
                }
            }
            Slot::Occupant(pos) => {
                if self.positions.get(&pos) == Some(&entity) {
                    self.positions.remove(&pos);
                }
            }
        }
    }
}

/// Keeps [`HexStorage`] in sync on [`HexPos`] insertions: spawns and moves
/// both land here, since `Insert` fires on overwrites too. Tile-ness is
/// read from the entity's [`Tile`] component, which is already present
/// when this observer runs — bundles are written before their observers
/// fire.
pub fn on_insert_hex_pos(
    insert: On<Insert<HexPos>>,
    entities: Query<(&HexPos, Option<&Tile>)>,
    mut storage: ResMut<HexStorage>,
) {
    let Ok((&pos, tile)) = entities.get(insert.entity) else {
        return;
    };
    storage.track(insert.entity, pos, tile.is_some());
}

/// Keeps [`HexStorage`] in sync when an entity's [`HexPos`] goes away —
/// component removal or despawn.
pub fn on_remove_hex_pos(remove: On<Remove<HexPos>>, mut storage: ResMut<HexStorage>) {
    storage.untrack(remove.entity);
}

/// Dense tile grid over one z layer — `bevy_ecs_tilemap`'s `TileStorage`
/// with an origin offset, since grid cells are signed. Holes are `None`,
/// and the grid grows outward as tiles spawn beyond it.
#[derive(Default)]
struct TileLayer {
    /// Row-major cells spanning `[origin_col, origin_col + cols)` ×
    /// `[origin_row, origin_row + rows)`.
    cells: Vec<Option<Entity>>,
    origin_col: i32,
    origin_row: i32,
    cols: i32,
    rows: i32,
}

impl TileLayer {
    /// The tile at hexagon center `pos`, if this layer holds one.
    fn get(&self, pos: HexPos) -> Option<Entity> {
        let (col, row) = pos.to_grid()?;
        let index = self.cell_index(col, row)?;
        self.cells[index]
    }

    /// Sets the tile at hexagon center `pos`, growing to cover it.
    /// Replaces whatever the cell held. Non-centers are skipped — callers
    /// assert against them.
    fn set(&mut self, pos: HexPos, entity: Entity) {
        let Some((col, row)) = pos.to_grid() else {
            return;
        };
        let mut index = self.cell_index(col, row);
        if index.is_none() {
            self.grow_to_include(col, row);
            index = self.cell_index(col, row);
        }
        self.cells[index.expect("grown to cover")] = Some(entity);
    }

    /// Clears the tile at hexagon center `pos`, wherever it lies.
    fn remove(&mut self, pos: HexPos) {
        let Some((col, row)) = pos.to_grid() else {
            return;
        };
        if let Some(index) = self.cell_index(col, row) {
            self.cells[index] = None;
        }
    }

    fn tile_count(&self) -> usize {
        self.cells.iter().filter(|cell| cell.is_some()).count()
    }

    /// The `cells` slot of `(col, row)`, or `None` if it lies outside the
    /// current bounds.
    fn cell_index(&self, col: i32, row: i32) -> Option<usize> {
        let (col, row) = (col - self.origin_col, row - self.origin_row);
        if col < 0 || col >= self.cols || row < 0 || row >= self.rows {
            return None;
        }
        Some((col + row * self.cols) as usize)
    }

    /// Expands the bounds to cover `(col, row)`, padding each side so
    /// creeping outward growth stays amortized, and carries the old cells
    /// over.
    fn grow_to_include(&mut self, col: i32, row: i32) {
        let pad_cols = (self.cols / 2).max(1);
        let pad_rows = (self.rows / 2).max(1);
        let origin_col = self.origin_col.min(col) - pad_cols;
        let origin_row = self.origin_row.min(row) - pad_rows;
        let cols = (self.origin_col + self.cols).max(col + 1) - origin_col + pad_cols;
        let rows = (self.origin_row + self.rows).max(row + 1) - origin_row + pad_rows;

        let mut cells = vec![None; (cols * rows) as usize];
        for row in 0..self.rows {
            let src = ((row * self.cols) as usize)..(((row + 1) * self.cols) as usize);
            let dst_row = row + self.origin_row - origin_row;
            let dst_start = (dst_row * cols + self.origin_col - origin_col) as usize;
            cells[dst_start..dst_start + self.cols as usize].copy_from_slice(&self.cells[src]);
        }
        *self = Self {
            cells,
            origin_col,
            origin_row,
            cols,
            rows,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// Stable fake entity ids for the world-free tests.
    fn entity(id: u32) -> Entity {
        Entity::from_raw_u32(id).unwrap()
    }

    #[test]
    fn tiles_are_stored_per_layer_with_holes() {
        let mut storage = HexStorage::default();
        let tile_a = entity(1);
        storage.track(tile_a, HexPos::from_grid(2, 1), true);
        assert_eq!(storage.tile_at(HexPos::from_grid(2, 1)), Some(tile_a));
        assert_eq!(storage.tile_count(), 1);

        // z layers are separate grids over the same cells.
        let tile_b = entity(2);
        storage.track(tile_b, HexPos::from_grid(2, 1).at_layer(1), true);
        assert_eq!(
            storage.tile_at(HexPos::from_grid(2, 1).at_layer(1)),
            Some(tile_b)
        );
        assert_eq!(storage.tile_at(HexPos::from_grid(2, 1)), Some(tile_a));

        // Holes are None: empty cells, vertices, other layers.
        assert_eq!(storage.tile_at(HexPos::from_grid(2, 2)), None);
        assert_eq!(storage.tile_at(HexPos::new(1, 2, 0)), None);
        assert_eq!(storage.tile_at(HexPos::from_grid(0, 0).at_layer(-4)), None);
        assert_eq!(storage.tile_count(), 2);
    }

    #[test]
    fn occupants_are_stored_independently_of_tiles() {
        let mut storage = HexStorage::default();
        let pos = HexPos::from_grid(1, 2);
        storage.track(entity(1), pos, true);
        storage.track(entity(2), pos, false);
        // The occupant sits on its floor without displacing it, and an
        // attachment can share the world at a vertex instead.
        assert_eq!(storage.tile_at(pos), Some(entity(1)));
        assert_eq!(storage.occupant_at(pos), Some(entity(2)));
        let vertex = HexPos::new(2, 0, 0);
        storage.track(entity(3), vertex, false);
        assert_eq!(storage.occupant_at(vertex), Some(entity(3)));
        assert_eq!(storage.tile_at(vertex), None);
    }

    #[test]
    fn moving_an_occupant_leaves_the_old_spot() {
        let mut storage = HexStorage::default();
        let mover = entity(7);
        let from = HexPos::from_grid(0, 0);
        let to = HexPos::from_grid(1, 0);
        storage.track(mover, from, false);
        assert_eq!(storage.occupant_at(from), Some(mover));

        storage.track(mover, to, false);
        assert_eq!(storage.occupant_at(from), None);
        assert_eq!(storage.occupant_at(to), Some(mover));

        storage.untrack(mover);
        assert_eq!(storage.occupant_at(to), None);
    }

    #[test]
    fn stale_slots_are_not_clobbered_by_identity_checks() {
        // Two occupants stack onto one position: the later one wins the
        // map slot. The earlier one leaving must not erase the winner's
        // entry.
        let mut storage = HexStorage::default();
        let pos = HexPos::from_grid(3, 1);
        storage.track(entity(1), pos, false);
        storage.track(entity(2), pos, false);
        assert_eq!(storage.occupant_at(pos), Some(entity(2)));

        storage.untrack(entity(1));
        assert_eq!(storage.occupant_at(pos), Some(entity(2)));
        storage.untrack(entity(2));
        assert_eq!(storage.occupant_at(pos), None);
    }

    #[test]
    fn is_free_needs_a_tile_and_no_occupant() {
        let mut storage = HexStorage::default();
        let pos = HexPos::from_grid(1, 2);
        // Holes aren't free, however empty.
        assert!(!storage.is_free(pos));

        storage.track(entity(1), pos, true);
        assert!(storage.is_free(pos));

        storage.track(entity(2), pos, false);
        assert!(!storage.is_free(pos));

        // Vertices hold no floor, so they're never enterable.
        assert!(!storage.is_free(HexPos::new(1, 2, 0)));
    }

    #[test]
    fn the_dense_grid_grows_outward_including_negatives() {
        let mut storage = HexStorage::default();
        let corners = [(0, 0), (-5, -4), (6, 5), (-7, 6), (8, -3), (-1, 7)];
        for (id, (col, row)) in corners.into_iter().enumerate() {
            let tile = entity(id as u32 + 1);
            storage.track(tile, HexPos::from_grid(col, row), true);
        }
        // Every cell survives every growth.
        for (id, (col, row)) in corners.into_iter().enumerate() {
            let pos = HexPos::from_grid(col, row);
            assert_eq!(
                storage.tile_at(pos),
                Some(entity(id as u32 + 1)),
                "cell ({col}, {row})"
            );
        }
        assert_eq!(storage.tile_count(), corners.len());
    }

    #[test]
    fn observers_track_spawn_move_and_despawn() {
        let mut world = World::new();
        world.insert_resource(HexStorage::default());
        world.add_observer(on_insert_hex_pos);
        world.add_observer(on_remove_hex_pos);

        let origin = HexPos::from_grid(0, 0);
        let east = HexPos::from_grid(1, 0);

        // Tiles spawned in a bundle are classified by their Tile component.
        let tile = world.spawn((origin, Tile { floor: 0 })).id();
        assert_eq!(world.resource::<HexStorage>().tile_at(origin), Some(tile));
        assert!(world.resource::<HexStorage>().is_free(origin));

        // A bare position is an occupant, making its spot unfree.
        let walker = world.spawn(origin).id();
        assert_eq!(
            world.resource::<HexStorage>().occupant_at(origin),
            Some(walker)
        );
        assert!(!world.resource::<HexStorage>().is_free(origin));

        // Moving by overwriting HexPos re-slots the entity.
        world.entity_mut(walker).insert(east);
        assert_eq!(world.resource::<HexStorage>().occupant_at(origin), None);
        assert_eq!(
            world.resource::<HexStorage>().occupant_at(east),
            Some(walker)
        );

        // Despawning cleans up without touching the tiles.
        world.despawn(walker);
        assert_eq!(world.resource::<HexStorage>().occupant_at(east), None);
        assert_eq!(world.resource::<HexStorage>().tile_at(origin), Some(tile));

        // Vertex attachments are tracked too, independent of tiles.
        let vertex = HexPos::new(1, 2, 0);
        let attachment = world.spawn(vertex).id();
        assert_eq!(
            world.resource::<HexStorage>().occupant_at(vertex),
            Some(attachment)
        );
        assert_eq!(world.resource::<HexStorage>().tile_at(vertex), None);
        world.despawn(attachment);
        assert_eq!(world.resource::<HexStorage>().occupant_at(vertex), None);
    }

    #[test]
    fn topmost_tile_at_takes_the_highest_floor() {
        let mut storage = HexStorage::default();
        let xy = HexPos::from_grid(2, 3); // (6, 12)
        // Layers stacked out of order, plus an occupant far above — which
        // is not a floor and must not win.
        storage.track(entity(1), xy.at_layer(0), true);
        storage.track(entity(2), xy.at_layer(-2), true);
        storage.track(entity(3), xy.at_layer(5), false);
        storage.track(entity(4), xy.at_layer(1), true);
        assert_eq!(storage.topmost_tile_at(6, 12), Some((1, entity(4))));

        // Removing the top exposes the layer below.
        storage.untrack(entity(4));
        assert_eq!(storage.topmost_tile_at(6, 12), Some((0, entity(1))));

        // Bare ground has no top at all.
        assert_eq!(storage.topmost_tile_at(0, 4), None);
    }

    #[test]
    fn edge_toward_redirects_southern_lookups_to_the_neighbor() {
        let mut world = World::new();
        world.insert_resource(HexStorage::default());
        world.add_observer(on_insert_hex_pos);
        world.add_observer(on_remove_hex_pos);

        let origin = HexPos::from_grid(0, 0);
        let north = origin.neighbor(Direction::North).unwrap();
        let south = origin.neighbor(Direction::South).unwrap();
        let origin_tile = world.spawn((origin, Tile { floor: 0 })).id();
        world.spawn((north, Tile { floor: 0 }));
        let south_tile = world.spawn((south, Tile { floor: 0 })).id();

        // Required components put fresh tiles in the free state on both
        // sides of every edge.
        assert_eq!(world.get::<TileState>(origin_tile), Some(&TileState::Free));
        assert_eq!(world.get::<TileEdges>(origin_tile), Some(&TileEdges::FREE));

        // Every edge with a tile on both sides reads free: north from the
        // origin's own flags, south from its southern neighbor's copy.
        world
            .run_system_once(move |storage: Res<HexStorage>, edges: Query<&TileEdges>| {
                for direction in [Direction::North, Direction::South] {
                    assert_eq!(
                        storage.edge_toward(origin, direction, &edges),
                        Some(EdgeState::Free),
                        "origin toward {direction:?}"
                    );
                }
                // The northern edges are the tile's own flags: they
                // answer even where no tile lies beyond the edge, reading
                // free from the origin itself.
                assert_eq!(
                    storage.edge_toward(origin, Direction::NorthWest, &edges),
                    Some(EdgeState::Free)
                );
            })
            .unwrap();

        // Blocking the origin's own north edge shows up when read from the
        // other side too: one physical edge, one stored copy. Southern
        // lookups stay independent — they read the southern neighbor's
        // copy, not this one.
        world.entity_mut(origin_tile).insert(TileEdges {
            north: EdgeState::Blocked,
            ..Default::default()
        });
        world
            .run_system_once(move |storage: Res<HexStorage>, edges: Query<&TileEdges>| {
                assert_eq!(
                    storage.edge_toward(origin, Direction::North, &edges),
                    Some(EdgeState::Blocked)
                );
                assert_eq!(
                    storage.edge_toward(north, Direction::South, &edges),
                    Some(EdgeState::Blocked)
                );
                assert_eq!(
                    storage.edge_toward(origin, Direction::South, &edges),
                    Some(EdgeState::Free)
                );
            })
            .unwrap();

        // Blocking the southern neighbor's north edge is the same act as
        // blocking the origin's south edge — the redirected lookup and the
        // owner's own agree.
        world.entity_mut(south_tile).insert(TileEdges {
            north: EdgeState::Blocked,
            ..Default::default()
        });
        world
            .run_system_once(move |storage: Res<HexStorage>, edges: Query<&TileEdges>| {
                assert_eq!(
                    storage.edge_toward(origin, Direction::South, &edges),
                    Some(EdgeState::Blocked)
                );
                assert_eq!(
                    storage.edge_toward(south, Direction::North, &edges),
                    Some(EdgeState::Blocked)
                );

                // No neighbor, no edge: south of the south tile and
                // southwest of the origin there is nothing to read, and
                // non-canonical positions have no edges at all.
                assert_eq!(storage.edge_toward(south, Direction::South, &edges), None);
                assert_eq!(
                    storage.edge_toward(origin, Direction::SouthWest, &edges),
                    None
                );
                assert_eq!(
                    storage.edge_toward(HexPos::new(1, 2, 0), Direction::North, &edges),
                    None
                );
            })
            .unwrap();
    }
}
