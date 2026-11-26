use bevy::prelude::*;

pub(crate) mod tiled;

/// Wires up third-party crates. Give each dependency its own module here
/// (e.g. `mod avian2d;`) so that external setup stays out of the game plugins.
pub(super) fn plugin(app: &mut App) {
    app.add_plugins(tiled::plugin);
}
