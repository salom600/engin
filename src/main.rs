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

use bevy::{log::LogPlugin, prelude::*};
use bevy_egui::EguiPlugin;

fn main() {
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
                    primary_window: Some(Window {
                        title: "Bevy Editor".to_string(),
                        resolution: (1600.0, 900.0).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(EguiPlugin::default())
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(editor::EditorPlugin)
        .run();
}
