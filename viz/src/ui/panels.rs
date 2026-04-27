use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::components::NodeSphere;
use crate::layout::compute_layout;
use crate::resources::{LayoutPositions, SceneGraphRes, SphereMeshHandle, VisibilityToggles};

/// Draws the left sidebar panel containing per-layer visibility toggles,
/// the AABB wireframe toggle, and the z-separation slider.
///
/// When the z-separation slider is moved, the layout is recomputed immediately
/// and all node sphere transforms are updated in place so the scene responds
/// in real time without requiring a restart.
pub fn draw_layer_panel(
    mut contexts: EguiContexts,
    mut vis: ResMut<VisibilityToggles>,
    sg_res: Res<SceneGraphRes>,
    mut layout: ResMut<LayoutPositions>,
    mut sphere_query: Query<(&NodeSphere, &mut Transform)>,
    sphere_handle: Res<SphereMeshHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
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
            ui.checkbox(&mut vis.show_nesting, "Show Hierarchical Edges");

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
                for (sphere, mut transform) in &mut sphere_query {
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
                egui::Slider::new(&mut vis.node_radius, 0.01f32..=10.0)
                    .logarithmic(true)
                    .text("r"),
            );

            if (vis.node_radius - old_radius).abs() > 1e-6 {
                if let Some(mesh) = meshes.get_mut(&sphere_handle.0) {
                    *mesh = Sphere::new(vis.node_radius).mesh().build();
                }
            }
        });
}
