//! All editor UI panels (egui).

use crate::components::*;
use crate::editor::{CompKind, EditorRequest, EditorState, Panels, PlayState, Tool};
use crate::log_layer;
use crate::{demo, scene_io};
use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use bevy::window::Window;
use bevy_egui::{egui, EguiContexts};
use bevy::math::EulerRot;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Layout guarantees
//
// The Game View lives in the root `CentralPanel`, which egui always sizes to
// the space left over after every side/top/bottom panel. To make sure the
// panels can never squeeze it to zero, each resizable panel is clamped with a
// `width_range`/`height_range` computed from the *current* sizes of the other
// panels, reserving the minimums below for the central area.
// ---------------------------------------------------------------------------

/// Minimum guaranteed width of the central Game View.
const CENTRAL_MIN_WIDTH: f32 = 200.0;
/// Minimum guaranteed height of the central Game View.
const CENTRAL_MIN_HEIGHT: f32 = 200.0;
/// Space used by the fixed menu bar + toolbar + status bar (plus margins).
const BARS_RESERVE: f32 = 100.0;
const HIERARCHY_MIN_WIDTH: f32 = 180.0;
const INSPECTOR_MIN_WIDTH: f32 = 220.0;
const DOCK_MIN_HEIGHT: f32 = 80.0;
const DOCK_ASSETS_MIN_WIDTH: f32 = 160.0;

// ---------------------------------------------------------------------------
// File dialog helpers
// ---------------------------------------------------------------------------

fn dialog_dir(sub: &str) -> PathBuf {
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let dir = base.join(sub);
    if dir.is_dir() {
        dir
    } else {
        base
    }
}

pub fn do_save(state: &mut EditorState, commands: &mut Commands, force_as: bool) {
    let path = if force_as {
        None
    } else {
        state.scene_path.clone()
    };
    let path = path.or_else(|| {
        rfd::FileDialog::new()
            .add_filter("Bevy scene (*.scn.ron)", &["ron"])
            .set_directory(dialog_dir("assets/scenes"))
            .set_file_name("scene.scn.ron")
            .save_file()
    });
    if let Some(path) = path {
        state.scene_path = Some(path.clone());
        state.scene_dirty = false;
        commands.queue(move |world: &mut World| match scene_io::save_scene(world, &path) {
            Ok(()) => info!("Scene saved to {}", path.display()),
            Err(e) => error!("Save failed: {e}"),
        });
    }
}

pub fn do_open(state: &mut EditorState, commands: &mut Commands) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Bevy scene (*.scn.ron)", &["ron"])
        .set_directory(dialog_dir("assets"))
        .pick_file()
    {
        state.scene_path = Some(path.clone());
        state.scene_dirty = false;
        state.selected = None;
        state.play = PlayState::Stopped;
        state.playback_snapshot = None;
        commands.queue(move |world: &mut World| match scene_io::load_scene(world, &path) {
            Ok(n) => info!("Scene loaded: {n} entities from {}", path.display()),
            Err(e) => error!("Load failed: {e}"),
        });
    }
}

fn do_new_scene(state: &mut EditorState, commands: &mut Commands, spawner: fn(&mut World)) {
    state.selected = None;
    state.scene_path = None;
    state.scene_dirty = false;
    state.play = PlayState::Stopped;
    state.playback_snapshot = None;
    commands.queue(move |world: &mut World| {
        scene_io::clear_scene(world);
        spawner(world);
    });
}

// ---------------------------------------------------------------------------
// Menu bar
// ---------------------------------------------------------------------------

pub fn menu_bar(mut contexts: EguiContexts, mut state: ResMut<EditorState>, mut commands: Commands) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    if !state.theme_applied {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(30, 32, 38);
        visuals.extreme_bg_color = egui::Color32::from_rgb(22, 23, 28);
        visuals.faint_bg_color = egui::Color32::from_rgb(35, 37, 44);
        visuals.selection.bg_fill = egui::Color32::from_rgb(233, 116, 21);
        visuals.selection.stroke.color = egui::Color32::BLACK;
        visuals.hyperlink_color = egui::Color32::from_rgb(126, 182, 255);
        ctx.set_visuals(visuals);
        state.theme_applied = true;
    }

    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    do_new_scene(&mut *state, &mut commands, demo::spawn_default_scene);
                    ui.close_menu();
                }
                if ui.button("Open Scene...  Ctrl+O").clicked() {
                    do_open(&mut *state, &mut commands);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Save Scene  Ctrl+S").clicked() {
                    do_save(&mut *state, &mut commands, false);
                    ui.close_menu();
                }
                if ui.button("Save Scene As...").clicked() {
                    do_save(&mut *state, &mut commands, true);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Load Demo Scene").clicked() {
                    do_new_scene(&mut *state, &mut commands, demo::spawn_demo_world);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    commands.send_event(AppExit::Success);
                    ui.close_menu();
                }
            });

            ui.menu_button("Edit", |ui| {
                ui.add_enabled(false, egui::Button::new("Undo  (not implemented)"));
                ui.add_enabled(false, egui::Button::new("Redo  (not implemented)"));
                ui.separator();
                let has_selection = state.selected.is_some();
                if ui.add_enabled(has_selection, egui::Button::new("Duplicate  Ctrl+D")).clicked() {
                    if let Some(e) = state.selected {
                        state.request = Some(EditorRequest::Duplicate(e));
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(has_selection, egui::Button::new("Delete  Del")).clicked() {
                    if let Some(e) = state.selected {
                        state.request = Some(EditorRequest::Delete(e));
                    }
                    ui.close_menu();
                }
                if ui.add_enabled(has_selection, egui::Button::new("Focus Selection  F")).clicked() {
                    if let Some(e) = state.selected {
                        state.focus_request = Some(e);
                    }
                    ui.close_menu();
                }
            });

            ui.menu_button("Create", |ui| {
                create_menu(ui, &mut *state, EditorRequest::Spawn);
            });

            ui.menu_button("View", |ui| {
                ui.menu_button("Panels", |ui| {
                    panel_toggle(ui, &mut state.panels);
                });
                ui.separator();
                ui.checkbox(&mut state.show_grid, "Show Grid");
                ui.checkbox(&mut state.show_selection_gizmo, "Selection Outline");
                ui.checkbox(&mut state.show_light_gizmos, "Light Markers");
            });

            ui.menu_button("Help", |ui| {
                if ui.button("About / Controls").clicked() {
                    state.about_open = true;
                    ui.close_menu();
                }
                ui.hyperlink_to("Bevy engine", "https://bevyengine.org");
                ui.hyperlink_to("bevy_egui", "https://github.com/vladbat00/bevy_egui");
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if state.scene_dirty {
                    ui.colored_label(egui::Color32::from_rgb(240, 200, 90), "unsaved changes");
                }
                ui.weak("bevy 0.16.1 · bevy_egui 0.34.1");
            });
        });
    });
}

fn create_menu(ui: &mut egui::Ui, state: &mut EditorState, make: impl Fn(SpawnKind) -> EditorRequest) {
    if ui.button("Empty Entity").clicked() {
        state.request = Some(make(SpawnKind::Empty));
        ui.close_menu();
    }
    ui.separator();
    ui.label(egui::RichText::new("3D Object").weak());
    for shape in PrimitiveShape::ALL {
        if ui.button(shape.label()).clicked() {
            state.request = Some(make(SpawnKind::Shape(shape)));
            ui.close_menu();
        }
    }
    ui.separator();
    ui.label(egui::RichText::new("Light").weak());
    if ui.button("Directional Light").clicked() {
        state.request = Some(make(SpawnKind::DirectionalLight));
        ui.close_menu();
    }
    if ui.button("Point Light").clicked() {
        state.request = Some(make(SpawnKind::PointLight));
        ui.close_menu();
    }
    if ui.button("Spot Light").clicked() {
        state.request = Some(make(SpawnKind::SpotLight));
        ui.close_menu();
    }
}

fn panel_toggle(ui: &mut egui::Ui, panels: &mut Panels) {
    ui.checkbox(&mut panels.hierarchy, "Hierarchy");
    ui.checkbox(&mut panels.inspector, "Inspector");
    ui.checkbox(&mut panels.assets, "Assets");
    ui.checkbox(&mut panels.console, "Console");
}

// ---------------------------------------------------------------------------
// Toolbar
// ---------------------------------------------------------------------------

pub fn toolbar(mut contexts: EguiContexts, mut state: ResMut<EditorState>) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            let playing = state.play != PlayState::Stopped;

            let play_label = match state.play {
                PlayState::Stopped => "Play",
                PlayState::Playing => "Playing",
                PlayState::Paused => "Resume",
            };
            let play_fill = if state.play == PlayState::Playing {
                egui::Color32::from_rgb(38, 110, 52)
            } else {
                egui::Color32::from_rgb(24, 82, 38)
            };
            if ui.add(egui::Button::new(play_label).fill(play_fill)).clicked() {
                state.request = Some(if state.play == PlayState::Stopped {
                    EditorRequest::Play
                } else {
                    EditorRequest::TogglePause
                });
            }
            if state.play == PlayState::Playing
                && ui
                    .add(egui::Button::new("Pause").fill(egui::Color32::from_rgb(110, 92, 24)))
                    .clicked()
            {
                state.request = Some(EditorRequest::TogglePause);
            }
            if ui
                .add_enabled(
                    playing,
                    egui::Button::new("Stop").fill(egui::Color32::from_rgb(122, 40, 40)),
                )
                .clicked()
            {
                state.request = Some(EditorRequest::Stop);
            }

            ui.separator();

            for (tool, label, key) in [
                (Tool::Select, "Select", "Q"),
                (Tool::Move, "Move", "W"),
                (Tool::Rotate, "Rotate", "E"),
                (Tool::Scale, "Scale", "R"),
            ] {
                if ui
                    .selectable_label(state.tool == tool, format!("{label} ({key})"))
                    .clicked()
                {
                    state.tool = tool;
                }
            }
            if state.tool != Tool::Select {
                ui.weak("drag in Game View to transform · hold Ctrl to snap");
            }

            ui.separator();
            ui.checkbox(&mut state.show_grid, "Grid");
            ui.checkbox(&mut state.show_selection_gizmo, "Outline");
            ui.checkbox(&mut state.show_light_gizmos, "Lights");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(match state.play {
                    PlayState::Stopped => "Editor mode",
                    PlayState::Playing => "PLAY MODE",
                    PlayState::Paused => "PAUSED",
                });
            });
        });
        ui.add_space(2.0);
    });

    // About / controls window.
    egui::Window::new("About Bevy Editor")
        .open(&mut state.about_open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.heading("Bevy Editor");
            ui.label("A visual scene editor built on the Bevy engine + bevy_egui.");
            ui.add_space(6.0);
            ui.heading("Controls");
            ui.monospace(
                "Right mouse drag : orbit camera\n\
                 Middle mouse drag: pan camera\n\
                 Mouse wheel      : zoom\n\
                 Left click       : select entity\n\
                 F                : focus selection\n\
                 Q/W/E/R          : select / move / rotate / scale tool\n\
                 Drag (W/E/R)     : transform entity, Ctrl = snap\n\
                 Del              : delete selection\n\
                 Ctrl+D           : duplicate\n\
                 Ctrl+S / Ctrl+O  : save / open scene\n\
                 Space            : play / pause",
            );
            ui.add_space(6.0);
            ui.heading("Workflow");
            ui.label("1. Create entities (Create menu or hierarchy context menu).");
            ui.label("2. Edit components in the Inspector; add more via 'Add Component'.");
            ui.label("3. Press Play to test (Rotator/Bobber components animate).");
            ui.label("4. Press Stop to restore the scene, then save with Ctrl+S.");
        });
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

pub fn status_bar(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    q_names: Query<&Name>,
    q_all_entities: Query<Entity>,
    diagnostics: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
    adapter: Option<Res<RenderAdapterInfo>>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    egui::TopBottomPanel::bottom("status_bar")
        .exact_height(20.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.weak(
                    "RMB drag: orbit · MMB drag: pan · Wheel: zoom · Click: select · F: focus · Ctrl+S: save",
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(adapter) = adapter {
                        ui.weak(format!("GPU: {}", adapter.0.name));
                        ui.separator();
                    }
                    if let Some(selected) = state.selected {
                        let name = q_names
                            .get(selected)
                            .map(|n| n.as_str().to_string())
                            .unwrap_or_else(|_| "unnamed".into());
                        ui.label(format!("Selected: {name}"));
                        ui.separator();
                    }
                    ui.label(format!("{} entities", q_all_entities.iter().count()));
                    ui.separator();
                    let fps = diagnostics
                        .as_deref()
                        .and_then(|d| {
                            d.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
                                .and_then(|diag| diag.smoothed())
                        })
                        .map(|v| format!("{v:.0} FPS"))
                        .unwrap_or_else(|| "- FPS".into());
                    ui.strong(fps);
                });
            });
        });
}

// ---------------------------------------------------------------------------
// Hierarchy
// ---------------------------------------------------------------------------

type HierarchyQuery<'w, 's, 'a> = Query<'w, 's, (
    Entity,
    Option<&'a Name>,
    Option<&'a Children>,
    Has<ChildOf>,
    Has<EditorInternal>,
    Has<Window>,
    Option<&'a PrimitiveMesh>,
    Has<PointLight>,
    Has<DirectionalLight>,
    Has<SpotLight>,
    Option<&'a Visibility>,
)>;

pub fn hierarchy_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    mut commands: Commands,
    q: HierarchyQuery,
    q_count: Query<Entity, With<SceneEntity>>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    // Never let this panel (plus the inspector's current width) squeeze the
    // central Game View below CENTRAL_MIN_WIDTH.
    let screen_width = ctx.screen_rect().width();
    let inspector = if state.panels.inspector {
        state.inspector_width
    } else {
        0.0
    };
    let max_width =
        (screen_width - CENTRAL_MIN_WIDTH - inspector).max(HIERARCHY_MIN_WIDTH);
    let panel = egui::SidePanel::left("hierarchy_panel")
        .resizable(true)
        .width_range(HIERARCHY_MIN_WIDTH..=max_width)
        .default_width(270.0)
        .show(ctx, |ui| {
            ui.heading("Hierarchy");
            ui.horizontal(|ui| {
                ui.menu_button("+ Add", |ui| {
                    create_menu(ui, &mut *state, EditorRequest::Spawn);
                });
                ui.add(
                    egui::TextEdit::singleline(&mut state.hierarchy_filter)
                        .hint_text("Search...")
                        .desired_width(120.0),
                );
            });
            ui.separator();

            let filter = state.hierarchy_filter.trim().to_lowercase();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if filter.is_empty() {
                        for item in q.iter() {
                            let (entity, _, _, has_parent, internal, window, ..) = item;
                            if !has_parent && !internal && !window {
                                entity_node(ui, entity, &mut *state, &mut commands, &q);
                            }
                        }
                    } else {
                        for item in q.iter() {
                            let (entity, name, _, _, internal, window, ..) = item;
                            if internal || window {
                                continue;
                            }
                            let label = entity_label(entity, name);
                            if label.to_lowercase().contains(&filter) {
                                let selected = state.selected == Some(entity);
                                if ui.selectable_label(selected, label).clicked() {
                                    state.selected = Some(entity);
                                }
                            }
                        }
                    }
                });

            ui.separator();
            ui.weak(format!("{} scene entities", q_count.iter().count()));
        });
    state.hierarchy_width = panel.response.rect.width();
}

fn entity_label(entity: Entity, name: Option<&Name>) -> String {
    name.map(|n| n.as_str().to_string())
        .unwrap_or_else(|| format!("Entity {}", entity.index()))
}

fn entity_node(
    ui: &mut egui::Ui,
    entity: Entity,
    state: &mut EditorState,
    commands: &mut Commands,
    q: &HierarchyQuery,
) {
    let Ok((e, name, children, _has_parent, _int, _win, mesh, pl, dl, sl, vis)) = q.get(entity)
    else {
        return;
    };

    let label = entity_label(e, name);
    let is_selected = state.selected == Some(e);
    let renaming = state.rename_target == Some(e);

    let dot_color = if mesh.is_some() {
        egui::Color32::from_rgb(96, 165, 230)
    } else if pl || dl || sl {
        egui::Color32::from_rgb(230, 190, 70)
    } else {
        egui::Color32::from_gray(150)
    };

    let child_ids: Vec<Entity> = children.map(|c| c.to_vec()).unwrap_or_default();
    let has_children = !child_ids.is_empty();
    let id = egui::Id::new(("hier", e));
    let mut node_state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        false,
    );

    ui.horizontal(|ui| {
        if has_children {
            let open = node_state.is_open();
            let label = if open { "-" } else { "+" };
            if ui.add(egui::Button::new(label).small()).clicked() {
                node_state.toggle(ui);
            }
        } else {
            ui.add_space(22.0);
        }

        // Visibility toggle
        let hidden = matches!(vis, Some(Visibility::Hidden));
        let mut shown = !hidden;
        ui.checkbox(&mut shown, "");
        if shown == hidden {
            commands.entity(e).insert(if shown {
                Visibility::Visible
            } else {
                Visibility::Hidden
            });
        }

        ui.label(egui::RichText::new("●").color(dot_color).small());

        if renaming {
            let te = ui.add(
                egui::TextEdit::singleline(&mut state.rename_buf).desired_width(110.0),
            );
            if te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                let new_name = std::mem::take(&mut state.rename_buf);
                commands.entity(e).insert(Name::new(new_name));
                state.rename_target = None;
            }
        } else {
            let response = ui.selectable_label(is_selected, &label);
            if response.clicked() {
                state.selected = Some(e);
            }
            context_menu(ui, &response, e, state);
        }
    });

    if has_children && node_state.is_open() {
        node_state.show_body_unindented(ui, |ui| {
            for child in child_ids {
                entity_node(ui, child, state, commands, q);
            }
        });
    }
}

fn context_menu(
    _ui: &mut egui::Ui,
    response: &egui::Response,
    entity: Entity,
    state: &mut EditorState,
) {
    response.context_menu(|ui| {
        let is_selected = state.selected == Some(entity);
        ui.menu_button("Create Child", |ui| {
            create_menu(ui, &mut *state, |kind| EditorRequest::SpawnChild(entity, kind));
        });
        ui.separator();
        if ui.button("Focus").clicked() {
            state.focus_request = Some(entity);
            state.selected = Some(entity);
            ui.close_menu();
        }
        if ui.button("Rename").clicked() {
            state.rename_target = Some(entity);
            ui.close_menu();
        }
        if ui.button("Duplicate").clicked() {
            state.request = Some(EditorRequest::Duplicate(entity));
            ui.close_menu();
        }
        if ui
            .add_enabled(!is_selected, egui::Button::new("Attach to Selected"))
            .clicked()
        {
            state.request = Some(EditorRequest::Reparent {
                child: entity,
                new_parent: state.selected,
            });
            ui.close_menu();
        }
        if ui.button("Detach to Root").clicked() {
            state.request = Some(EditorRequest::Reparent {
                child: entity,
                new_parent: None,
            });
            ui.close_menu();
        }
        ui.separator();
        if ui
            .add(
                egui::Button::new("Delete")
                    .fill(egui::Color32::from_rgb(122, 40, 40)),
            )
            .clicked()
        {
            state.request = Some(EditorRequest::Delete(entity));
            ui.close_menu();
        }
    });
}

// ---------------------------------------------------------------------------
// Inspector
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
pub fn inspector_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    mut commands: Commands,
    mut q_names: Query<&mut Name>,
    mut q_transforms: Query<&mut Transform>,
    mut q_vis: Query<&mut Visibility>,
    mut q_meshes: Query<&mut PrimitiveMesh>,
    mut q_pbr: Query<&mut PbrDef>,
    mut q_dl: Query<&mut DirectionalLight>,
    mut q_pl: Query<&mut PointLight>,
    mut q_sl: Query<&mut SpotLight>,
    mut q_rot: Query<&mut Rotator>,
    mut q_bob: Query<&mut Bobber>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    // Never let this panel (plus the hierarchy's current width) squeeze the
    // central Game View below CENTRAL_MIN_WIDTH.
    let screen_width = ctx.screen_rect().width();
    let hierarchy = if state.panels.hierarchy {
        state.hierarchy_width
    } else {
        0.0
    };
    let max_width =
        (screen_width - CENTRAL_MIN_WIDTH - hierarchy).max(INSPECTOR_MIN_WIDTH);
    let panel = egui::SidePanel::right("inspector_panel")
        .resizable(true)
        .width_range(INSPECTOR_MIN_WIDTH..=max_width)
        .default_width(330.0)
        .show(ctx, |ui| {
            ui.heading("Inspector");
            let Some(sel) = state.selected else {
                ui.add_space(10.0);
                ui.weak("Nothing selected.");
                ui.weak("Click an entity in the Game View or the Hierarchy.");
                return;
            };

            // Name (sync buffer when selection changes)
            if state.name_buf_entity != Some(sel) {
                state.name_buf = q_names
                    .get(sel)
                    .map(|n| n.as_str().to_string())
                    .unwrap_or_default();
                state.name_buf_entity = Some(sel);
            }
            ui.horizontal(|ui| {
                ui.label("Name:");
                let te = ui.add(
                    egui::TextEdit::singleline(&mut state.name_buf).desired_width(f32::INFINITY),
                );
                if te.changed() {
                    let name = Name::new(state.name_buf.clone());
                    commands.entity(sel).insert(name);
                    state.scene_dirty = true;
                }
            });
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                let hidden = matches!(q_vis.get(sel), Ok(Visibility::Hidden));
                let mut shown = !hidden;
                ui.checkbox(&mut shown, "Visible");
                if shown == hidden {
                    commands.entity(sel).insert(if shown {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    });
                }
                if ui.button("Focus").clicked() {
                    state.focus_request = Some(sel);
                }
                if ui.button("Duplicate").clicked() {
                    state.request = Some(EditorRequest::Duplicate(sel));
                }
                if ui
                    .add(egui::Button::new("Delete").fill(egui::Color32::from_rgb(122, 40, 40)))
                    .clicked()
                {
                    state.request = Some(EditorRequest::Delete(sel));
                }
            });
            ui.add_space(4.0);
            ui.separator();
            ui.strong(format!("Entity #{}", sel.index()));

            // Transform
            if q_transforms.get(sel).is_ok() {
                egui::CollapsingHeader::new("Transform")
                    .id_salt("xform")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut t) = q_transforms.get_mut(sel) else {
                            return;
                        };
                        transform_editor(ui, &mut t);
                        if ui.button("Reset").clicked() {
                            *t = Transform::IDENTITY;
                            state.scene_dirty = true;
                        }
                    });
            }

            // Primitive mesh
            if q_meshes.get(sel).is_ok() {
                egui::CollapsingHeader::new("Mesh (primitive)")
                    .id_salt("mesh")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut pm) = q_meshes.get_mut(sel) else {
                            return;
                        };
                        ui.horizontal(|ui| {
                            ui.label("Shape:");
                            egui::ComboBox::from_id_salt("shape_select")
                                .selected_text(pm.shape.label())
                                .show_ui(ui, |ui| {
                                    for shape in PrimitiveShape::ALL {
                                        ui.selectable_value(&mut pm.shape, shape, shape.label());
                                    }
                                });
                        });
                        remove_button(ui, &mut *state, sel, CompKind::PrimitiveMesh);
                    });
            }

            // PBR material
            if q_pbr.get(sel).is_ok() {
                egui::CollapsingHeader::new("Material (PBR)")
                    .id_salt("pbr")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut def) = q_pbr.get_mut(sel) else {
                            return;
                        };
                        color_row(ui, "Base Color", &mut def.base_color);
                        color_row(ui, "Emissive", &mut def.emissive);
                        slider_row(ui, "Metallic", &mut def.metallic, 0.0..=1.0);
                        slider_row(ui, "Roughness", &mut def.perceptual_roughness, 0.0..=1.0);
                        ui.checkbox(&mut def.unlit, "Unlit");
                        ui.checkbox(&mut def.double_sided, "Double-sided");
                        remove_button(ui, &mut *state, sel, CompKind::PbrDef);
                    });
            }

            // Lights
            if q_dl.get(sel).is_ok() {
                egui::CollapsingHeader::new("Directional Light")
                    .id_salt("dlight")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut light) = q_dl.get_mut(sel) else {
                            return;
                        };
                        color_row(ui, "Color", &mut light.color);
                        slider_row(ui, "Illuminance (lux)", &mut light.illuminance, 0.0..=120_000.0);
                        ui.checkbox(&mut light.shadows_enabled, "Shadows");
                        remove_button(ui, &mut *state, sel, CompKind::DirectionalLight);
                    });
            }
            if q_pl.get(sel).is_ok() {
                egui::CollapsingHeader::new("Point Light")
                    .id_salt("plight")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut light) = q_pl.get_mut(sel) else {
                            return;
                        };
                        color_row(ui, "Color", &mut light.color);
                        slider_row(ui, "Intensity (lm)", &mut light.intensity, 0.0..=1_000_000.0);
                        slider_row(ui, "Range (m)", &mut light.range, 0.0..=100.0);
                        slider_row(ui, "Radius (m)", &mut light.radius, 0.0..=10.0);
                        ui.checkbox(&mut light.shadows_enabled, "Shadows");
                        remove_button(ui, &mut *state, sel, CompKind::PointLight);
                    });
            }
            if q_sl.get(sel).is_ok() {
                egui::CollapsingHeader::new("Spot Light")
                    .id_salt("slight")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut light) = q_sl.get_mut(sel) else {
                            return;
                        };
                        color_row(ui, "Color", &mut light.color);
                        slider_row(ui, "Intensity (lm)", &mut light.intensity, 0.0..=1_000_000.0);
                        slider_row(ui, "Range (m)", &mut light.range, 0.0..=100.0);
                        let mut outer = light.outer_angle.to_degrees();
                        slider_row(ui, "Outer angle (°)", &mut outer, 1.0..=89.0);
                        light.outer_angle = outer.to_radians();
                        let mut inner = light.inner_angle.to_degrees();
                        slider_row(ui, "Inner angle (°)", &mut inner, 0.0..=89.0);
                        light.inner_angle = inner.to_radians();
                        ui.checkbox(&mut light.shadows_enabled, "Shadows");
                        remove_button(ui, &mut *state, sel, CompKind::SpotLight);
                    });
            }

            // Gameplay
            if q_rot.get(sel).is_ok() {
                egui::CollapsingHeader::new("Rotator (gameplay)")
                    .id_salt("rot")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut rot) = q_rot.get_mut(sel) else {
                            return;
                        };
                        slider_row(ui, "Speed (°/s)", &mut rot.speed_deg_per_sec, -360.0..=360.0);
                        ui.label("Axis:");
                        vec3_drag(ui, &mut rot.axis, 0.02, "axis");
                        remove_button(ui, &mut *state, sel, CompKind::Rotator);
                    });
            }
            if q_bob.get(sel).is_ok() {
                egui::CollapsingHeader::new("Bobber (gameplay)")
                    .id_salt("bob")
                    .default_open(true)
                    .show(ui, |ui| {
                        let Ok(mut bob) = q_bob.get_mut(sel) else {
                            return;
                        };
                        slider_row(ui, "Amplitude (m)", &mut bob.amplitude, 0.0..=5.0);
                        slider_row(ui, "Frequency (Hz)", &mut bob.frequency, 0.0..=3.0);
                        slider_row(ui, "Phase", &mut bob.phase, 0.0..=std::f32::consts::TAU);
                        remove_button(ui, &mut *state, sel, CompKind::Bobber);
                    });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.menu_button("Add Component", |ui| {
                component_checklist(
                    ui,
                    sel,
                    &mut *state,
                    &[
                        (CompKind::PrimitiveMesh, q_meshes.get(sel).is_ok()),
                        (CompKind::PbrDef, q_pbr.get(sel).is_ok()),
                        (CompKind::DirectionalLight, q_dl.get(sel).is_ok()),
                        (CompKind::PointLight, q_pl.get(sel).is_ok()),
                        (CompKind::SpotLight, q_sl.get(sel).is_ok()),
                        (CompKind::Rotator, q_rot.get(sel).is_ok()),
                        (CompKind::Bobber, q_bob.get(sel).is_ok()),
                    ],
                );
            });
        });
    state.inspector_width = panel.response.rect.width();
}

fn remove_button(ui: &mut egui::Ui, state: &mut EditorState, entity: Entity, kind: CompKind) {
    if ui
        .add(egui::Button::new("Remove Component").small())
        .clicked()
    {
        state.request = Some(EditorRequest::RemoveComponent(entity, kind));
    }
}

fn component_checklist(ui: &mut egui::Ui, entity: Entity, state: &mut EditorState, present: &[(CompKind, bool)]) {
    let mut last_group = "";
    for (kind, has) in present {
        let group = match kind {
            CompKind::PrimitiveMesh | CompKind::PbrDef => "Rendering",
            CompKind::PointLight | CompKind::DirectionalLight | CompKind::SpotLight => "Lighting",
            CompKind::Rotator | CompKind::Bobber => "Gameplay",
        };
        if group != last_group {
            ui.label(egui::RichText::new(group).weak());
            last_group = group;
        }
        let mark = if *has { "✓" } else { "+" };
        if ui.selectable_label(*has, format!("{mark} {}", kind.label())).clicked() {
            state.request = Some(if *has {
                EditorRequest::RemoveComponent(entity, *kind)
            } else {
                EditorRequest::AddComponent(entity, *kind)
            });
            ui.close_menu();
        }
    }
}

fn transform_editor(ui: &mut egui::Ui, transform: &mut Transform) {
    egui::Grid::new("transform_grid")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Position");
            vec3_drag(ui, &mut transform.translation, 0.02, "pos");
            ui.end_row();

            ui.label("Rotation");
            let (ex, ey, ez) = transform.rotation.to_euler(EulerRot::XYZ);
            let mut deg = Vec3::new(ex.to_degrees(), ey.to_degrees(), ez.to_degrees());
            let before = deg;
            vec3_drag(ui, &mut deg, 0.5, "rot");
            if deg != before {
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    deg.x.to_radians(),
                    deg.y.to_radians(),
                    deg.z.to_radians(),
                );
            }
            ui.end_row();

            ui.label("Scale");
            vec3_drag(ui, &mut transform.scale, 0.01, "scale");
            ui.end_row();
        });
}

fn vec3_drag(ui: &mut egui::Ui, value: &mut Vec3, speed: f32, _id: &str) {
    ui.horizontal(|ui| {
        let entries = [("X:", &mut value.x), ("Y:", &mut value.y), ("Z:", &mut value.z)];
        for (prefix, part) in entries {
            ui.add(
                egui::DragValue::new(part)
                    .speed(speed)
                    .prefix(prefix)
                    .range(-100_000.0..=100_000.0),
            );
        }
    });
}

fn slider_row(ui: &mut egui::Ui, label: &str, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    let wide = range.end() - range.start() > 1000.0;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(value, range).logarithmic(wide));
    });
}

fn color_row(ui: &mut egui::Ui, label: &str, color: &mut Color) {
    ui.horizontal(|ui| {
        ui.label(label);
        let [r, g, b, _a] = Srgba::from(*color).to_f32_array();
        let mut egui_color = egui::Color32::from_rgb(
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );
        egui::color_picker::color_edit_button_srgba(
            ui,
            &mut egui_color,
            egui::color_picker::Alpha::Opaque,
        );
        let [r, g, b, a] = egui_color.to_array();
        *color = Color::srgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        );
    });
}

// ---------------------------------------------------------------------------
// Game View
// ---------------------------------------------------------------------------

pub fn game_view_panel(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    image: Res<crate::editor::GameViewImage>,
    q_names: Query<&Name>,
) {
    state.viewport.new_frame();
    let Some(texture_id) = contexts.image_id(&image.0) else {
        return;
    };
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    let image_size = state.viewport.image_size;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.strong("Game");
            ui.separator();
            ui.weak(format!("{} × {}", image_size.x as u32, image_size.y as u32));
            let (label, color) = match state.play {
                PlayState::Stopped => ("Stopped", egui::Color32::GRAY),
                PlayState::Playing => ("▶ PLAYING", egui::Color32::from_rgb(90, 220, 110)),
                PlayState::Paused => ("PAUSED", egui::Color32::from_rgb(240, 200, 90)),
            };
            ui.colored_label(color, label);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(hovered) = state.hovered_entity {
                    let name = q_names
                        .get(hovered)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_else(|_| format!("Entity {}", hovered.index()));
                    ui.weak(format!("under cursor: {name}"));
                } else {
                    ui.weak("no entity under cursor");
                }
            });
        });
        ui.separator();

        let avail = ui.available_size();
        if avail.x < 8.0 || avail.y < 8.0 || image_size.x < 1.0 || image_size.y < 1.0 {
            return;
        }
        let aspect = image_size.x / image_size.y;
        let mut size = avail;
        if size.x / size.y > aspect {
            size.x = size.y * aspect;
        } else {
            size.y = size.x / aspect;
        }

        let response = ui.add(
            egui::Image::new(egui::load::SizedTexture::new(texture_id, size))
                .sense(egui::Sense::click_and_drag()),
        );

        let v = &mut state.viewport;
        v.hovered = response.hovered();
        if let Some(pos) = response.interact_pointer_pos() {
            let rect = response.rect;
            let uv = Vec2::new(
                (pos.x - rect.left()) / rect.width(),
                (pos.y - rect.top()) / rect.height(),
            );
            v.pointer_uv = Some(uv.clamp(Vec2::ZERO, Vec2::splat(1.0)));
        } else if !response.dragged() {
            v.pointer_uv = None;
        }

        v.click = response.clicked_by(egui::PointerButton::Primary);
        v.drag_left_started = response.drag_started_by(egui::PointerButton::Primary);
        v.drag_left = response.dragged_by(egui::PointerButton::Primary);
        v.drag_left_ended = response.drag_stopped_by(egui::PointerButton::Primary);
        if v.drag_left_started {
            v.drag_px = 0.0;
        }
        v.drag_right = response.dragged_by(egui::PointerButton::Secondary);
        v.drag_middle = response.dragged_by(egui::PointerButton::Middle);
        v.drag_delta = {
            let d = response.drag_delta();
            Vec2::new(d.x, d.y)
        };
        if v.drag_left {
            v.drag_px += v.drag_delta.length();
        }
        if v.hovered {
            v.wheel = ui.input(|i| i.smooth_scroll_delta.y);
        }

        let ppp = ui.pixels_per_point();
        v.desired_px = Vec2::new((size.x * ppp).max(64.0), (size.y * ppp).max(64.0));
    });
}

// ---------------------------------------------------------------------------
// Bottom dock: Assets + Console
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum AssetNode {
    Dir {
        name: String,
        path: PathBuf,
        children: Vec<AssetNode>,
    },
    File {
        name: String,
        path: PathBuf,
        size: u64,
    },
}

#[derive(Resource, Default)]
pub struct AssetBrowser {
    pub root: Vec<AssetNode>,
    pub scanned: bool,
}

fn scan_dir(dir: &std::path::Path, depth: u32) -> Vec<AssetNode> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut dirs = vec![];
    let mut files = vec![];
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if file_type.is_dir() {
            if depth < 5 && name != ".git" {
                let children = scan_dir(&path, depth + 1);
                dirs.push(AssetNode::Dir { name, path, children });
            }
        } else {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            files.push(AssetNode::File { name, path, size });
        }
    }
    dirs.sort_by(|a, b| a.name().cmp(b.name()));
    files.sort_by(|a, b| a.name().cmp(b.name()));
    dirs.extend(files);
    dirs
}

impl AssetNode {
    fn name(&self) -> &str {
        match self {
            AssetNode::Dir { name, .. } => name,
            AssetNode::File { name, .. } => name,
        }
    }
}

pub fn bottom_dock(
    mut contexts: EguiContexts,
    mut state: ResMut<EditorState>,
    mut commands: Commands,
    mut browser: ResMut<AssetBrowser>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    if state.refresh_assets {
        browser.root = scan_dir(std::path::Path::new("assets"), 0);
        browser.scanned = true;
        state.refresh_assets = false;
    }
    if !browser.scanned {
        browser.root = scan_dir(std::path::Path::new("assets"), 0);
        browser.scanned = true;
    }

    let height = if state.panels.assets || state.panels.console {
        200.0
    } else {
        0.0
    };
    if height == 0.0 {
        return;
    }

    // The dock may never eat the vertical space the central Game View needs.
    let screen_height = ctx.screen_rect().height();
    let max_height =
        (screen_height - BARS_RESERVE - CENTRAL_MIN_HEIGHT).max(DOCK_MIN_HEIGHT);
    egui::TopBottomPanel::bottom("bottom_dock")
        .resizable(true)
        .height_range(DOCK_MIN_HEIGHT..=max_height)
        .default_height(height)
        .show(ctx, |ui| {
            if state.panels.assets {
                // Inside the dock: assets take at most 75% so the console
                // always stays visible next to them.
                let dock_width = ui.available_width();
                let assets_max = (dock_width * 0.75).max(DOCK_ASSETS_MIN_WIDTH);
                egui::SidePanel::left("dock_assets")
                    .resizable(true)
                    .width_range(DOCK_ASSETS_MIN_WIDTH..=assets_max)
                    .default_width(dock_width * 0.42)
                    .show_inside(ui, |ui| {
                        assets_ui(ui, &mut state, &mut commands, &browser);
                    });
            }
            if state.panels.console {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    console_ui(ui, &mut state);
                });
            } else {
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    ui.weak("Console hidden (View ▸ Panels)");
                });
            }
        });
}

fn assets_ui(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    commands: &mut Commands,
    browser: &AssetBrowser,
) {
    ui.heading("Assets");
    ui.horizontal(|ui| {
        if ui.button("Refresh").clicked() {
            state.refresh_assets = true;
        }
        if ui.button("Open Folder").clicked() {
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("explorer").arg("assets").spawn();
        }
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if browser.root.is_empty() {
                ui.weak("assets/ is empty. Saved scenes go to assets/scenes.");
            }
            for node in &browser.root {
                asset_node_ui(ui, node, state, commands);
            }
        });
    ui.separator();
    match &state.asset_selected {
        Some(path) => {
            let meta = std::fs::metadata(path).ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            ui.monospace(path.display().to_string());
            ui.weak(format!("{:.1} KB", size as f64 / 1024.0));
            if path.to_string_lossy().ends_with(".scn.ron")
                && ui.button("Load this scene").clicked()
            {
                state.scene_path = Some(path.clone());
                state.scene_dirty = false;
                state.selected = None;
                state.play = PlayState::Stopped;
                state.playback_snapshot = None;
                let path = path.clone();
                commands.queue(move |world: &mut World| match scene_io::load_scene(world, &path) {
                    Ok(n) => info!("Scene loaded: {n} entities from {}", path.display()),
                    Err(e) => error!("Load failed: {e}"),
                });
            }
        }
        None => {
            ui.weak("Select a file to inspect it.");
        }
    }
}

fn asset_node_ui(ui: &mut egui::Ui, node: &AssetNode, state: &mut EditorState, commands: &mut Commands) {
    match node {
        AssetNode::Dir { name, path, children } => {
            egui::CollapsingHeader::new(format!("📁 {name}"))
                .id_salt(egui::Id::new(path.as_os_str()))
                .default_open(children.len() < 8)
                .show(ui, |ui| {
                    for child in children {
                        asset_node_ui(ui, child, state, commands);
                    }
                });
        }
        AssetNode::File { name, path, size } => {
            let selected = state.asset_selected.as_deref() == Some(path.as_path());
            let is_scene = name.ends_with(".scn.ron");
            let dot = if is_scene {
                egui::Color32::from_rgb(230, 190, 70)
            } else if name.ends_with(".png") || name.ends_with(".jpg") {
                egui::Color32::from_rgb(230, 130, 170)
            } else {
                egui::Color32::from_gray(140)
            };
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("●").color(dot).small());
                let response =
                    ui.selectable_label(selected, format!("{name}  ({:.1} KB)", *size as f64 / 1024.0));
                if response.clicked() {
                    state.asset_selected = Some(path.clone());
                }
                if response.double_clicked() && is_scene {
                    state.asset_selected = Some(path.clone());
                    state.refresh_assets = true; // no-op keeps state fresh
                    let path = path.clone();
                    commands.queue(move |world: &mut World| {
                        match scene_io::load_scene(world, &path) {
                            Ok(n) => info!("Scene loaded: {n} entities from {}", path.display()),
                            Err(e) => error!("Load failed: {e}"),
                        }
                    });
                }
            });
        }
    }
}

fn console_ui(ui: &mut egui::Ui, state: &mut EditorState) {
    ui.heading("Console");
    ui.horizontal(|ui| {
        let f = &mut state.console;
        ui.toggle_value(&mut f.info, "Info");
        ui.toggle_value(&mut f.warn, "Warn");
        ui.toggle_value(&mut f.error, "Error");
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut f.search)
                .hint_text("Filter...")
                .desired_width(110.0),
        );
        if ui.button("Clear").clicked() {
            if let Ok(mut buffer) = log_layer::log_buffer().lock() {
                buffer.clear();
            }
        }
        ui.checkbox(&mut f.auto_scroll, "Auto-scroll");
    });
    ui.separator();

    let buffer = log_layer::log_buffer();
    let Ok(lines) = buffer.lock() else {
        return;
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .stick_to_bottom(state.console.auto_scroll)
        .show(ui, |ui| {
            let filter = state.console.search.to_lowercase();
            use bevy::log::tracing::Level;
            for line in lines.iter() {
                let visible = match line.level {
                    Level::ERROR => state.console.error,
                    Level::WARN => state.console.warn,
                    _ => state.console.info,
                };
                if !visible {
                    continue;
                }
                if !filter.is_empty()
                    && !line.message.to_lowercase().contains(&filter)
                    && !line.target.to_lowercase().contains(&filter)
                {
                    continue;
                }
                let color = match line.level {
                    Level::ERROR => egui::Color32::from_rgb(235, 90, 90),
                    Level::WARN => egui::Color32::from_rgb(235, 200, 90),
                    Level::INFO => egui::Color32::from_gray(190),
                    _ => egui::Color32::from_gray(140),
                };
                ui.monospace(
                    egui::RichText::new(format!(
                        "{} [{:>5}] {}",
                        line.time, line.target_short(), line.message
                    ))
                    .color(color)
                    .small(),
                );
            }
        });
}

// ---------------------------------------------------------------------------
// Keyboard shortcuts
// ---------------------------------------------------------------------------

pub fn shortcuts(mut contexts: EguiContexts, mut state: ResMut<EditorState>, mut commands: Commands) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };
    if ctx.wants_keyboard_input() {
        return;
    }
    let ctrl = ctx.input(|i| i.modifiers.ctrl);
    let pressed = |key: egui::Key| ctx.input(|i| i.key_pressed(key));

    if ctrl && pressed(egui::Key::S) {
        do_save(&mut *state, &mut commands, false);
    } else if ctrl && pressed(egui::Key::O) {
        do_open(&mut *state, &mut commands);
    } else if ctrl && pressed(egui::Key::D) {
        if let Some(e) = state.selected {
            state.request = Some(EditorRequest::Duplicate(e));
        }
    } else if ctrl {
        // other ctrl combos: ignore
    } else {
        if pressed(egui::Key::Q) {
            state.tool = Tool::Select;
        }
        if pressed(egui::Key::W) {
            state.tool = Tool::Move;
        }
        if pressed(egui::Key::E) {
            state.tool = Tool::Rotate;
        }
        if pressed(egui::Key::R) {
            state.tool = Tool::Scale;
        }
        if pressed(egui::Key::F) {
            if let Some(e) = state.selected {
                state.focus_request = Some(e);
            }
        }
        if pressed(egui::Key::Escape) {
            state.selected = None;
        }
        if pressed(egui::Key::Delete) {
            if let Some(e) = state.selected {
                state.request = Some(EditorRequest::Delete(e));
            }
        }
        if pressed(egui::Key::Space) {
            state.request = Some(match state.play {
                PlayState::Stopped => EditorRequest::Play,
                _ => EditorRequest::TogglePause,
            });
        }
    }
}
