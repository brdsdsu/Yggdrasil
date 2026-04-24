use bevy::prelude::*;

use crate::{
    components::{LayerTag, NodeSphere, Opacity, VisPos},
    resources::{LayoutPositions, MaterialHandles, SceneGraphRes, VisibilityToggles},
};

/// Spawns the 3D camera with orbit/pan/zoom controls centered on the scene.
pub fn spawn_camera(mut commands: Commands, layout: Res<LayoutPositions>) {
    // Focus the camera on the centroid of all node positions.
    let focus = if layout.0.is_empty() {
        Vec3::ZERO
    } else {
        let sum: Vec3 = layout.0.values().copied().sum();
        sum / layout.0.len() as f32
    };

    commands.spawn((
        Camera3d::default(),
        bevy_panorbit_camera::PanOrbitCamera {
            focus,
            ..default()
        },
        Transform::from_translation(focus + Vec3::new(0.0, 10.0, 30.0))
            .looking_at(focus, Vec3::Y),
    ));
}

/// Spawns a directional light and sets the scene ambient light.
pub fn spawn_lights(mut commands: Commands, mut ambient: ResMut<AmbientLight>) {
    ambient.brightness = 300.0;
    ambient.color = Color::WHITE;

    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Spawns one sphere mesh entity per scene graph node.
///
/// Each sphere is positioned at the node's computed visual position, colored by
/// its layer using an HSV palette, and given alpha-blend materials so the hover
/// fade system can independently control per-node opacity.
pub fn spawn_node_spheres(
    mut commands: Commands,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut mat_handles: ResMut<MaterialHandles>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count().max(1) as f32;
    // Sphere radius scales with z-separation so it remains legible at any scene scale.
    let radius = (vis.z_separation * 0.05).max(0.1);
    let sphere_mesh = meshes.add(Sphere::new(radius));

    for layer in sg.layers() {
        // Spread layer hues evenly around the HSL color wheel.
        let hue = (layer.id as f32 / num_layers) * 360.0;
        let layer_color = Color::hsl(hue, 0.75, 0.55);
        let initially_visible = vis.layer_visible.get(&layer.id).copied().unwrap_or(true);

        for node in layer.all_nodes() {
            let pos = layout.0.get(&node.id).copied().unwrap_or(Vec3::ZERO);

            // Each node gets its own material instance so opacity can be set independently.
            let mat = materials.add(StandardMaterial {
                base_color: layer_color,
                alpha_mode: AlphaMode::Blend,
                ..default()
            });
            mat_handles.0.insert(node.id, mat.clone());

            commands.spawn((
                Mesh3d(sphere_mesh.clone()),
                MeshMaterial3d(mat),
                Transform::from_translation(pos),
                NodeSphere { node_id: node.id },
                LayerTag { layer_id: layer.id },
                VisPos(pos),
                Opacity { target: 1.0, current: 1.0 },
                if initially_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }
}
