use bevy::prelude::*;

use crate::{
    components::{CoordIndex, LayerTag, NodeSphere, VisPos},
    resources::{LayerMaterials, LayoutPositions, SceneGraphRes, SphereMeshHandle, VisibilityToggles},
    theme::{Theme, layer_hue},
};

/// Spawns the 3D camera with orbit/pan/zoom controls centered on the scene.
///
/// The camera is placed in front of the scene along the +Z axis, far enough to
/// see the full vertical extent of all layers (which are separated on the Y axis).
pub fn spawn_camera(mut commands: Commands, layout: Res<LayoutPositions>) {
    let (focus, distance) = if layout.0.is_empty() {
        (Vec3::ZERO, 50.0_f32)
    } else {
        let sum: Vec3 = layout.0.values().map(|l| l.anchor).sum();
        let center = sum / layout.0.len() as f32;

        let (y_min, y_max) = layout.0.values().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(mn, mx), l| (mn.min(l.anchor.y), mx.max(l.anchor.y)),
        );
        // Distance chosen so the full layer stack fits in the default 45° vertical FOV.
        let y_span = (y_max - y_min).max(10.0);
        let dist = (y_span * 2.0).max(20.0);

        (center, dist)
    };

    commands.spawn((
        Camera3d::default(),
        bevy_panorbit_camera::PanOrbitCamera {
            focus,
            ..default()
        },
        Transform::from_translation(focus + Vec3::new(0.0, 0.0, distance))
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

/// Spawns one sphere mesh entity per node coordinate.
///
/// A node holds a list of coordinates, so it gets one sphere per coordinate — all of
/// them tagged with the same [`NodeSphere::node_id`] and distinguished only by their
/// [`CoordIndex`]. Nodes with no coordinates (semantic and root nodes) get a single
/// sphere at their anchor, exactly as before.
///
/// All spheres within the same layer share a single `StandardMaterial` so the GPU
/// bind-group count is O(n_layers) instead of O(n_spheres).
pub fn spawn_node_spheres(
    mut commands: Commands,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    theme: Res<Theme>,
    mut layer_mats: ResMut<LayerMaterials>,
    mut sphere_handle: ResMut<SphereMeshHandle>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count();
    let sphere_mesh = meshes.add(Sphere::new(vis.node_radius));
    sphere_handle.0 = sphere_mesh.clone();

    for layer in sg.layers() {
        let hue = layer_hue(layer.id, num_layers);
        let layer_color = theme.node(hue);
        let initially_visible = vis.layer_visible.get(&layer.id).copied().unwrap_or(true);

        // One material shared by every node in this layer.
        let mat = materials.add(StandardMaterial {
            base_color: layer_color,
            ..default()
        });
        layer_mats.0.insert(layer.id, mat.clone());

        for node in layer.all_nodes() {
            // A coordinate-less node still needs exactly one sphere, placed at its anchor.
            let anchor = layout.anchor(node.id).unwrap_or(Vec3::ZERO);
            let coords = layout.coords(node.id);
            let positions: &[Vec3] = if coords.is_empty() {
                std::slice::from_ref(&anchor)
            } else {
                coords
            };

            for (index, &pos) in positions.iter().enumerate() {
                commands.spawn((
                    Mesh3d(sphere_mesh.clone()),
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation(pos),
                    NodeSphere { node_id: node.id },
                    CoordIndex(index),
                    LayerTag { layer_id: layer.id },
                    VisPos(pos),
                    if initially_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ));
            }
        }
    }
}
