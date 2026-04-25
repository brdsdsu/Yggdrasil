use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use scene_graph::Bounds;

use crate::resources::{HoverState, SceneGraphRes};

/// Draws a floating properties window anchored to the top-right corner of the
/// viewport whenever a node is hovered.
///
/// Displays the node's layer-local and global ID, 3D coordinates, bounding box
/// extents, all key-value features, and the list of outgoing intra-layer edges.
/// The window is hidden entirely when nothing is hovered.
pub fn draw_properties_panel(
    mut contexts: EguiContexts,
    hover: Res<HoverState>,
    sg_res: Res<SceneGraphRes>,
) {
    let Some(hovered_id) = hover.hovered_node else {
        return;
    };

    let sg = &sg_res.0;
    let Ok(node) = sg.node(hovered_id) else {
        return;
    };

    // Position the window just to the right of the left panel.
    // available_rect() reflects the central area after all side panels are allocated,
    // so left() gives the exact x where the left panel ends.
    let ctx = contexts.ctx_mut();
    let start_x = ctx.available_rect().left() + 8.0;

    egui::Window::new("Properties")
        .default_pos(egui::pos2(start_x, 8.0))
        .resizable(true)
        .collapsible(true)
        .min_width(220.0)
        .show(ctx, |ui| {
            // Identity
            egui::Grid::new("identity_grid")
                .num_columns(2)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    ui.label("Layer");
                    ui.label(hovered_id.layer_id().to_string());
                    ui.end_row();

                    ui.label("Node");
                    ui.label(hovered_id.node_id().to_string());
                    ui.end_row();

                    // Parent
                    if let Some(pid) = node.pid {
                        ui.label("Parent");
                        ui.label(format!("{}:{}", pid.layer_id(), pid.node_id()));
                        ui.end_row();
                    }
                });

            // Coordinates
            ui.separator();
            if let Some(coord) = node.coordinates {
                ui.label("Coordinates");
                egui::Grid::new("coord_grid")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        ui.label("x");
                        ui.label(format!("{:.3}", coord.x));
                        ui.end_row();
                        ui.label("y");
                        ui.label(format!("{:.3}", coord.y));
                        ui.end_row();
                        ui.label("z");
                        ui.label(format!("{:.3}", coord.z));
                        ui.end_row();
                    });
            } else {
                ui.label("Coordinates: none");
            }

            // Bounding box
            if let Some(Bounds::Bounded(bb)) = node.bounds {
                ui.separator();
                ui.label("Bounds");
                let min = bb.min();
                let max = bb.max();
                egui::Grid::new("bounds_grid")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .show(ui, |ui| {
                        ui.label("min");
                        ui.label(format!("({:.2}, {:.2}, {:.2})", min.x, min.y, min.z));
                        ui.end_row();
                        ui.label("max");
                        ui.label(format!("({:.2}, {:.2}, {:.2})", max.x, max.y, max.z));
                        ui.end_row();
                    });
            }

            // Features
            if !node.features.is_empty() {
                ui.separator();
                ui.label("Features");
                egui::Grid::new("features_grid")
                    .num_columns(2)
                    .spacing([8.0, 2.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for f in &node.features {
                            ui.label(f.key());
                            ui.label(f.value());
                            ui.end_row();
                        }
                    });
            }

            // Outgoing edges
            if !node.edges.is_empty() {
                ui.separator();
                ui.label("Edges");
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for edge in &node.edges {
                            ui.label(format!(
                                "{}:{} → {}:{} [{}]",
                                edge.src.layer_id(),
                                edge.src.node_id(),
                                edge.dst.layer_id(),
                                edge.dst.node_id(),
                                edge.desc,
                            ));
                        }
                    });
            }
        });
}
