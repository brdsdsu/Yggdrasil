use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use scene_graph::Bounds;

use crate::resources::{HoverState, LayoutPositions, SceneGraphRes};

/// Draws the properties panel as a floating, draggable egui window that appears
/// whenever a node is hovered.
///
/// `constrain_to` is applied every frame using the current `available_rect`,
/// so the window can never slide behind the layers panel regardless of how the
/// user resizes it. The window is still freely draggable within the remaining
/// central area.
pub fn draw_properties_panel(
    mut contexts: EguiContexts,
    hover: Res<HoverState>,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
) {
    let Some(hovered_id) = hover.hovered_node else {
        return;
    };

    let sg = &sg_res.0;
    let Ok(node) = sg.node(hovered_id) else {
        return;
    };

    let ctx = contexts.ctx_mut();

    // Left boundary is the right edge of the layers panel plus a small gap.
    // available_rect() is updated every frame after side panels are allocated,
    // so this adapts automatically when the layers panel is resized.
    let left_bound = ctx.available_rect().left() + 16.0;
    let screen = ctx.screen_rect();
    let constrain = egui::Rect::from_min_max(
        egui::pos2(left_bound, screen.min.y),
        screen.max,
    );

    egui::Window::new("Properties")
        .default_pos(egui::pos2(left_bound, 8.0))
        .constrain_to(constrain)
        .resizable(true)
        .collapsible(true)
        .min_width(200.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
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

                        if let Some(pid) = node.pid {
                            ui.label("Parent");
                            ui.label(format!("{}:{}", pid.layer_id(), pid.node_id()));
                            ui.end_row();
                        }
                    });

                // Coordinates
                ui.separator();
                if node.coordinates.is_empty() {
                    ui.label("Coordinates: none");
                } else {
                    ui.label(format!("Coordinates ({})", node.coordinates.len()));
                    // The anchor is where this node's edges and selection ring attach,
                    // so mark which coordinate it is.
                    let anchor_index = layout
                        .anchor(hovered_id)
                        .and_then(|a| layout.coords(hovered_id).iter().position(|&c| c == a));
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .id_salt("coords_scroll")
                        .show(ui, |ui| {
                            egui::Grid::new("coord_grid")
                                .num_columns(2)
                                .spacing([8.0, 2.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    for (i, coord) in node.coordinates.iter().enumerate() {
                                        if Some(i) == anchor_index {
                                            ui.label(format!("{i} ⚓"));
                                        } else {
                                            ui.label(i.to_string());
                                        }
                                        ui.label(format!(
                                            "({:.3}, {:.3}, {:.3})",
                                            coord.x, coord.y, coord.z
                                        ));
                                        ui.end_row();
                                    }
                                });
                        });
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
                        .id_salt("edges_scroll")
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
        });
}
