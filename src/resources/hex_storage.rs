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

use crate::prelude::*;
use bevy::prelude::*;

/// Where every positioned tile currently sits, so gameplay can ask
/// "what is at this hex?" without scanning the world. Mirrors
/// `bevy_ecs_tilemap`'s `TileStorage`, generalized to signed coordinates,
/// z layers, and entities that aren't tiles.
#[derive(Resource, Default)]
pub struct HexStorage {
    /// Floor tiles per z layer, dense over the grid cells seen so far.
    layers: HashMap<i32, TileLayer>,
}

impl HexStorage {
    /// The tile entity at hexagon center `pos`, if a floor tile exists
    /// there.
    pub fn tile_at(&self, pos: GridPos) -> Option<Entity> {
        self.layers.get(&pos.layer)?.get(pos)
    }

    /// Number of stored floor tiles across all layers.
    pub fn tile_count(&self) -> usize {
        self.layers.values().map(TileLayer::tile_count).sum()
    }

    /// Number of z layers that hold at least one tile so far.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    fn register_tile(&mut self, entity: Entity, pos: GridPos) {
        self.unregister_tile(entity, pos);
        self.layers.entry(pos.layer).or_default().set(pos, entity);
    }

    fn unregister_tile(&mut self, entity: Entity, pos: GridPos) {
        if let Some(layer) = self.layers.get_mut(&pos.layer) {
            if layer.get(pos) == Some(entity) {
                layer.remove(pos);
            }
        }
    }
}

/// Keeps [`HexStorage`] in sync on [`HexPos`] insertions: spawns and moves
/// both land here, since `Insert` fires on overwrites too. Tile-ness is
/// read from the entity's [`Tile`] component, which is already present
/// when this observer runs — bundles are written before their observers
/// fire.
pub fn on_insert_tile(
    insert: On<Insert<GridPos>>,
    entities: Query<&GridPos, With<Tile>>,
    mut storage: ResMut<HexStorage>,
) {
    let Ok(&pos) = entities.get(insert.entity) else {
        return;
    };
    storage.register_tile(insert.entity, pos);
}

/// Keeps [`HexStorage`] in sync when an entity's [`HexPos`] goes away —
/// component removal or despawn.
pub fn on_remove_tile(
    remove: On<Remove<GridPos>>,
    entities: Query<&GridPos, With<Tile>>,
    mut storage: ResMut<HexStorage>,
) {
    let Ok(&pos) = entities.get(remove.entity) else {
        return;
    };

    storage.unregister_tile(remove.entity, pos);
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
    fn get(&self, pos: GridPos) -> Option<Entity> {
        let (col, row) = pos.to_offset();
        let index = self.cell_index(col, row)?;
        self.cells[index]
    }

    /// Sets the tile at hexagon center `pos`, growing to cover it.
    /// Replaces whatever the cell held. Non-centers are skipped — callers
    /// assert against them.
    fn set(&mut self, pos: GridPos, entity: Entity) {
        let (col, row) = pos.to_offset();
        let mut index = self.cell_index(col, row);
        if index.is_none() {
            self.grow_to_include(col, row);
            index = self.cell_index(col, row);
        }
        self.cells[index.expect("grown to cover")] = Some(entity);
    }

    /// Clears the tile at hexagon center `pos`, wherever it lies.
    fn remove(&mut self, pos: GridPos) {
        let (col, row) = pos.to_offset();
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
        storage.register_tile(tile_a, GridPos::from_offset(2, 1, 0));
        assert_eq!(storage.tile_at(GridPos::from_offset(2, 1, 0)), Some(tile_a));
        assert_eq!(storage.tile_count(), 1);

        // z layers are separate grids over the same cells.
        let tile_b = entity(2);
        storage.register_tile(tile_b, GridPos::from_offset(2, 1, 1));
        assert_eq!(storage.tile_at(GridPos::from_offset(2, 1, 1)), Some(tile_b));
        assert_eq!(storage.tile_at(GridPos::from_offset(2, 1, 0)), Some(tile_a));

        // Holes are None: empty cells, vertices, other layers.
        assert_eq!(storage.tile_at(GridPos::from_offset(2, 2, 0)), None);
        assert_eq!(storage.tile_at(GridPos::from_offset(2, 2, -4)), None);
        assert_eq!(storage.tile_count(), 2);
    }

    #[test]
    fn the_dense_grid_grows_outward_including_negatives() {
        let mut storage = HexStorage::default();
        let corners = [(0, 0), (-5, -4), (6, 5), (-7, 6), (8, -3), (-1, 7)];
        for (id, (col, row)) in corners.into_iter().enumerate() {
            let tile = entity(id as u32 + 1);
            storage.register_tile(tile, GridPos::from_offset(col, row, 0));
        }
        // Every cell survives every growth.
        for (id, (col, row)) in corners.into_iter().enumerate() {
            let pos = GridPos::from_offset(col, row, 0);
            assert_eq!(
                storage.tile_at(pos),
                Some(entity(id as u32 + 1)),
                "cell ({col}, {row})"
            );
        }
        assert_eq!(storage.tile_count(), corners.len());
    }

    #[test]
    fn observers_track_spawn_and_despawn() {
        let mut world = World::new();
        world.insert_resource(HexStorage::default());
        world.add_observer(on_insert_tile);
        world.add_observer(on_remove_tile);

        let origin = GridPos::from_offset(0, 0, 0);
        let east = GridPos::from_offset(1, 0, 0);

        // Tiles spawned in a bundle are classified by their Tile component.
        let tile = world.spawn((origin, Tile)).id();
        assert_eq!(world.resource::<HexStorage>().tile_at(origin), Some(tile));

        world.despawn(tile);

        assert_eq!(world.resource::<HexStorage>().tile_at(origin), None);
    }
}
