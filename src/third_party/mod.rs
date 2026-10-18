use bevy::prelude::*;

/// Wires up third-party Bevy plugins. Give each dependency its own module here
/// (e.g. `mod avian2d;`) so that external setup stays out of the game plugins.
pub(super) fn plugin(_app: &mut App) {
    // No third-party plugins yet. Per the Tech Requirements, candidates include
    // a physics engine and an input manager, to be added when a tech design
    // calls for them.
}
