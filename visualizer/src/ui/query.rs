use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use bevy_panorbit_camera::PanOrbitCamera;

use crate::resources::{QueryPanelState, QueryScope, QueryType, SelectionState};

/// Returns the display label for a [`QueryType`] variant.
fn query_type_label(qt: &QueryType) -> &'static str {
    match qt {
        QueryType::NodesByFeature => "Nodes by Feature",
        QueryType::NodesByKey => "Nodes by Key",
        QueryType::EdgesByLabel => "Edges by Label",
        QueryType::EdgesFrom => "Edges From",
        QueryType::EdgesTo => "Edges To",
        QueryType::NearestNode => "Nearest Node",
        QueryType::NodesWithin => "Nodes Within Radius",
        QueryType::VisibleSubgraph => "Visible Subgraph",
    }
}

/// Returns the display label for a [`QueryScope`] variant.
fn scope_label(scope: &QueryScope) -> &'static str {
    match scope {
        QueryScope::Graph => "Graph",
        QueryScope::Layer => "Layer",
        QueryScope::Node => "Node",
    }
}

/// Draws the right-side query panel.
///
/// Contains a query-type `ComboBox`, a scope `ComboBox` (Layer/Node are grayed
/// out when no selection exists), dynamic input fields that change based on the
/// selected query type, a Run button that sets `QueryPanelState.submitted`, and
/// a scroll area showing result node/edge IDs from the last executed query.
pub fn draw_query_panel(
    mut contexts: EguiContexts,
    mut qps: ResMut<QueryPanelState>,
    selection: Res<SelectionState>,
    camera_query: Query<&Transform, With<PanOrbitCamera>>,
) {
    let can_scope_layer = selection.selected_layer.is_some();
    let can_scope_node = selection.selected_node.is_some();

    // Downgrade scope if the backing selection was cleared.
    if qps.scope == QueryScope::Layer && !can_scope_layer {
        qps.scope = QueryScope::Graph;
    }
    if qps.scope == QueryScope::Node && !can_scope_node {
        qps.scope = QueryScope::Graph;
    }

    egui::SidePanel::right("query")
        .resizable(true)
        .default_width(240.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Query");
            ui.separator();

            // --- Query type ---
            egui::ComboBox::from_label("Type")
                .selected_text(query_type_label(&qps.query_type))
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::NodesByFeature,
                        "Nodes by Feature",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::NodesByKey,
                        "Nodes by Key",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::EdgesByLabel,
                        "Edges by Label",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::EdgesFrom,
                        "Edges From",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::EdgesTo,
                        "Edges To",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::NearestNode,
                        "Nearest Node",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::NodesWithin,
                        "Nodes Within Radius",
                    );
                    ui.selectable_value(
                        &mut qps.query_type,
                        QueryType::VisibleSubgraph,
                        "Visible Subgraph",
                    );
                });

            // --- Scope ---
            egui::ComboBox::from_label("Scope")
                .selected_text(scope_label(&qps.scope))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut qps.scope, QueryScope::Graph, "Graph");
                    ui.add_enabled_ui(can_scope_layer, |ui| {
                        ui.selectable_value(&mut qps.scope, QueryScope::Layer, "Layer");
                    });
                    ui.add_enabled_ui(can_scope_node, |ui| {
                        ui.selectable_value(&mut qps.scope, QueryScope::Node, "Node");
                    });
                });

            ui.separator();

            // --- Dynamic input fields ---
            match qps.query_type {
                QueryType::NodesByFeature => {
                    ui.label("Key:");
                    ui.text_edit_singleline(&mut qps.feature_key);
                    ui.label("Value:");
                    ui.text_edit_singleline(&mut qps.feature_value);
                }
                QueryType::NodesByKey => {
                    ui.label("Key:");
                    ui.text_edit_singleline(&mut qps.feature_key);
                }
                QueryType::EdgesByLabel => {
                    ui.label("Label:");
                    ui.text_edit_singleline(&mut qps.edge_label);
                }
                QueryType::EdgesFrom | QueryType::EdgesTo => {
                    ui.label("Node ID (layer:node):");
                    ui.text_edit_singleline(&mut qps.root_node);
                    if let Some(nid) = selection.selected_node {
                        if ui.small_button("Use selection").clicked() {
                            qps.root_node =
                                format!("{}:{}", nid.layer_id(), nid.node_id());
                        }
                    }
                }
                QueryType::NearestNode => {
                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[0])
                                .desired_width(52.0),
                        );
                        ui.label("y");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[1])
                                .desired_width(52.0),
                        );
                        ui.label("z");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[2])
                                .desired_width(52.0),
                        );
                    });
                    ui.label("Count:");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            !qps.use_all,
                            egui::TextEdit::singleline(&mut qps.count).desired_width(60.0),
                        );
                        ui.checkbox(&mut qps.use_all, "All");
                    });
                }
                QueryType::NodesWithin => {
                    ui.label("Position:");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[0])
                                .desired_width(52.0),
                        );
                        ui.label("y");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[1])
                                .desired_width(52.0),
                        );
                        ui.label("z");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.coord[2])
                                .desired_width(52.0),
                        );
                    });
                    ui.label("Radius:");
                    ui.text_edit_singleline(&mut qps.radius);
                    ui.label("Count:");
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            !qps.use_all,
                            egui::TextEdit::singleline(&mut qps.count).desired_width(60.0),
                        );
                        ui.checkbox(&mut qps.use_all, "All");
                    });
                }
                QueryType::VisibleSubgraph => {
                    ui.label("Observer Position:");
                    ui.horizontal(|ui| {
                        ui.label("x");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_pos[0])
                                .desired_width(52.0),
                        );
                        ui.label("y");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_pos[1])
                                .desired_width(52.0),
                        );
                        ui.label("z");
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_pos[2])
                                .desired_width(52.0),
                        );
                    });
                    ui.label("Yaw / Pitch / Roll (degrees):");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_yaw)
                                .desired_width(52.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_pitch)
                                .desired_width(52.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_roll)
                                .desired_width(52.0),
                        );
                    });
                    ui.label("Half-angle (degrees):");
                    ui.text_edit_singleline(&mut qps.fov_half_angle);
                    ui.label("Near / Far:");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_near)
                                .desired_width(72.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut qps.fov_far)
                                .desired_width(72.0),
                        );
                    });
                    ui.label("Root Node (layer:node):");
                    ui.text_edit_singleline(&mut qps.root_node);
                    if ui.button("Use Camera").clicked() {
                        if let Ok(transform) = camera_query.get_single() {
                            let p = transform.translation;
                            qps.fov_pos[0] = format!("{:.3}", p.x);
                            qps.fov_pos[1] = format!("{:.3}", p.y);
                            qps.fov_pos[2] = format!("{:.3}", p.z);
                            let (yaw, pitch, roll) =
                                transform.rotation.to_euler(EulerRot::YXZ);
                            qps.fov_yaw = format!("{:.2}", yaw.to_degrees());
                            qps.fov_pitch = format!("{:.2}", pitch.to_degrees());
                            qps.fov_roll = format!("{:.2}", roll.to_degrees());
                        }
                    }
                }
            }

            ui.separator();

            // --- Run button ---
            if ui.button("▶  Run Query").clicked() {
                qps.submitted = true;
                qps.error = None;
                qps.highlighted_nodes.clear();
                qps.highlighted_edges.clear();
            }

            // --- Error ---
            if let Some(ref err) = qps.error.clone() {
                ui.colored_label(egui::Color32::RED, err);
            }

            // --- Results ---
            let total = qps.highlighted_nodes.len() + qps.highlighted_edges.len();
            if total > 0 {
                ui.separator();
                ui.label(format!("{} result(s)", total));
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        for nid in &qps.highlighted_nodes {
                            ui.label(format!(
                                "Node {}:{}",
                                nid.layer_id(),
                                nid.node_id()
                            ));
                        }
                        for (src, dst) in &qps.highlighted_edges {
                            ui.label(format!(
                                "Edge {}:{} → {}:{}",
                                src.layer_id(),
                                src.node_id(),
                                dst.layer_id(),
                                dst.node_id()
                            ));
                        }
                    });
            }
        });
}
