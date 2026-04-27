use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::components::NodeSphere;
use crate::layout::compute_layout;
use crate::resources::{LayoutPositions, SceneGraphRes, VisibilityToggles};

/// Draws the left sidebar panel containing per-layer visibility toggles,
/// the AABB wireframe toggle, the z-separation slider, and the node radius slider.
///
/// When the z-separation slider is moved, the layout is recomputed immediately
/// and all node sphere transforms are updated in place so the scene responds
/// in real time without requiring a restart.
///
/// When the node radius slider is moved, a new sphere mesh is created and
/// assigned to every node sphere entity so all spheres resize immediately.
pub fn draw_layer_panel(
    mut contexts: EguiContexts,
    mut vis: ResMut<VisibilityToggles>,
    sg_res: Res<SceneGraphRes>,
    mut layout: ResMut<LayoutPositions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut sphere_query: Query<(&NodeSphere, &mut Transform, &mut Mesh3d)>,
) {
    let sg = &sg_res.0;

    egui::SidePanel::left("layers")
        .resizable(true)
        .default_width(210.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Layers");
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for layer in sg.layers() {
                        let node_count = layer.all_nodes().count();
                        let visible =
                            vis.layer_visible.entry(layer.id).or_insert(true);
                        ui.checkbox(
                            visible,
                            format!("Layer {} ({} nodes)", layer.id, node_count),
                        );
                    }
                });

            ui.separator();
            ui.checkbox(&mut vis.show_aabbs, "Show AABBs");

            ui.separator();
            ui.label("Z Separation");

            let old_z_sep = vis.z_separation;
            ui.add(
                egui::Slider::new(&mut vis.z_separation, 0.1f32..=500.0)
                    .logarithmic(true)
                    .text("units"),
            );

            if (vis.z_separation - old_z_sep).abs() > 1e-4 {
                let new_positions = compute_layout(sg, vis.z_separation);
                for (sphere, mut transform, _) in &mut sphere_query {
                    if let Some(&pos) = new_positions.get(&sphere.node_id) {
                        transform.translation = pos;
                    }
                }
                layout.0 = new_positions;
            }

            ui.separator();
            ui.label("Node Radius");

            let old_radius = vis.node_radius;
            ui.add(
                egui::Slider::new(&mut vis.node_radius, 0.01f32..=20.0)
                    .logarithmic(true)
                    .text("units"),
            );

            if (vis.node_radius - old_radius).abs() > 1e-5 {
                let new_mesh = meshes.add(Sphere::new(vis.node_radius));
                for (_, _, mut mesh3d) in &mut sphere_query {
                    mesh3d.0 = new_mesh.clone();
                }
            }
        });
}
