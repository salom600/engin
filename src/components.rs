//! Editor-side component definitions and the systems that keep them in sync
//! with the actual render components (meshes / materials).
//!
//! Design: user-facing scene data is stored in small, fully serializable
//! components (`PrimitiveMesh`, `PbrDef`, `Rotator`, `Bobber`). Systems watch
//! them and create/patch the native Bevy render components (`Mesh3d`,
//! `MeshMaterial3d<StandardMaterial>`). This keeps scene files self-contained
//! (no opaque asset handles in the .scn.ron).

use crate::editor::{EditorState, PlayState};
use bevy::prelude::*;
use bevy::render::render_resource::FaceCullMode;

// ---------------------------------------------------------------------------
// Marker components
// ---------------------------------------------------------------------------

/// Marks every entity that belongs to the user's scene (saved / loaded / snapshotted).
#[derive(Component, Debug, Clone, Copy)]
pub struct SceneRoot;

/// Marks editor-internal entities (editor camera etc.) — hidden from the UI.
#[derive(Component, Debug, Clone, Copy)]
pub struct EditorInternal;

/// Marks the editor's orbit camera.
#[derive(Component, Debug, Clone, Copy)]
pub struct EditorCamera;

// ---------------------------------------------------------------------------
// Scene content components (all serializable)
// ---------------------------------------------------------------------------

#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[reflect(Component)]
pub enum PrimitiveShape {
    #[default]
    Cube,
    Sphere,
    Plane,
    Torus,
    Cylinder,
    Cone,
    Capsule,
    Icosphere,
}

impl PrimitiveShape {
    pub const ALL: [PrimitiveShape; 8] = [
        PrimitiveShape::Cube,
        PrimitiveShape::Sphere,
        PrimitiveShape::Plane,
        PrimitiveShape::Torus,
        PrimitiveShape::Cylinder,
        PrimitiveShape::Cone,
        PrimitiveShape::Capsule,
        PrimitiveShape::Icosphere,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            PrimitiveShape::Cube => "Cube",
            PrimitiveShape::Sphere => "Sphere",
            PrimitiveShape::Plane => "Plane",
            PrimitiveShape::Torus => "Torus",
            PrimitiveShape::Cylinder => "Cylinder",
            PrimitiveShape::Cone => "Cone",
            PrimitiveShape::Capsule => "Capsule",
            PrimitiveShape::Icosphere => "Icosphere",
        }
    }
}

/// Which primitive mesh an entity renders. Synced to a native `Mesh3d`.
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component)]
pub struct PrimitiveMesh {
    pub shape: PrimitiveShape,
}

/// Material definition synced to a native `MeshMaterial3d<StandardMaterial>`.
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct PbrDef {
    pub base_color: Color,
    pub emissive: Color,
    pub metallic: f32,
    pub perceptual_roughness: f32,
    pub unlit: bool,
    pub double_sided: bool,
}

impl Default for PbrDef {
    fn default() -> Self {
        Self {
            base_color: Color::srgb(0.8, 0.8, 0.8),
            emissive: Color::BLACK,
            metallic: 0.0,
            perceptual_roughness: 0.5,
            unlit: false,
            double_sided: false,
        }
    }
}

impl From<&PbrDef> for StandardMaterial {
    fn from(def: &PbrDef) -> Self {
        Self {
            base_color: def.base_color,
            emissive: def.emissive,
            metallic: def.metallic,
            perceptual_roughness: def.perceptual_roughness,
            unlit: def.unlit,
            cull_mode: if def.double_sided {
                FaceCullMode::None
            } else {
                FaceCullMode::Back
            },
            ..default()
        }
    }
}

/// Gameplay: continuously rotates the entity (active in Play mode).
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Rotator {
    /// Degrees per second around `axis`.
    pub speed_deg_per_sec: f32,
    pub axis: Vec3,
}

impl Default for Rotator {
    fn default() -> Self {
        Self {
            speed_deg_per_sec: 45.0,
            axis: Vec3::Y,
        }
    }
}

/// Gameplay: bobs the entity up and down around its spawn height (Play mode).
#[derive(Component, Reflect, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Bobber {
    pub amplitude: f32,
    /// Cycles per second.
    pub frequency: f32,
    pub phase: f32,
    /// Runtime-captured base height; never serialized.
    #[reflect(skip_serializing)]
    pub base_y: f32,
}

impl Default for Bobber {
    fn default() -> Self {
        Self {
            amplitude: 0.5,
            frequency: 0.5,
            phase: 0.0,
            base_y: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Mesh building / bounds
// ---------------------------------------------------------------------------

pub fn build_mesh(shape: PrimitiveShape) -> Mesh {
    match shape {
        PrimitiveShape::Cube => Cuboid::default().mesh().build(),
        PrimitiveShape::Sphere => Sphere::default().mesh().uv(32, 18),
        PrimitiveShape::Plane => Plane3d::default().mesh().build(),
        PrimitiveShape::Torus => Torus::default().mesh().build(),
        PrimitiveShape::Cylinder => Cylinder::default().mesh().build(),
        PrimitiveShape::Cone => Cone::default().mesh().build(),
        PrimitiveShape::Capsule => Capsule3d::default().mesh().build(),
        PrimitiveShape::Icosphere => Sphere::default().mesh().ico(3).unwrap_or_default(),
    }
}

/// Approximate local-space center / half-extents per shape; used for picking
/// and selection outlines.
pub fn shape_bounds(shape: PrimitiveShape) -> (Vec3, Vec3) {
    let (c, h) = match shape {
        PrimitiveShape::Cube => (Vec3::ZERO, Vec3::splat(0.5)),
        PrimitiveShape::Sphere | PrimitiveShape::Icosphere => (Vec3::ZERO, Vec3::splat(1.0)),
        PrimitiveShape::Plane => (Vec3::ZERO, Vec3::new(0.5, 0.05, 0.5)),
        PrimitiveShape::Torus => (Vec3::ZERO, Vec3::new(0.65, 0.65, 0.65)),
        PrimitiveShape::Cylinder | PrimitiveShape::Cone => (Vec3::ZERO, Vec3::splat(0.5)),
        PrimitiveShape::Capsule => (Vec3::ZERO, Vec3::new(0.5, 1.0, 0.5)),
    };
    (c, h)
}

// ---------------------------------------------------------------------------
// What can be spawned
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpawnKind {
    Empty,
    Shape(PrimitiveShape),
    PointLight,
    DirectionalLight,
    SpotLight,
}

impl SpawnKind {
    pub fn label(&self) -> String {
        match self {
            SpawnKind::Empty => "Empty Entity".into(),
            SpawnKind::Shape(s) => s.label().to_string(),
            SpawnKind::PointLight => "Point Light".into(),
            SpawnKind::DirectionalLight => "Directional Light".into(),
            SpawnKind::SpotLight => "Spot Light".into(),
        }
    }
}

/// Spawns a scene entity at `translation` and returns it.
pub fn spawn_scene_entity(world: &mut World, name: &str, kind: SpawnKind, translation: Vec3) -> Entity {
    let mut ec = world.spawn((
        Name::new(name.to_string()),
        Transform::from_translation(translation),
        Visibility::default(),
        SceneRoot,
    ));
    match kind {
        SpawnKind::Empty => {}
        SpawnKind::Shape(shape) => {
            ec.insert(PrimitiveMesh { shape });
            ec.insert(PbrDef::default());
        }
        SpawnKind::PointLight => {
            ec.insert(PointLight {
                intensity: 80_000.0,
                range: 20.0,
                shadows_enabled: true,
                ..default()
            });
        }
        SpawnKind::DirectionalLight => {
            ec.insert(DirectionalLight {
                shadows_enabled: true,
                ..default()
            });
        }
        SpawnKind::SpotLight => {
            ec.insert(SpotLight {
                intensity: 150_000.0,
                range: 25.0,
                shadows_enabled: true,
                ..default()
            });
        }
    }
    ec.id()
}

// ---------------------------------------------------------------------------
// Sync systems: editor components -> native render components
// ---------------------------------------------------------------------------

pub fn sync_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(Entity, &PrimitiveMesh), Or<(Added<PrimitiveMesh>, Changed<PrimitiveMesh>)>>,
) {
    for (entity, pm) in &q {
        let handle = meshes.add(build_mesh(pm.shape));
        commands.entity(entity).insert(Mesh3d(handle));
    }
}

/// Ensures a material asset handle exists for entities that just got a `PbrDef`.
pub fn ensure_materials(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    q: Query<(Entity, &PbrDef), Added<PbrDef>>,
) {
    for (entity, def) in &q {
        let handle = materials.add(StandardMaterial::from(def));
        commands.entity(entity).insert(MeshMaterial3d(handle));
    }
}

/// Patches the material asset when `PbrDef` changes.
pub fn sync_materials(
    materials: Option<ResMut<Assets<StandardMaterial>>>,
    q: Query<(&PbrDef, &MeshMaterial3d<StandardMaterial>), Changed<PbrDef>>,
) {
    let Some(mut materials) = materials else { return };
    for (def, handle) in &q {
        if let Some(mat) = materials.get_mut(&handle.0) {
            *mat = StandardMaterial::from(def);
        }
    }
}

/// Captures the spawn height of bobbing entities.
pub fn capture_bobber_base(mut q: Query<(&Transform, &mut Bobber), Added<Bobber>>) {
    for (transform, mut bobber) in &mut q {
        bobber.base_y = transform.translation.y;
    }
}

// ---------------------------------------------------------------------------
// Gameplay systems (run in Play mode)
// ---------------------------------------------------------------------------

pub fn rotator_system(time: Res<Time>, state: Res<EditorState>, mut q: Query<(&Rotator, &mut Transform)>) {
    if state.play != PlayState::Playing {
        return;
    }
    let dt = time.delta_secs();
    for (rotator, mut transform) in &mut q {
        let axis = rotator.axis.normalize_or_zero();
        if axis == Vec3::ZERO {
            continue;
        }
        transform.rotate_axis(axis, rotator.speed_deg_per_sec.to_radians() * dt);
    }
}

pub fn bobber_system(time: Res<Time>, state: Res<EditorState>, mut q: Query<(&Bobber, &mut Transform)>) {
    if state.play != PlayState::Playing {
        return;
    }
    let t = time.elapsed_secs();
    for (bobber, mut transform) in &mut q {
        transform.translation.y =
            bobber.base_y + bobber.amplitude * (t * bobber.frequency * std::f32::consts::TAU + bobber.phase).sin();
    }
}
