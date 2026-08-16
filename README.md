# Bevy Editor

A visual engine interface (scene editor) for the [Bevy engine](https://bevyengine.org),
built with **bevy 0.16.1** and **bevy_egui 0.34.1**.

It provides the panels and workflow you know from engines like Unity or Godot:

| Panel | What it does |
|---|---|
| **Game View** (center) | Live rendered 3D viewport (camera renders to a texture shown by egui, resized live with the panel). Click to pick entities, right-drag to orbit, middle-drag to pan, wheel to zoom. |
| **Hierarchy** (left) | Scene tree with parenting, visibility toggles, search, inline rename and a full context menu (create child, duplicate, reparent, delete). |
| **Inspector** (right) | Component editors for the selected entity: Transform, Mesh (primitive), PBR Material (color picker, metallic/roughness…), lights, gameplay components — plus an **Add Component checklist**. |
| **Assets** (bottom-left) | Browser for the `assets/` folder; double-click a `.scn.ron` to load it. |
| **Console** (bottom-right) | Captured engine logs with level filters and text search. |
| **Toolbar / Menu / Status bar** | Play / Pause / Stop, transform tools (Q/W/E/R), grid & gizmo toggles, FPS, entity count, GPU name. |

## Feature highlights

- **Play mode** – pressing Play snapshots the scene, runs gameplay systems
  (`Rotator`, `Bobber`), and Stop restores the scene exactly.
- **Scene save/load** – Bevy-native `.scn.ron` scene files via native file dialogs.
- **Live picking & transform tools** – ray-cast selection in the Game View, drag
  to move/rotate/scale, hold Ctrl to snap.
- **Editor gizmos** – grid, world axes, selection outline, light markers.

## Controls

```
Right mouse drag  orbit camera        Q/W/E/R  Select/Move/Rotate/Scale tool
Middle mouse drag pan camera          Drag     transform (with tool active)
Mouse wheel       zoom                Ctrl     snap while transforming
Left click        select entity       F        focus selection
Del               delete selection    Ctrl+D   duplicate
Ctrl+S / Ctrl+O   save / open scene   Space    play / pause
```

## Build & run

```bash
cargo run            # debug build (fast to compile)
cargo run --release  # final executable (target/release/bevy_editor.exe)
```

Run from the project folder so the `assets/` directory is found.

## Project layout

```
src/
  main.rs             App setup, plugins, window
  components.rs       Editor component definitions + mesh/material sync + gameplay
  demo.rs             Demo scene
  scene_io.rs         Save / load / snapshot / duplicate (bevy_scene RON)
  log_layer.rs        tracing layer capturing logs for the Console panel
  editor/mod.rs       Editor state, orbit camera, picking, tools, gizmos, actions
  editor/ui.rs        All egui panels
```

## Extending

- **New editable component**: define it in `components.rs` with
  `#[derive(Component, Reflect)] #[reflect(Component)]`, register it in
  `EditorPlugin`, add it to the allow list in `scene_io.rs`, and add an editor
  section + checklist entry in `editor/ui.rs`.
- **New gameplay system**: add a system gated on `state.play == PlayState::Playing`
  like `rotator_system`.
