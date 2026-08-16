//! bevy_editor — a visual engine interface (scene editor) for Bevy.
//!
//! Layout: top menu bar + toolbar, hierarchy (left), inspector (right),
//! assets + console (bottom dock), status bar, and a central Game View that
//! renders the scene into a texture displayed by egui.

mod components;
mod demo;
mod editor;
mod log_layer;
mod scene_io;
mod settings;

use bevy::{log::LogPlugin, prelude::*};
use bevy_egui::EguiPlugin;

fn main() {
    // Restore the window exactly as the user left it (size + position).
    let saved = settings::load();
    let window = match &saved {
        Some(s) => Window {
            title: "Bevy Editor".to_string(),
            resolution: (s.window.width, s.window.height).into(),
            position: s
                .window
                .position
                .map(|(x, y)| bevy::window::WindowPosition::At(IVec2::new(x, y))),
            ..default()
        },
        None => Window {
            title: "Bevy Editor".to_string(),
            resolution: (1600.0_f32, 900.0_f32).into(),
            ..default()
        },
    };

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(LogPlugin {
                    filter: "info,bevy_editor=debug,wgpu=error,naga=warn".to_string(),
                    level: bevy::log::Level::DEBUG,
                    custom_layer: log_layer::editor_log_layer,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(window),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin {
            enable_multipass_for_primary_context: false,
        })
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(editor::EditorPlugin)
        .run();
}
