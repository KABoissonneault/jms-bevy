#![allow(unused_imports)]

use bevy::prelude::*;

mod components;
mod movement;
mod plugins;
mod projection;
mod resources;
mod styles;
mod third_party;

/// Use this module instead of importing the `components`, `plugins`, `resources`, and `utils`
/// modules directly.
mod prelude {
    pub use super::*;
    pub use {components::*, movement::*, plugins::*, projection::*, resources::*, styles::*};
}

pub struct AppPlugin;

impl Plugin for AppPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            third_party::plugin,
            plugins::camera::plugin,
            plugins::defaults::plugin,
            plugins::game::plugin,
            plugins::input::plugin,
            plugins::tiling::plugin,
            plugins::entity::plugin,
        ));

        // Enable dev tools for dev builds.
        #[cfg(feature = "dev")]
        app.add_plugins(plugins::debug::plugin);
    }
}
