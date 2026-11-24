use bevy::app::App;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::window::{
    MonitorSelection, WindowCloseRequested, WindowPosition, WindowRef, WindowResolution,
};
use bevy_egui::{EguiContext, EguiContexts, EguiGlobalSettings, EguiPlugin, EguiSchedule, egui};
use bevy_remote::{RemotePlugin, http::RemoteHttpPlugin};

// Collects diagnostics and displays them in an egui window toggled with F1.
// This keeps the console output clear for warnings, errors, and important
// event logging.
//
// Debug UI never renders inside the main viewport: every debug tool draws
// into a separate OS "Debug" window with its own egui context. F1 opens the
// window when the first tool becomes visible; closing it hides every tool.
//
// To add a future debug tool: give `ShowDebug` a visibility flag (include it
// in `any_visible` and `hide_all`), toggle it from an `Update` system, and
// draw its egui window from a system on `DebugWindowPass`.

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

/// Which debug tools are currently visible, plus the entities of the OS
/// window hosting them.
#[derive(Resource, Default)]
pub(crate) struct ShowDebug {
    diagnostics: bool,
    window: Option<DebugWindow>,
}

impl ShowDebug {
    /// Whether any debug tool wants the debug window open.
    fn any_visible(&self) -> bool {
        self.diagnostics
    }

    /// Hides every debug tool; called when the debug window is closed.
    fn hide_all(&mut self) {
        self.diagnostics = false;
    }
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
            toggle_diagnostics_window,
            handle_debug_window_close,
            sync_debug_window,
        )
            .chain(),
    )
    .add_systems(DebugWindowPass, show_diagnostics_window);
}

/// F1 toggles the diagnostics tool.
fn toggle_diagnostics_window(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut show_debug: ResMut<ShowDebug>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        show_debug.diagnostics = !show_debug.diagnostics;
    }
}

/// Hides every debug tool when the debug window is closed through the OS
/// close button.
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
            show_debug.hide_all();
        }
    }
}

/// Keeps the debug window open exactly while a debug tool is visible.
///
/// This runs in `Update` on purpose: the egui pass loop reads back the
/// context entity after its schedule completes, so the context must not be
/// despawned from within `DebugWindowPass`. Flag changes made there (e.g.
/// through an egui close button) take effect on the next frame.
fn sync_debug_window(mut commands: Commands, mut show_debug: ResMut<ShowDebug>) {
    match (show_debug.any_visible(), show_debug.window) {
        (true, None) => open_debug_window(&mut commands, &mut show_debug),
        (false, Some(debug_window)) => {
            close_debug_window(&mut commands, &mut show_debug, debug_window)
        }
        _ => {}
    }
}

/// Draws the diagnostics egui window inside the debug OS window.
fn show_diagnostics_window(
    mut show_debug: ResMut<ShowDebug>,
    mut contexts: EguiContexts,
    diagnostics: Res<DiagnosticsStore>,
) -> Result {
    let Some(debug_window) = show_debug.window else {
        return Ok(());
    };
    if !show_debug.diagnostics {
        return Ok(());
    }

    let ctx = contexts.ctx_for_entity_mut(debug_window.camera)?;
    let mut open = true;
    egui::Window::new("Diagnostics")
        .open(&mut open)
        .show(ctx, |ui| diagnostics_grid(ui, &diagnostics));
    if !open {
        // Closed through the egui window's close button; the sync system
        // closes the OS window once no tool is visible anymore.
        show_debug.diagnostics = false;
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

/// Spawns the debug OS window with its own egui context.
///
/// Note: on Wayland, compositor policy decides the placement of new windows,
/// so the requested centered position may be ignored.
fn open_debug_window(commands: &mut Commands, show_debug: &mut ShowDebug) {
    let window = commands
        .spawn((Window {
            title: "Debug".into(),
            position: WindowPosition::Centered(MonitorSelection::Current),
            resolution: WindowResolution::new(480, 320),
            ..default()
        },))
        .id();

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
