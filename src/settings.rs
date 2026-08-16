//! Persistent editor settings: window geometry, panel layout toggles, camera
//! framing and the full egui layout memory (splitter positions, panel sizes).
//!
//! Two files are kept next to the executable (portable layout):
//! - `editor_settings.ron`       — window size/position + editor preferences
//! - `editor_egui_memory.ron`    — serialized `egui::Memory` (all panel widths)
//!
//! They are written every few seconds while running and on window close, and
//! re-applied on the next launch so the editor always comes back exactly the
//! way the user left it.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const SETTINGS_FILE: &str = "editor_settings.ron";
const EGUI_MEMORY_FILE: &str = "editor_egui_memory.ron";

/// How often the layout is flushed to disk while the editor is running.
pub const AUTOSAVE_SECS: f32 = 3.0;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct WindowSettings {
    pub width: f32,
    pub height: f32,
    /// Last known window position in physical pixels, if we ever saw one.
    pub position: Option<(i32, i32)>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LayoutSettings {
    pub window: WindowSettings,
    pub hierarchy: bool,
    pub inspector: bool,
    pub assets: bool,
    pub console: bool,
    pub show_grid: bool,
    pub show_selection_gizmo: bool,
    pub show_light_gizmos: bool,
    pub orbit_target: [f32; 3],
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        Self {
            window: WindowSettings {
                width: 1600.0,
                height: 900.0,
                position: None,
            },
            hierarchy: true,
            inspector: true,
            assets: true,
            console: true,
            show_grid: true,
            show_selection_gizmo: true,
            show_light_gizmos: true,
            orbit_target: [0.0, 0.8, 0.0],
            orbit_yaw: 0.9,
            orbit_pitch: 0.55,
            orbit_distance: 11.0,
        }
    }
}

pub fn load() -> Option<LayoutSettings> {
    let text = std::fs::read_to_string(SETTINGS_FILE).ok()?;
    let settings: LayoutSettings = ron::de::from_str(&text).ok()?;
    // Sanity-clamp the restored window so a stale/off-screen geometry can
    // never make the editor unreachable.
    let w = settings.window.width.clamp(400.0, 7680.0);
    let h = settings.window.height.clamp(300.0, 4320.0);
    let mut settings = settings;
    settings.window.width = w;
    settings.window.height = h;
    Some(settings)
}

pub fn save(settings: &LayoutSettings) {
    if let Ok(text) = ron::ser::to_string_pretty(settings, Default::default()) {
        let _ = std::fs::write(SETTINGS_FILE, text);
    }
}

pub fn load_egui_memory() -> Option<bevy_egui::egui::Memory> {
    let text = std::fs::read_to_string(EGUI_MEMORY_FILE).ok()?;
    ron::de::from_str(&text).ok()
}

pub fn save_egui_memory(memory: &bevy_egui::egui::Memory) {
    if let Ok(text) = ron::ser::to_string_pretty(memory, Default::default()) {
        let _ = std::fs::write(EGUI_MEMORY_FILE, text);
    }
}
