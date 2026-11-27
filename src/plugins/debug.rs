use bevy::app::App;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::window::{WindowCloseRequested, WindowRef, WindowResolution};
use bevy_egui::{EguiContext, EguiGlobalSettings, EguiPlugin, EguiSchedule, egui};
use bevy_inspector_egui::{DefaultInspectorConfigPlugin, bevy_inspector};
use bevy_remote::{RemotePlugin, http::RemoteHttpPlugin};

// Dev-only debug tools, each shown as a tab of a separate OS "Debug" window
// with its own egui context, toggled with F1:
// - a diagnostics tab, which keeps the console output clear for warnings,
//   errors, and important event logging;
// - the `bevy-inspector-egui` world inspector, to browse and edit entities,
//   resources, and assets at runtime.
//
// Debug UI never renders inside the main viewport.
//
// To add a future debug tool: add a `DebugTool` variant and draw its tab
// content in `show_debug_tools`.

/// The schedule driving the egui context of the debug window. Each egui
/// context must have its own unique pass schedule.
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct DebugWindowPass;

/// The debug OS window and the camera that renders its egui context.
#[derive(Clone, Copy)]
struct DebugWindow {
    window: Entity,
    camera: Entity,
}

/// A debug tool displayed as a tab of the debug window.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum DebugTool {
    #[default]
    Diagnostics,
    WorldInspector,
}

impl DebugTool {
    const ALL: [DebugTool; 2] = [DebugTool::Diagnostics, DebugTool::WorldInspector];

    fn title(self) -> &'static str {
        match self {
            DebugTool::Diagnostics => "Diagnostics",
            DebugTool::WorldInspector => "World Inspector",
        }
    }
}

/// Whether the debug window should be open, the tab it shows, plus the
/// entities of the OS window hosting it.
#[derive(Resource, Default)]
pub(crate) struct ShowDebug {
    open: bool,
    tab: DebugTool,
    window: Option<DebugWindow>,
}

pub(crate) fn plugin(app: &mut App) {
    // bevy_egui attaches a context to the first camera spawned by default.
    // Debug UI must stay out of the main viewport, so no primary context is
    // created and the debug window's camera carries the only egui context.
    app.insert_resource(EguiGlobalSettings {
        auto_create_primary_context: false,
        ..Default::default()
    })
    .add_plugins((
        EguiPlugin::default(),
        // Registers the reflection options and egui impls the inspector needs.
        DefaultInspectorConfigPlugin,
        FrameTimeDiagnosticsPlugin::default(),
        EntityCountDiagnosticsPlugin::default(),
        SystemInformationDiagnosticsPlugin,
        RemotePlugin::default(),
        RemoteHttpPlugin::default(),
    ))
    .init_resource::<ShowDebug>()
    .add_systems(
        Update,
        (
            toggle_debug_window,
            handle_debug_window_close,
            sync_debug_window,
        )
            .chain(),
    )
    .add_systems(DebugWindowPass, show_debug_tools);
}

/// F1 toggles the debug window.
fn toggle_debug_window(keyboard: Res<ButtonInput<KeyCode>>, mut show_debug: ResMut<ShowDebug>) {
    if keyboard.just_pressed(KeyCode::F1) {
        show_debug.open = !show_debug.open;
    }
}

/// Closes the debug window when it is closed through the OS close button.
fn handle_debug_window_close(
    mut commands: Commands,
    mut close_requested: MessageReader<WindowCloseRequested>,
    mut show_debug: ResMut<ShowDebug>,
) {
    for request in close_requested.read() {
        if let Some(debug_window) = show_debug.window
            && debug_window.window == request.window
        {
            // Bevy despawns the window entity itself in response to the close
            // request; only our camera needs to go.
            commands.entity(debug_window.camera).despawn();
            show_debug.window = None;
            show_debug.open = false;
        }
    }
}

/// Keeps the debug window open exactly while `ShowDebug::open` is set.
fn sync_debug_window(mut commands: Commands, mut show_debug: ResMut<ShowDebug>) {
    match (show_debug.open, show_debug.window) {
        (true, None) => open_debug_window(&mut commands, &mut show_debug),
        (false, Some(debug_window)) => {
            close_debug_window(&mut commands, &mut show_debug, debug_window)
        }
        _ => {}
    }
}

/// Draws the debug window: a tab bar with one tab per tool, and the selected
/// tool's content filling the rest of the window.
///
/// Takes the world exclusively: the world inspector browses and edits
/// entities, resources, and assets directly, which requires `&mut World`.
fn show_debug_tools(world: &mut World) -> Result {
    let (tab, window) = {
        let show_debug = world.resource::<ShowDebug>();
        (show_debug.tab, show_debug.window)
    };
    let Some(debug_window) = window else {
        return Ok(());
    };

    // The egui context is a cheap handle: cloning it leaves the world free
    // for the tools while the window is drawn.
    let Some(ctx) = world
        .get_entity_mut(debug_window.camera)?
        .get_mut::<EguiContext>()
        .map(|mut context| context.get_mut().clone())
    else {
        return Ok(());
    };

    // Panels are shown inside a root `Ui` spanning the viewport, as in
    // bevy_egui's `side_panel` example.
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "debug_viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    let mut selected = tab;
    egui::Panel::top("debug_tool_tabs").show(&mut viewport_ui, |ui| {
        ui.horizontal(|ui| {
            for tool in DebugTool::ALL {
                if ui
                    .add(egui::Button::selectable(selected == tool, tool.title()))
                    .clicked()
                {
                    selected = tool;
                }
            }
        });
    });
    egui::CentralPanel::default().show(&mut viewport_ui, |ui| match selected {
        DebugTool::Diagnostics => {
            let diagnostics = world.resource::<DiagnosticsStore>();
            diagnostics_grid(ui, diagnostics);
        }
        DebugTool::WorldInspector => {
            egui::ScrollArea::both().show(ui, |ui| bevy_inspector::ui_for_world(world, ui));
        }
    });
    if selected != tab {
        world.resource_mut::<ShowDebug>().tab = selected;
    }

    Ok(())
}

fn diagnostics_grid(ui: &mut egui::Ui, diagnostics: &DiagnosticsStore) {
    egui::Grid::new("diagnostics_grid")
        .num_columns(2)
        .show(ui, |ui| {
            for diagnostic in diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.is_enabled)
            {
                let Some(value) = diagnostic.smoothed() else {
                    continue;
                };
                ui.label(diagnostic.path().to_string());
                match diagnostic.average() {
                    Some(average) => ui.label(format!(
                        "{value:.2}{} (avg {average:.2})",
                        diagnostic.suffix
                    )),
                    None => ui.label(format!("{value:.2}{}", diagnostic.suffix)),
                };
                ui.end_row();
            }
        });
}

/// Spawns the debug OS window with its own egui context, maximized.
fn open_debug_window(commands: &mut Commands, show_debug: &mut ShowDebug) {
    let mut window = Window {
        title: "Debug".into(),
        // Size restored when the window is un-maximized.
        resolution: WindowResolution::new(960, 600),
        ..default()
    };
    window.set_maximized(true);
    let window = commands.spawn(window).id();

    let camera = commands
        .spawn((
            Camera2d,
            Camera {
                order: 0,
                ..default()
            },
            // The game's entities live on render layer 0; render only the
            // egui context in this window.
            RenderLayers::none(),
            RenderTarget::Window(WindowRef::Entity(window)),
            EguiContext::default(),
            EguiSchedule::new(DebugWindowPass),
        ))
        .id();

    show_debug.window = Some(DebugWindow { window, camera });
}

fn close_debug_window(commands: &mut Commands, show_debug: &mut ShowDebug, window: DebugWindow) {
    // Despawn the camera (which owns the egui context) before the window it
    // targets.
    commands.entity(window.camera).despawn();
    commands.entity(window.window).despawn();
    show_debug.window = None;
}
