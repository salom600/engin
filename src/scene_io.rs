//! Scene serialization: save / load / snapshot user scenes as Bevy RON scene
//! files (`.scn.ron`).
//!
//! Only the small serializable editor components are written (see the allow
//! list), so files stay clean and portable; render components (meshes,
//! material assets) are rebuilt from them on load.

use crate::components::*;
use bevy::ecs::entity::EntityHashMap;
use bevy::hierarchy::{Children, Parent};
use bevy::prelude::*;
use bevy::scene::DynamicSceneBuilder;
use std::path::Path;

pub type SceneIoResult<T> = Result<T, String>;

/// Builds a snapshot of every `SceneEntity` entity.
pub fn snapshot_scene(world: &World) -> DynamicScene {
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<SceneEntity>>()
        .iter(world)
        .collect();

    DynamicSceneBuilder::from_world(world)
        .deny_all_resources()
        .deny_all_components()
        .allow_component::<Name>()
        .allow_component::<Transform>()
        .allow_component::<Visibility>()
        .allow_component::<Parent>()
        .allow_component::<Children>()
        .allow_component::<PrimitiveMesh>()
        .allow_component::<PrimitiveShape>()
        .allow_component::<PbrDef>()
        .allow_component::<Rotator>()
        .allow_component::<Bobber>()
        .allow_component::<DirectionalLight>()
        .allow_component::<PointLight>()
        .allow_component::<SpotLight>()
        .extract_entities(entities.into_iter())
        .remove_empty_entities()
        .build()
}

/// Despawns every scene entity.
pub fn clear_scene(world: &mut World) {
    let roots: Vec<Entity> = world
        .query_filtered::<Entity, (With<SceneEntity>, Without<Parent>)>()
        .iter(world)
        .collect();
    for entity in roots {
        let _ = world.entity_mut(entity).despawn_recursive();
    }
}

pub fn save_scene(world: &World, path: &Path) -> SceneIoResult<()> {
    let scene = snapshot_scene(world);
    let registry = world.resource::<AppTypeRegistry>();
    let serialized = scene
        .serialize(&registry.read())
        .map_err(|e| format!("serialization failed: {e}"))?;
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))
        .map_err(|e| format!("cannot create folder: {e}"))?;
    std::fs::write(path, serialized).map_err(|e| format!("cannot write file: {e}"))?;
    Ok(())
}

/// Replaces the current scene with the one stored in `path`.
/// Returns the number of entities spawned.
pub fn load_scene(world: &mut World, path: &Path) -> SceneIoResult<usize> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read file: {e}"))?;
    let scene: DynamicScene =
        ron::de::from_str(&text).map_err(|e| format!("invalid scene file: {e}"))?;
    clear_scene(world);
    let mut map = EntityHashMap::default();
    scene
        .write_to_world(world, &mut map)
        .map_err(|e| format!("cannot spawn scene: {e}"))?;
    for spawned in map.values() {
        if let Ok(mut entity) = world.get_entity_mut(*spawned) {
            entity.insert(SceneEntity);
        }
    }
    Ok(map.len())
}

/// Deep-clones an entity subtree. Copies stay under the same parent
/// (or at root level for root entities).
pub fn duplicate_entity_tree(world: &mut World, source: Entity) -> Option<Entity> {
    duplicate_tree_inner(world, source, None, true)
}

fn duplicate_tree_inner(
    world: &mut World,
    source: Entity,
    parent_override: Option<Entity>,
    rename: bool,
) -> Option<Entity> {
    if !world.entities().contains(source) {
        return None;
    }
    let original_parent = world.get::<Parent>(source).map(|p| p.get());
    let children: Vec<Entity> = world
        .get::<Children>(source)
        .map(|c| c.iter().copied().collect())
        .unwrap_or_default();

    let name = world.get::<Name>(source).map(|n| {
        if rename {
            Name::new(format!("{} (copy)", n.as_str()))
        } else {
            n.clone()
        }
    });

    let mut entity = world.spawn((
        name.unwrap_or_else(|| Name::new("Entity")),
        Transform::default(),
        Visibility::default(),
        SceneEntity,
    ));
    let new_id = entity.id();

    copy_component::<Transform>(world, source, new_id);
    copy_component::<Visibility>(world, source, new_id);
    copy_component::<PrimitiveMesh>(world, source, new_id);
    copy_component::<PbrDef>(world, source, new_id);
    copy_component::<Rotator>(world, source, new_id);
    copy_component::<Bobber>(world, source, new_id);
    copy_component::<DirectionalLight>(world, source, new_id);
    copy_component::<PointLight>(world, source, new_id);
    copy_component::<SpotLight>(world, source, new_id);

    let attach_to = parent_override.or(original_parent);
    if let Some(parent) = attach_to {
        if world.entities().contains(parent) {
            world.entity_mut(parent).add_child(new_id);
        }
    }

    for child in children {
        duplicate_tree_inner(world, child, Some(new_id), false);
    }

    Some(new_id)
}

fn copy_component<T: Component + Clone>(world: &mut World, source: Entity, target: Entity) {
    let component = world.get::<T>(source).cloned();
    if let Some(component) = component {
        world.entity_mut(target).insert(component);
    }
}
