use bevy::app::App;
use bevy::diagnostic::{
    DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
    SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

// Collects diagnostics and displays them in an egui window toggled with F1.
// This keeps the console output clear for warnings, errors, and important
// event logging.

/// Whether the diagnostics window is currently visible.
#[derive(Resource, Default)]
pub(crate) struct ShowDiagnostics(bool);

pub(crate) fn plugin(app: &mut App) {
    app.add_plugins((
        EguiPlugin::default(),
        FrameTimeDiagnosticsPlugin::default(),
        EntityCountDiagnosticsPlugin::default(),
        SystemInformationDiagnosticsPlugin,
    ))
    .init_resource::<ShowDiagnostics>()
    .add_systems(Update, toggle_diagnostics_window)
    .add_systems(EguiPrimaryContextPass, show_diagnostics_window);
}

fn toggle_diagnostics_window(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut show_diagnostics: ResMut<ShowDiagnostics>,
) {
    if keyboard.just_pressed(KeyCode::F1) {
        show_diagnostics.0 = !show_diagnostics.0;
    }
}

fn show_diagnostics_window(
    mut contexts: EguiContexts,
    mut show_diagnostics: ResMut<ShowDiagnostics>,
    diagnostics: Res<DiagnosticsStore>,
) -> Result {
    if !show_diagnostics.0 {
        return Ok(());
    }

    // Binding the open state to the window keeps the resource in sync when it
    // is closed through the window's close button.
    let mut open = show_diagnostics.0;
    egui::Window::new("Diagnostics")
        .open(&mut open)
        .show(contexts.ctx_mut()?, |ui| {
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
        });
    show_diagnostics.0 = open;

    Ok(())
}
