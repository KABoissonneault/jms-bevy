#![allow(unused)]

use bevy::prelude::*;

// Hex geometry. See `doc/vault/jms/Tech Design/Tiling.md`.

// Steps between neighboring grid positions, in dx/dy units.

/// x delta between adjacent tile columns: `3 dx`.
pub const COLUMN_DX: i32 = 3;
/// y stagger between adjacent tile columns: `2 dy`.
pub const COLUMN_DY: i32 = 2;
/// y delta between adjacent tile rows: `4 dy`.
pub const ROW_DY: i32 = 4;

/// The six directions between neighboring hexagons, named for the flat-top
/// layout and matching the `QWEASD` movement keys of `Tiling.md`: Q moves
/// `NorthWest`, W `North`, E `NorthEast`, A `SouthWest`, S `South`, and D
/// `SouthEast`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    NorthWest,
    North,
    NorthEast,
    SouthWest,
    South,
    SouthEast,
}

impl Direction {
    /// Every direction, in `QWEASD` order.
    pub const ALL: [Direction; 6] = [
        Direction::NorthWest,
        Direction::North,
        Direction::NorthEast,
        Direction::SouthWest,
        Direction::South,
        Direction::SouthEast,
    ];

    /// Step between neighboring hexagons in this direction, in `(dx, dy)`
    /// quarter-hex units: `Tiling.md`'s neighbor offsets `(0, ±4dy)` and
    /// `(±3dx, ±2dy)`.
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::NorthWest => (-COLUMN_DX, COLUMN_DY),
            Direction::North => (0, ROW_DY),
            Direction::NorthEast => (COLUMN_DX, COLUMN_DY),
            Direction::SouthWest => (-COLUMN_DX, -COLUMN_DY),
            Direction::South => (0, -ROW_DY),
            Direction::SouthEast => (COLUMN_DX, -COLUMN_DY),
        }
    }

    /// The direction pointing back where this one comes from.
    pub const fn opposite(self) -> Self {
        match self {
            Direction::NorthWest => Direction::SouthEast,
            Direction::North => Direction::South,
            Direction::NorthEast => Direction::SouthWest,
            Direction::SouthWest => Direction::NorthEast,
            Direction::South => Direction::North,
            Direction::SouthEast => Direction::NorthWest,
        }
    }
}

/// The kind of site a canonical [`HexPos`] holds (`Tiling.md`): the center
/// of a hexagon, or one of the vertices where three hexagons meet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PosKind {
    /// A hexagon center, where tiles live.
    Center,
    /// A vertex shared by three hexagons, where vertex attachments live.
    Vertex,
}

/// Position of a gameplay entity on the hex world, in the game coordinates
/// of `Tiling.md`: (x, y) are in quarter-hex units (dx and dy) on the
/// horizontal slice of the world, and z is the hex grid layer index.
/// Hexagon centers and vertices are always aligned to an even y.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    /// The grid cell `(col, row)` under this position: the inverse of
    /// [`HexPos::from_grid`]. Only hexagon centers sit on grid cells —
    /// checking `x % 3 == 0` directly because [`PosKind`] comparison isn't
    /// const — so vertices and non-canonical positions return `None`.
    pub const fn to_grid(self) -> Option<(i32, i32)> {
        if !self.is_canonical() || self.x.rem_euclid(COLUMN_DX) != 0 {
            return None;
        }
        let col = self.x.div_euclid(COLUMN_DX);
        // Odd columns sit COLUMN_DY above the row line, same as in
        // `from_grid`.
        let row = (self.y - COLUMN_DY * (col & 1)).div_euclid(ROW_DY);
        Some((col, row))
    }

    /// Whether `(x, y)` is a canonical position (`Tiling.md`): aligned to a
    /// hexagon center or a vertex. Canonical positions have an even y whose
    /// half matches the parity of x — equivalently, `2x + y` is a multiple
    /// of 4. `rem_euclid` keeps this correct for negative coordinates. The
    /// z layer never affects canonicity.
    pub const fn is_canonical(self) -> bool {
        (2 * self.x + self.y).rem_euclid(4) == 0
    }

    /// Whether this position is a hexagon center or a vertex: centers have
    /// x divisible by 3, vertices x ≡ 1 or 2 (mod 3). Only meaningful for
    /// canonical positions — check [`HexPos::is_canonical`] first when the
    /// position could come from outside the tiling API.
    pub const fn kind(self) -> PosKind {
        if self.x.rem_euclid(3) == 0 {
            PosKind::Center
        } else {
            PosKind::Vertex
        }
    }

    /// The position one `direction` step away, on the same z layer, using
    /// the neighbor offsets from `Tiling.md`. All six neighbors of a
    /// canonical position exist and keep its [`HexPos::kind`]: centers
    /// neighbor centers, vertices neighbor vertices (each stepping to the
    /// matching vertex of the neighboring hexes).
    ///
    /// Returns `None` for non-canonical positions, which sit outside the
    /// hex lattice and have no neighbors.
    pub const fn neighbor(self, direction: Direction) -> Option<Self> {
        if !self.is_canonical() {
            return None;
        }
        let (dx, dy) = direction.delta();
        Some(Self::new(self.x + dx, self.y + dy, self.z))
    }

    /// All six neighbors, paired with their direction from this position,
    /// in `QWEASD` order. Empty for non-canonical positions — the same
    /// cases where [`HexPos::neighbor`] returns `None`.
    pub fn neighbors(self) -> impl Iterator<Item = (Direction, Self)> {
        Direction::ALL.into_iter().filter_map(move |direction| {
            self.neighbor(direction)
                .map(|neighbor| (direction, neighbor))
        })
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
        // columns (`& 1` is never negative) are staggered up as well.
        assert_eq!(
            HexPos::from_grid(-1, 0),
            HexPos::new(-COLUMN_DX, COLUMN_DY, 0)
        );
        // Odd columns sit COLUMN_DY above the row line.
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
    fn to_grid_inverts_from_grid() {
        for col in -4..=4 {
            for row in -4..=4 {
                assert_eq!(
                    HexPos::from_grid(col, row).to_grid(),
                    Some((col, row)),
                    "cell ({col}, {row})"
                );
            }
        }
        // Vertices and non-canonical positions have no grid cell.
        assert_eq!(HexPos::new(1, 2, 0).to_grid(), None);
        assert_eq!(HexPos::new(2, 0, 0).to_grid(), None);
        assert_eq!(HexPos::new(0, 2, 0).to_grid(), None);
        assert_eq!(HexPos::new(1, 1, 7).to_grid(), None);
    }

    #[test]
    fn neighbor_offsets_match_tiling_doc() {
        // Tiling.md: neighbor hexes at (0, ±4dy) and (±3dx, ±2dy), with
        // y increasing upward. QWEASD maps to NW N NE / SW S SE.
        let origin = HexPos::new(0, 0, 5);
        assert_eq!(
            origin.neighbor(Direction::NorthWest),
            Some(HexPos::new(-COLUMN_DX, COLUMN_DY, 5))
        );
        assert_eq!(
            origin.neighbor(Direction::North),
            Some(HexPos::new(0, ROW_DY, 5))
        );
        assert_eq!(
            origin.neighbor(Direction::NorthEast),
            Some(HexPos::new(COLUMN_DX, COLUMN_DY, 5))
        );
        assert_eq!(
            origin.neighbor(Direction::SouthWest),
            Some(HexPos::new(-COLUMN_DX, -COLUMN_DY, 5))
        );
        assert_eq!(
            origin.neighbor(Direction::South),
            Some(HexPos::new(0, -ROW_DY, 5))
        );
        assert_eq!(
            origin.neighbor(Direction::SouthEast),
            Some(HexPos::new(COLUMN_DX, -COLUMN_DY, 5))
        );
    }

    #[test]
    fn neighbor_walks_adjacent_grid_cells() {
        // Every grid cell's neighbors are the adjacent grid cells: straight
        // north/south within the column, and diagonally into the next
        // column, whose stagger depends on the starting column's parity.
        // Exercises both parities and negative columns.
        for col in -4..=4 {
            let stagger = col & 1;
            for row in -4..=4 {
                let tile = HexPos::from_grid(col, row);
                assert_eq!(
                    tile.neighbor(Direction::North),
                    Some(HexPos::from_grid(col, row + 1))
                );
                assert_eq!(
                    tile.neighbor(Direction::South),
                    Some(HexPos::from_grid(col, row - 1))
                );
                assert_eq!(
                    tile.neighbor(Direction::NorthEast),
                    Some(HexPos::from_grid(col + 1, row + stagger))
                );
                assert_eq!(
                    tile.neighbor(Direction::NorthWest),
                    Some(HexPos::from_grid(col - 1, row + stagger))
                );
                assert_eq!(
                    tile.neighbor(Direction::SouthEast),
                    Some(HexPos::from_grid(col + 1, row + stagger - 1))
                );
                assert_eq!(
                    tile.neighbor(Direction::SouthWest),
                    Some(HexPos::from_grid(col - 1, row + stagger - 1))
                );
            }
        }
    }

    #[test]
    fn canonical_positions_are_exactly_centers_and_vertices() {
        // Every grid cell is a canonical center.
        for col in -4..=4 {
            for row in -4..=4 {
                let center = HexPos::from_grid(col, row);
                assert!(center.is_canonical());
                assert_eq!(center.kind(), PosKind::Center);
            }
        }
        // Around every center, the six vertex offsets hold canonical
        // vertices.
        for center in [
            HexPos::from_grid(0, 0),
            HexPos::from_grid(1, 0),
            HexPos::from_grid(-1, 2),
        ] {
            for (vx, vy) in [(-1, 2), (1, 2), (2, 0), (1, -2), (-1, -2), (-2, 0)] {
                let vertex = HexPos::new(center.x + vx, center.y + vy, 0);
                assert!(vertex.is_canonical());
                assert_eq!(vertex.kind(), PosKind::Vertex);
            }
        }
        // Between the sites lie positions that are neither centers nor
        // vertices — e.g. (0, 2), halfway up the edge shared by vertically
        // adjacent hexes — and they must not pass as canonical.
        for pos in [
            HexPos::new(0, 2, 0),
            HexPos::new(3, 0, 0),
            HexPos::new(2, 2, 0),
            HexPos::new(1, 0, 0),
            HexPos::new(-1, 0, 0),
            HexPos::new(-2, 2, 0),
            HexPos::new(1, 1, 0),
        ] {
            assert!(!pos.is_canonical());
        }
        // Canonicality holds for negative coordinates too, where a plain
        // `%` would go wrong.
        assert!(HexPos::new(-COLUMN_DX, COLUMN_DY, 0).is_canonical());
        assert!(HexPos::new(-1, -2, 0).is_canonical());
    }

    #[test]
    fn neighbor_rejects_non_canonical_positions() {
        for pos in [
            HexPos::new(0, 2, 0),
            HexPos::new(1, 1, 0),
            HexPos::new(-2, 2, 3),
        ] {
            for direction in Direction::ALL {
                assert_eq!(pos.neighbor(direction), None);
            }
            assert_eq!(pos.neighbors().count(), 0);
        }
    }

    #[test]
    fn neighbors_preserve_canonical_kind() {
        // Neighbor steps translate the lattice: centers stay centers,
        // vertices stay vertices, everything stays canonical on the same
        // z layer.
        for col in -3..=3 {
            for row in -3..=3 {
                let center = HexPos::from_grid(col, row).at_layer(2);
                for (_, neighbor) in center.neighbors() {
                    assert!(neighbor.is_canonical());
                    assert_eq!(neighbor.kind(), PosKind::Center);
                    assert_eq!(neighbor.z, 2);
                }
                // One vertex of each offset shape per center.
                for (vx, vy) in [(-1, 2), (2, 0)] {
                    let vertex = HexPos::new(center.x + vx, center.y + vy, -1);
                    for (_, neighbor) in vertex.neighbors() {
                        assert!(neighbor.is_canonical());
                        assert_eq!(neighbor.kind(), PosKind::Vertex);
                        assert_eq!(neighbor.z, -1);
                    }
                }
            }
        }
    }

    #[test]
    fn vertex_neighbors_translate_the_vertex_lattice() {
        // Vertex (1, 2) — the top-right vertex of the origin hex — steps to
        // the matching vertex of each neighboring hex.
        let vertex = HexPos::new(1, 2, 0);
        assert_eq!(
            vertex.neighbor(Direction::North),
            Some(HexPos::new(1, 6, 0))
        );
        assert_eq!(
            vertex.neighbor(Direction::NorthEast),
            Some(HexPos::new(4, 4, 0))
        );
        assert_eq!(
            vertex.neighbor(Direction::SouthEast),
            Some(HexPos::new(4, 0, 0))
        );
        assert_eq!(
            vertex.neighbor(Direction::South),
            Some(HexPos::new(1, -2, 0))
        );
        assert_eq!(
            vertex.neighbor(Direction::SouthWest),
            Some(HexPos::new(-2, 0, 0))
        );
        assert_eq!(
            vertex.neighbor(Direction::NorthWest),
            Some(HexPos::new(-2, 4, 0))
        );
    }

    #[test]
    fn opposite_directions_backtrack() {
        for direction in Direction::ALL {
            assert_eq!(direction.opposite().opposite(), direction);
            let start = HexPos::from_grid(1, 2).at_layer(-3);
            assert_eq!(
                start
                    .neighbor(direction)
                    .and_then(|step| step.neighbor(direction.opposite())),
                Some(start)
            );
        }
    }

    #[test]
    fn neighbors_lists_all_six_directions() {
        let center = HexPos::from_grid(2, -1);
        let listed: Vec<_> = center.neighbors().collect();
        assert_eq!(listed.len(), Direction::ALL.len());
        for (direction, neighbor) in listed {
            assert!(Direction::ALL.contains(&direction));
            assert_eq!(center.neighbor(direction), Some(neighbor));
        }
    }
}
