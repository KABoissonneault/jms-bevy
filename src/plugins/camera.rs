use bevy::{camera::visibility::RenderLayers, prelude::*};

// RenderLayers help organize entities into different cameras,
// which helps manage render passes.
// Entities without a layer (ie: layer 0) are rendered on the
// main camera.
//
// Alternate render layer, which draws behind layer 0
pub const BACKGROUND: RenderLayers = RenderLayers::layer(1);

#[derive(Component)]
#[require(Camera2d)]
pub struct BackgroundCamera;

#[derive(Component)]
#[require(Camera2d)]
pub struct MainCamera;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Startup, initialize_camera);
}

fn initialize_camera(mut commands: Commands) {
    commands.spawn((
        BackgroundCamera,
        Camera {
            order: 0,
            ..default()
        },
        BACKGROUND,
    ));

    let main_camera = (
        MainCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
    );

    // In dev builds, the primary Egui context lives on this camera so the
    // debug UI renders above everything it draws, including Bevy UI.
    #[cfg(feature = "dev")]
    let main_camera = (main_camera, bevy_egui::PrimaryEguiContext);

    commands.spawn(main_camera);
}
