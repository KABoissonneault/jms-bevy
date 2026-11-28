#![allow(unused)]

use bevy::prelude::*;

dev_reflect_component! {
    /// Grid position in "Doubled" coordinates. See https://www.redblobgames.com/grids/hexagons/#coordinates
    /// Each row adds two "drows", and odd columns are offset by 1 drow
    #[derive(Component, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GridPos {
        pub col: i32,
        pub drow: i32,
        pub layer: i32,
    }
}

impl GridPos {
    pub const fn new(col: i32, drow: i32, layer: i32) -> Self {
        Self { col, drow, layer }
    }

    /// From "offset" coordinates (ie: diff from origin)
    pub const fn from_offset(col: i32, row: i32, layer: i32) -> Self {
        Self {
            col,
            drow: 2 * row - (col & 1),
            layer,
        }
    }

    /// The exact inverse of [`GridPos::from_offset`]: the `+ (col & 1)` is
    /// what makes the division exact, so odd drows on odd columns (which are
    /// negative) don't truncate onto the even drow below them.
    pub const fn to_offset(self) -> (i32, i32) {
        (self.col, (self.drow + (self.col & 1)) / 2)
    }

    pub const fn at_layer(self, layer: i32) -> Self {
        Self::new(self.col, self.drow, layer)
    }

    /// The position one `direction` step away, on the same z layer
    pub const fn neighbor(self, direction: Direction) -> Self {
        let (dx, dy) = direction.grid_delta();
        GridPos::new(self.col + dx, self.drow + dy, self.layer)
    }

    /// All six neighbors, paired with their direction from this position
    pub fn neighbors(self) -> impl Iterator<Item = (Direction, GridPos)> {
        Direction::ALL
            .into_iter()
            .map(move |direction| (direction, self.neighbor(direction)))
    }
}

/// The six directions between neighboring hexagons,
/// named for the flat-top layout
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
    pub const ALL: [Direction; 6] = [
        Direction::NorthWest,
        Direction::North,
        Direction::NorthEast,
        Direction::SouthWest,
        Direction::South,
        Direction::SouthEast,
    ];

    /// Assumes Doubled coordinates
    pub const fn grid_delta(self) -> (i32, i32) {
        match self {
            Direction::NorthWest => (-1, 1),
            Direction::North => (0, 2),
            Direction::NorthEast => (1, 1),
            Direction::SouthWest => (-1, -1),
            Direction::South => (0, -2),
            Direction::SouthEast => (1, -1),
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
