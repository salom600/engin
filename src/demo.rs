//! The demo scene shown on first launch.

use crate::components::*;
use bevy::prelude::*;

/// Spawns a fresh, empty scene (one sun light) — used by "New Scene".
pub fn spawn_default_scene(world: &mut World) {
    let sun = spawn_scene_entity(world, "Sun", SpawnKind::DirectionalLight, Vec3::new(4.0, 8.0, 4.0));
    if let Ok(mut entity) = world.get_entity_mut(sun) {
        entity.insert(Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y));
    }
}

/// Spawns the demo scene.
pub fn spawn_demo_world(world: &mut World) {
    // Ground plane (a scaled 1x1 plane).
    let ground = spawn_scene_entity(world, "Ground", SpawnKind::Shape(PrimitiveShape::Plane), Vec3::ZERO);
    patch(world, ground, |e| {
        e.insert(Transform::from_scale(Vec3::new(24.0, 1.0, 24.0)));
    });
    if let Some(def) = world.get::<PbrDef>(ground).cloned() {
        let mut def = def;
        def.base_color = Color::srgb(0.16, 0.17, 0.20);
        def.perceptual_roughness = 0.9;
        world.entity_mut(ground).insert(def);
    }

    // Sun.
    let sun = spawn_scene_entity(world, "Sun", SpawnKind::DirectionalLight, Vec3::new(6.0, 10.0, 4.0));
    patch(world, sun, |e| {
        e.insert(Transform::from_xyz(6.0, 10.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y));
    });

    // Center piece: polished sphere.
    let sphere = spawn_scene_entity(world, "Metal Sphere", SpawnKind::Shape(PrimitiveShape::Icosphere), Vec3::new(0.0, 1.1, 0.0));
    if let Some(mut def) = world.get::<PbrDef>(sphere).cloned() {
        def.base_color = Color::srgb(0.85, 0.86, 0.9);
        def.metallic = 0.95;
        def.perceptual_roughness = 0.12;
        world.entity_mut(sphere).insert(def);
    }

    // Ring of cubes.
    let cube_colors = [
        Color::srgb(0.89, 0.26, 0.26),
        Color::srgb(0.26, 0.65, 0.89),
        Color::srgb(0.95, 0.68, 0.2),
        Color::srgb(0.55, 0.8, 0.35),
    ];
    for i in 0..8 {
        let angle = i as f32 / 8.0 * std::f32::consts::TAU;
        let position = Vec3::new(angle.cos() * 3.2, 0.5, angle.sin() * 3.2);
        let name = format!("Cube {i}");
        let cube = spawn_scene_entity(world, &name, SpawnKind::Shape(PrimitiveShape::Cube), position);
        if let Some(mut def) = world.get::<PbrDef>(cube).cloned() {
            def.base_color = cube_colors[i % cube_colors.len()];
            def.perceptual_roughness = 0.45;
            world.entity_mut(cube).insert(def);
        }
        if i % 2 == 0 {
            let rotator = Rotator {
                speed_deg_per_sec: 30.0 + i as f32 * 8.0,
                axis: Vec3::Y,
            };
            world.entity_mut(cube).insert(rotator);
        }
    }

    // Floating emissive torus.
    let torus = spawn_scene_entity(world, "Glow Torus", SpawnKind::Shape(PrimitiveShape::Torus), Vec3::new(0.0, 3.0, 0.0));
    if let Some(mut def) = world.get::<PbrDef>(torus).cloned() {
        def.base_color = Color::srgb(0.1, 0.25, 0.35);
        def.emissive = Color::srgb(0.1, 0.75, 0.95);
        def.metallic = 0.4;
        def.perceptual_roughness = 0.3;
        world.entity_mut(torus).insert(def);
    }
    patch(world, torus, |e| {
        e.insert(Transform::from_xyz(0.0, 3.0, 0.0).with_scale(Vec3::splat(1.6)));
        e.insert(Rotator {
            speed_deg_per_sec: 60.0,
            axis: Vec3::Y,
        });
        e.insert(Bobber {
            amplitude: 0.35,
            frequency: 0.4,
            ..default()
        });
    });

    // Two accent lights.
    let cyan = spawn_scene_entity(world, "Cyan Light", SpawnKind::PointLight, Vec3::new(-4.5, 2.5, -3.0));
    if let Some(mut light) = world.get::<PointLight>(cyan).cloned() {
        light.color = Color::srgb(0.2, 0.85, 1.0);
        light.intensity = 60_000.0;
        light.shadows_enabled = false;
        world.entity_mut(cyan).insert(light);
    }
    let magenta = spawn_scene_entity(world, "Magenta Light", SpawnKind::PointLight, Vec3::new(4.5, 2.5, 3.0));
    if let Some(mut light) = world.get::<PointLight>(magenta).cloned() {
        light.color = Color::srgb(1.0, 0.3, 0.8);
        light.intensity = 60_000.0;
        light.shadows_enabled = false;
        world.entity_mut(magenta).insert(light);
    }

    info!("Demo scene spawned");
}

fn patch(world: &mut World, entity: Entity, f: impl FnOnce(&mut EntityWorldMut)) {
    if let Ok(mut e) = world.get_entity_mut(entity) {
        f(&mut e);
    }
}
