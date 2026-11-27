//! Components used within this project. Consider importing components using
//! the `crate::prelude::*` method instead of directly referencing the
//! `components` module.

// Declared before the modules below, so the macro is in scope inside them.
// Uses `bevy::prelude` paths for `Reflect`/`Component`/`Default`, so the
// wrapping file only needs the standard `bevy::prelude::*` import.
/// Defines a component that is reflected only in dev builds, so the world
/// inspector can show and edit its fields. The type must derive `Default`,
/// and must be added to [`register_reflected`] to appear in the inspector.
macro_rules! dev_reflect_component {
    ($item:item) => {
        #[cfg_attr(feature = "dev", derive(Reflect), reflect(Component, Default))]
        $item
    };
}

#[cfg(feature = "dev")]
use bevy::app::App;

pub(super) mod entity;
pub(super) mod game_pos;
pub(super) mod grid_pos;
pub(super) mod tile;

pub(super) use entity::*;
pub(super) use game_pos::*;
pub(super) use grid_pos::*;
pub(super) use tile::*;

/// Registers the dev-reflected components with the type registry, so the
/// world inspector can show their fields. Add every type wrapped in
/// `dev_reflect_component!` here.
#[cfg(feature = "dev")]
pub(super) fn register_reflected(app: &mut App) {
    app.register_type::<Character>()
        .register_type::<DynamicPosEntity>()
        .register_type::<GamePos>()
        .register_type::<GridPos>()
        .register_type::<Player>()
        .register_type::<Tile>()
        .register_type::<TileIndex>()
        .register_type::<TileState>();
}
