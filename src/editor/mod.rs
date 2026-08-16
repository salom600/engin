//! Editor core: state, plugin, viewport camera, picking, tools, gizmos and
//! the action/request pipeline.

pub mod ui;

use crate::components;
use crate::components::*;
use crate::{demo, scene_io};
use bevy::hierarchy::Parent;
use bevy::prelude::*;
use bevy::render::camera::{Camera, RenderTarget};
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::EguiUserTextures;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Move,
    Rotate,
    Scale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct Panels {
    pub hierarchy: bool,
    pub inspector: bool,
    pub assets: bool,
    pub console: bool,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            hierarchy: true,
            inspector: true,
            assets: true,
            console: true,
        }
    }
}

/// Interaction state of the Game View panel, written by the UI every frame
/// and consumed by camera/picking/tools systems.
#[derive(Debug, Clone, Copy)]
pub struct ViewportInteraction {
    /// Current render texture size in pixels.
    pub image_size: Vec2,
    /// Desired texture size (panel size in pixels) for live resizing.
    pub desired_px: Vec2,
    pub hovered: bool,
    /// Pointer position in 0..=1 texture coordinates (top-left origin).
    pub pointer_uv: Option<Vec2>,
    pub click: bool,
    /// Accumulated left-drag distance in px (to distinguish click vs drag).
    pub drag_px: f32,
    pub drag_left_started: bool,
    pub drag_left: bool,
    pub drag_left_ended: bool,
    pub drag_right: bool,
    pub drag_middle: bool,
    pub drag_delta: Vec2,
    pub wheel: f32,
}

impl Default for ViewportInteraction {
    fn default() -> Self {
        Self {
            image_size: Vec2::new(1024.0, 640.0),
            desired_px: Vec2::new(1024.0, 640.0),
            hovered: false,
            pointer_uv: None,
            click: false,
            drag_px: 0.0,
            drag_left_started: false,
            drag_left: false,
            drag_left_ended: false,
            drag_right: false,
            drag_middle: false,
            drag_delta: Vec2::ZERO,
            wheel: 0.0,
        }
    }
}

impl ViewportInteraction {
    fn new_frame(&mut self) {
        self.click = false;
        self.drag_left_started = false;
        self.drag_left_ended = false;
        self.drag_right = false;
        self.drag_middle = false;
        self.drag_delta = Vec2::ZERO;
        self.wheel = 0.0;
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ToolDrag {
    pub start_hit: Vec3,
    pub start_translation: Vec3,
    pub start_rotation: Quat,
    pub start_scale: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompKind {
    PrimitiveMesh,
    PbrDef,
    Rotator,
    Bobber,
    PointLight,
    DirectionalLight,
    SpotLight,
}

impl CompKind {
    pub fn label(&self) -> &'static str {
        match self {
            CompKind::PrimitiveMesh => "Primitive Mesh",
            CompKind::PbrDef => "PBR Material",
            CompKind::Rotator => "Rotator (gameplay)",
            CompKind::Bobber => "Bobber (gameplay)",
            CompKind::PointLight => "Point Light",
            CompKind::DirectionalLight => "Directional Light",
            CompKind::SpotLight => "Spot Light",
        }
    }

    pub const EDITABLE: [CompKind; 7] = [
        CompKind::PrimitiveMesh,
        CompKind::PbrDef,
        CompKind::PointLight,
        CompKind::DirectionalLight,
        CompKind::SpotLight,
        CompKind::Rotator,
        CompKind::Bobber,
    ];
}

#[derive(Clone, Debug)]
pub enum EditorRequest {
    Spawn(SpawnKind),
    SpawnChild(Entity, SpawnKind),
    Delete(Entity),
    Duplicate(Entity),
    Focus(Entity),
    Reparent {
        child: Entity,
        new_parent: Option<Entity>,
    },
    Play,
    Stop,
    TogglePause,
    AddComponent(Entity, CompKind),
    RemoveComponent(Entity, CompKind),
}

#[derive(Debug, Clone)]
pub struct ConsoleFilter {
    pub search: String,
    pub info: bool,
    pub warn: bool,
    pub error: bool,
    pub auto_scroll: bool,
}

impl Default for ConsoleFilter {
    fn default() -> Self {
        Self {
            search: String::new(),
            info: true,
            warn: true,
            error: true,
            auto_scroll: true,
        }
    }
}

/// Central editor state.
#[derive(Resource)]
pub struct EditorState {
    pub selected: Option<Entity>,
    pub hovered_entity: Option<Entity>,
    pub tool: Tool,
    pub play: PlayState,
    pub panels: Panels,
    pub show_grid: bool,
    pub show_selection_gizmo: bool,
    pub show_light_gizmos: bool,
    pub scene_path: Option<PathBuf>,
    pub scene_dirty: bool,
    pub orbit_target: Vec3,
    pub orbit_yaw: f32,
    pub orbit_pitch: f32,
    pub orbit_distance: f32,
    pub focus_request: Option<Entity>,
    pub viewport: ViewportInteraction,
    pub about_open: bool,
    pub rename_target: Option<Entity>,
    pub rename_buf: String,
    pub name_buf: String,
    pub name_buf_entity: Option<Entity>,
    pub hierarchy_filter: String,
    pub playback_snapshot: Option<DynamicScene>,
    pub request: Option<EditorRequest>,
    pub console: ConsoleFilter,
    pub asset_selected: Option<PathBuf>,
    pub refresh_assets: bool,
    pub tool_drag: Option<ToolDrag>,
    pub theme_applied: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selected: None,
            hovered_entity: None,
            tool: Tool::Select,
            play: PlayState::Stopped,
            panels: Panels::default(),
            show_grid: true,
            show_selection_gizmo: true,
            show_light_gizmos: true,
            scene_path: None,
            scene_dirty: false,
            orbit_target: Vec3::new(0.0, 0.8, 0.0),
            orbit_yaw: 0.9,
            orbit_pitch: 0.55,
            orbit_distance: 11.0,
            focus_request: None,
            viewport: ViewportInteraction::default(),
            about_open: false,
            rename_target: None,
            rename_buf: String::new(),
            name_buf: String::new(),
            name_buf_entity: None,
            hierarchy_filter: String::new(),
            playback_snapshot: None,
            request: None,
            console: ConsoleFilter::default(),
            asset_selected: None,
            refresh_assets: false,
            tool_drag: None,
            theme_applied: false,
        }
    }
}

/// Handle of the texture the scene camera renders into (shown in the Game View).
#[derive(Resource, Deref, Debug)]
pub struct GameViewImage(pub Handle<Image>);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorState>()
            .register_type::<PrimitiveShape>()
            .register_type::<PrimitiveMesh>()
            .register_type::<PbrDef>()
            .register_type::<Rotator>()
            .register_type::<Bobber>()
            .insert_resource(ClearColor(Color::srgb(0.085, 0.09, 0.115)))
            .add_systems(Startup, (setup_editor_world, spawn_startup_scene))
            .add_systems(
                Update,
                (
                    components::sync_meshes,
                    components::ensure_materials,
                    components::sync_materials,
                    components::capture_bobber_base,
                    resize_game_view_system,
                    request_system,
                ),
            )
            .add_systems(Update, (components::rotator_system, components::bobber_system))
            .add_systems(Update, editor_camera_system)
            .add_systems(Update, (picking_system, tool_drag_system, gizmos_system).chain())
            .add_systems(
                bevy_egui::EguiContextPass,
                (
                    ui::menu_bar,
                    ui::toolbar,
                    ui::status_bar,
                    ui::bottom_dock,
                    ui::hierarchy_panel,
                    ui::inspector_panel,
                    ui::game_view_panel,
                    ui::shortcuts,
                )
                    .chain(),
            )
    }
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn setup_editor_world(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut user_textures: ResMut<EguiUserTextures>,
    mut gizmo_config: ResMut<GizmoConfigStore>,
) {
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 45.0,
    });

    // Draw editor gizmos on top of geometry.
    let (config, _) = gizmo_config.config_mut::<DefaultGizmoConfigGroup>();
    config.depth_bias = -1.0;

    // Create the assets folder so the browser has something to show.
    let _ = std::fs::create_dir_all("assets/scenes");

    let handle = images.add(make_game_view_image(UVec2::new(1024, 640)));
    user_textures.add_image(handle.clone());
    commands.insert_resource(GameViewImage(handle.clone()));

    commands.spawn((
        Name::new("Editor Camera"),
        EditorInternal,
        EditorCamera,
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(handle.into()),
            ..default()
        },
        Transform::default(),
    ));
}

fn spawn_startup_scene(world: &mut World) {
    demo::spawn_demo_world(world);
}

pub fn make_game_view_image(size: UVec2) -> Image {
    let size = Extent3d {
        width: size.x.max(1),
        height: size.y.max(1),
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some("bevy_editor.game_view"),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    image.resize(size);
    image
}

// ---------------------------------------------------------------------------
// Game view resizing
// ---------------------------------------------------------------------------

fn resize_game_view_system(
    mut state: ResMut<EditorState>,
    image: Res<GameViewImage>,
    mut images: ResMut<Assets<Image>>,
) {
    let desired = state.viewport.desired_px;
    if desired.x < 64.0 || desired.y < 64.0 {
        return;
    }
    let desired = ((desired / 2.0).round() * 2.0).clamp(Vec2::splat(64.0), Vec2::splat(3840.0));
    if (desired - state.viewport.image_size).abs().max_element() < 4.0 {
        return;
    }
    if let Some(asset) = images.get_mut(&image.0) {
        let size = Extent3d {
            width: desired.x as u32,
            height: desired.y as u32,
            depth_or_array_layers: 1,
        };
        asset.texture_descriptor.size = size;
        asset.resize(size);
        state.viewport.image_size = desired;
    }
}

// ---------------------------------------------------------------------------
// Editor camera (orbit / pan / zoom)
// ---------------------------------------------------------------------------

fn editor_camera_system(
    mut state: ResMut<EditorState>,
    mut q_cam: Query<(&mut Transform, &GlobalTransform), With<EditorCamera>>,
    q_selected: Query<(&GlobalTransform, Option<&PrimitiveMesh>), With<SceneEntity>>,
) {
    let Ok((mut transform, global)) = q_cam.single_mut() else {
        return;
    };
    let v = &state.viewport;

    if v.drag_right {
        state.orbit_yaw -= v.drag_delta.x * 0.007;
        state.orbit_pitch = (state.orbit_pitch + v.drag_delta.y * 0.007).clamp(-1.54, 1.54);
    }
    if v.drag_middle {
        let right = global.right();
        let up = global.up();
        let k = state.orbit_distance * 0.0015;
        state.orbit_target -= right * (v.drag_delta.x * k) + up * (v.drag_delta.y * k);
    }
    if v.hovered && v.wheel.abs() > 0.0 {
        state.orbit_distance = (state.orbit_distance * (1.0 - v.wheel * 0.0012)).clamp(0.25, 500.0);
    }

    if let Some(entity) = state.focus_request.take() {
        if let Ok((gt, mesh)) = q_selected.get(entity) {
            let (center, half) = match mesh {
                Some(pm) => {
                    let (c, h) = shape_bounds(pm.shape);
                    (gt.transform_point(c), h)
                }
                None => (gt.translation(), Vec3::splat(0.3)),
            };
            state.orbit_target = center;
            state.orbit_distance = (half.length() * 3.2 + 0.6).clamp(0.7, 500.0);
        }
    }

    let dir = Vec3::new(
        state.orbit_pitch.cos() * state.orbit_yaw.cos(),
        state.orbit_pitch.sin(),
        state.orbit_pitch.cos() * state.orbit_yaw.sin(),
    );
    transform.translation = state.orbit_target + dir * state.orbit_distance;
    transform.look_at(state.orbit_target, Vec3::Y);
}

// ---------------------------------------------------------------------------
// Picking
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
fn picking_system(
    mut state: ResMut<EditorState>,
    q_cam: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    q_targets: Query<
        (
            Entity,
            &GlobalTransform,
            Option<&PrimitiveMesh>,
            Option<&PointLight>,
            Option<&DirectionalLight>,
            Option<&SpotLight>,
        ),
        With<SceneEntity>,
    >,
) {
    state.hovered_entity = None;
    let Ok((camera, cam_gt)) = q_cam.single() else {
        return;
    };
    let Some(ray) = pointer_ray(&state, camera, cam_gt) else {
        return;
    };

    let ro = ray.origin;
    let rd = Vec3::from(ray.direction);
    let mut best: Option<(f32, Entity)> = None;

    for (entity, gt, mesh, pl, dl, sl) in q_targets.iter() {
        let (center, half) = if let Some(pm) = mesh {
            shape_bounds(pm.shape)
        } else if pl.is_some() || dl.is_some() || sl.is_some() {
            (Vec3::ZERO, Vec3::splat(0.35))
        } else {
            (Vec3::ZERO, Vec3::splat(0.15))
        };

        let inverse = gt.compute_matrix().inverse();
        let local_origin = inverse.project_point3(ro);
        let local_dir = inverse.transform_vector3(rd);
        // `local_dir` is intentionally not normalized: slab `t` values then map
        // 1:1 to world-space distances.
        if let Some(t) = ray_aabb(local_origin, local_dir, center, half) {
            if best.map_or(true, |(bt, _)| t < bt) {
                best = Some((t, entity));
            }
        }
    }

    if let Some((_, entity)) = best {
        state.hovered_entity = Some(entity);
    }

    // A click that wasn't a drag selects the entity under the cursor.
    if state.viewport.click && state.viewport.drag_px < 5.0 {
        state.selected = state.hovered_entity;
    }
}

fn pointer_ray(state: &EditorState, camera: &Camera, cam_gt: &GlobalTransform) -> Option<Ray3d> {
    let uv = state.viewport.pointer_uv?;
    let size = state.viewport.image_size;
    if size.x < 1.0 || size.y < 1.0 {
        return None;
    }
    let px = Vec2::new(uv.x * size.x, uv.y * size.y);
    camera.viewport_to_world(cam_gt, px).ok()
}

fn ray_aabb(o: Vec3, d: Vec3, center: Vec3, half: Vec3) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        let (oi, di, ci, hi) = (o[i], d[i], center[i], half[i]);
        if di.abs() < 1e-8 {
            if oi < ci - hi || oi > ci + hi {
                return None;
            }
        } else {
            let mut t1 = (ci - hi - oi) / di;
            let mut t2 = (ci + hi - oi) / di;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

// ---------------------------------------------------------------------------
// Transform tools (drag in game view)
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
fn tool_drag_system(
    mut state: ResMut<EditorState>,
    q_cam: Query<(&Camera, &GlobalTransform), With<EditorCamera>>,
    mut q_transform: Query<&mut Transform>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if state.tool == Tool::Select {
        state.tool_drag = None;
        return;
    }
    let Some(target) = state.selected else {
        state.tool_drag = None;
        return;
    };
    let Ok((camera, cam_gt)) = q_cam.single() else {
        return;
    };
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let tool = state.tool;

    if state.viewport.drag_left_started {
        if let Ok(t) = q_transform.get(target) {
            let anchor = t.translation;
            let start_hit = pointer_ray(&state, camera, cam_gt)
                .and_then(|ray| ray_plane(ray, anchor, Vec3::from(cam_gt.forward())))
                .unwrap_or(anchor);
            state.tool_drag = Some(ToolDrag {
                start_hit,
                start_translation: anchor,
                start_rotation: t.rotation,
                start_scale: t.scale,
            });
        }
    }

    let Some(drag) = state.tool_drag else {
        return;
    };
    if !state.viewport.drag_left {
        if state.viewport.drag_left_ended {
            state.tool_drag = None;
        }
        return;
    }

    if let Ok(mut t) = q_transform.get_mut(target) {
        match tool {
            Tool::Move => {
                if let Some(ray) = pointer_ray(&state, camera, cam_gt) {
                    let forward = Vec3::from(cam_gt.forward());
                    if let Some(hit) = ray_plane(ray, drag.start_hit, forward) {
                        let mut new_translation = drag.start_translation + (hit - drag.start_hit);
                        if ctrl {
                            new_translation = (new_translation / 0.25).round() * 0.25;
                        }
                        t.translation = new_translation;
                        state.scene_dirty = true;
                    }
                }
            }
            Tool::Rotate => {
                let mut yaw = -state.viewport.drag_delta.x * 0.01;
                if ctrl {
                    yaw = (yaw / 0.261_799).round() * 0.261_799; // 15° steps
                }
                t.rotation = Quat::from_rotation_y(yaw) * drag.start_rotation;
                state.scene_dirty = true;
            }
            Tool::Scale => {
                let factor = (-state.viewport.drag_delta.y * 0.006).exp();
                let mut scale = (drag.start_scale * factor).max(Vec3::splat(0.01));
                if ctrl {
                    scale = (scale / 0.25).round() * 0.25;
                }
                t.scale = scale;
                state.scene_dirty = true;
            }
            Tool::Select => {}
        }
    }
}

fn ray_plane(ray: Ray3d, point: Vec3, normal: Vec3) -> Option<Vec3> {
    let d = Vec3::from(ray.direction);
    let denom = d.dot(normal);
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = (point - ray.origin).dot(normal) / denom;
    Some(ray.origin + d * t)
}

// ---------------------------------------------------------------------------
// Gizmos (grid, axes, selection outline, light markers)
// ---------------------------------------------------------------------------

#[allow(clippy::type_complexity)]
fn gizmos_system(
    state: Res<EditorState>,
    mut gizmos: Gizmos,
    q_scene: Query<(Entity, &GlobalTransform, Option<&PrimitiveMesh>), With<SceneEntity>>,
    q_lights: Query<
        (
            Entity,
            &GlobalTransform,
            AnyOf<(&PointLight, &DirectionalLight, &SpotLight)>,
        ),
    >,
) {
    if state.show_grid {
        let half = 12;
        let minor = Color::srgb(0.24, 0.25, 0.30);
        for i in -half..=half {
            if i == 0 {
                continue;
            }
            let f = i as f32;
            gizmos.line(Vec3::new(f, 0.0, -half as f32), Vec3::new(f, 0.0, half as f32), minor);
            gizmos.line(Vec3::new(-half as f32, 0.0, f), Vec3::new(half as f32, 0.0, f), minor);
        }
        gizmos.line(Vec3::new(-half as f32, 0.0, 0.0), Vec3::new(half as f32, 0.0, 0.0), Color::srgb(0.78, 0.3, 0.3));
        gizmos.line(Vec3::new(0.0, 0.0, -half as f32), Vec3::new(0.0, 0.0, half as f32), Color::srgb(0.3, 0.44, 0.85));
        gizmos.line(Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), Color::srgb(0.32, 0.8, 0.36));
    }

    if state.show_light_gizmos {
        for (entity, gt, (_pl, dl, _sl)) in &q_lights {
            let selected = state.selected == Some(entity) || state.hovered_entity == Some(entity);
            let color = if selected {
                Color::srgb(1.0, 0.82, 0.35)
            } else {
                Color::srgb(0.62, 0.55, 0.25)
            };
            let pos = gt.translation();
            if dl.is_some() {
                gizmos.line(pos, pos + Vec3::from(gt.forward()) * 3.0, color);
            }
            gizmos.circle(Isometry3d::from_translation(pos), 0.25, color);
        }
    }

    if state.show_selection_gizmo {
        for (entity, gt, mesh) in q_scene.iter() {
            let selected = state.selected == Some(entity);
            if !selected && state.hovered_entity != Some(entity) {
                continue;
            }
            let (c, h) = match mesh {
                Some(pm) => shape_bounds(pm.shape),
                None => (Vec3::ZERO, Vec3::splat(0.3)),
            };
            let color = if selected {
                Color::srgb(1.0, 0.62, 0.1)
            } else {
                Color::srgb(0.62, 0.66, 0.72)
            };
            let box_transform = gt.mul_transform(Transform::from_translation(c).with_scale(h * 2.02));
            gizmos.cuboid(box_transform, color);
            if selected {
                let p = gt.translation();
                gizmos.line(p, p + Vec3::from(gt.right()) * 0.8, Color::srgb(0.9, 0.32, 0.32));
                gizmos.line(p, p + Vec3::from(gt.up()) * 0.8, Color::srgb(0.32, 0.85, 0.36));
                gizmos.line(p, p + Vec3::from(gt.back()) * 0.8, Color::srgb(0.32, 0.47, 0.95));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Request handling (world mutations from the UI)
// ---------------------------------------------------------------------------

fn request_system(mut commands: Commands, mut state: ResMut<EditorState>) {
    let Some(request) = state.request.take() else {
        return;
    };
    commands.queue(move |world: &mut World| handle_request(world, request));
}

pub fn handle_request(world: &mut World, request: EditorRequest) {
    let mut state = world
        .remove_resource::<EditorState>()
        .expect("EditorState must exist");

    match request {
        EditorRequest::Spawn(kind) => {
            let position = state.orbit_target;
            let entity = spawn_scene_entity(world, &kind.label(), kind, position);
            state.selected = Some(entity);
            state.scene_dirty = true;
            info!("Spawned {} ({:?})", kind.label(), entity);
        }
        EditorRequest::SpawnChild(parent, kind) => {
            let child = spawn_scene_entity(world, &kind.label(), kind, Vec3::ZERO);
            if world.entities().contains(parent) {
                world.entity_mut(parent).add_child(child);
            }
            state.selected = Some(child);
            state.scene_dirty = true;
        }
        EditorRequest::Delete(entity) => {
            if world.entities().contains(entity) {
                let _ = world.entity_mut(entity).despawn_recursive();
            }
            if state.selected == Some(entity) {
                state.selected = None;
            }
            state.scene_dirty = true;
        }
        EditorRequest::Duplicate(entity) => {
            if let Some(new) = scene_io::duplicate_entity_tree(world, entity) {
                state.selected = Some(new);
                state.scene_dirty = true;
            }
        }
        EditorRequest::Focus(entity) => {
            state.selected = Some(entity);
            state.focus_request = Some(entity);
        }
        EditorRequest::Reparent { child, new_parent } => {
            if world.entities().contains(child) {
                let valid = match new_parent {
                    Some(p) => p != child && world.entities().contains(p) && !in_subtree(world, child, p),
                    None => true,
                };
                if valid {
                    if let Some(old) = world.get::<Parent>(child).map(|p| p.get()) {
                        if world.entities().contains(old) {
                            world.entity_mut(old).remove_children(&[child]);
                        }
                    }
                    if let Some(p) = new_parent {
                        world.entity_mut(p).add_child(child);
                    }
                    state.scene_dirty = true;
                }
            }
        }
        EditorRequest::Play => {
            if state.play == PlayState::Stopped {
                state.playback_snapshot = Some(scene_io::snapshot_scene(world));
                info!("Play mode started (scene snapshotted)");
            }
            state.play = PlayState::Playing;
        }
        EditorRequest::Stop => {
            if let Some(snapshot) = state.playback_snapshot.take() {
                scene_io::clear_scene(world);
                let mut map = bevy::ecs::entity::EntityHashMap::default();
                if let Err(e) = snapshot.write_to_world(world, &mut map) {
                    error!("Failed to restore scene after play: {e}");
                }
                for spawned in map.values() {
                    if let Ok(mut entity) = world.get_entity_mut(*spawned) {
                        entity.insert(SceneEntity);
                    }
                }
                info!("Play mode stopped (scene restored)");
            }
            state.play = PlayState::Stopped;
            state.selected = None;
            state.hovered_entity = None;
        }
        EditorRequest::TogglePause => {
            state.play = match state.play {
                PlayState::Playing => PlayState::Paused,
                PlayState::Paused => PlayState::Playing,
                PlayState::Stopped => PlayState::Stopped,
            };
        }
        EditorRequest::AddComponent(entity, kind) => {
            if world.entities().contains(entity) {
                add_component(world, entity, kind);
                state.scene_dirty = true;
            }
        }
        EditorRequest::RemoveComponent(entity, kind) => {
            if world.entities().contains(entity) {
                remove_component(world, entity, kind);
                state.scene_dirty = true;
            }
        }
    }

    world.insert_resource(state);
}

/// True if `entity` is inside the subtree rooted at `root` (or is `root`).
fn in_subtree(world: &World, root: Entity, entity: Entity) -> bool {
    let mut current = entity;
    for _ in 0..10_000 {
        if current == root {
            return true;
        }
        match world.get::<Parent>(current) {
            Some(parent) => current = parent.get(),
            None => return false,
        }
    }
    false
}

fn add_component(world: &mut World, entity: Entity, kind: CompKind) {
    let mut e = world.entity_mut(entity);
    match kind {
        CompKind::PrimitiveMesh => {
            e.insert(PrimitiveMesh {
                shape: PrimitiveShape::Cube,
            });
        }
        CompKind::PbrDef => {
            e.insert(PbrDef::default());
        }
        CompKind::Rotator => {
            e.insert(Rotator::default());
        }
        CompKind::Bobber => {
            e.insert(Bobber::default());
        }
        CompKind::PointLight => {
            e.insert(PointLight {
                intensity: 80_000.0,
                range: 20.0,
                shadows_enabled: true,
                ..default()
            });
        }
        CompKind::DirectionalLight => {
            e.insert(DirectionalLight {
                shadows_enabled: true,
                ..default()
            });
        }
        CompKind::SpotLight => {
            e.insert(SpotLight {
                intensity: 150_000.0,
                range: 25.0,
                shadows_enabled: true,
                ..default()
            });
        }
    }
}

fn remove_component(world: &mut World, entity: Entity, kind: CompKind) {
    let mut e = world.entity_mut(entity);
    match kind {
        CompKind::PrimitiveMesh => {
            e.remove::<PrimitiveMesh>().remove::<Mesh3d>();
        }
        CompKind::PbrDef => {
            e.remove::<PbrDef>()
                .remove::<MeshMaterial3d<StandardMaterial>>();
        }
        CompKind::Rotator => {
            e.remove::<Rotator>();
        }
        CompKind::Bobber => {
            e.remove::<Bobber>();
        }
        CompKind::PointLight => {
            e.remove::<PointLight>();
        }
        CompKind::DirectionalLight => {
            e.remove::<DirectionalLight>();
        }
        CompKind::SpotLight => {
            e.remove::<SpotLight>();
        }
    }
}

// Re-exports used by the UI module.
pub use bevy::input::keyboard::KeyCode;
