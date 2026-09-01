use std::collections::HashSet;

use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use scene_graph::NodeId;

use crate::theme::{Theme, layer_hue};
use crate::components::{EdgeBaseMesh, EdgeHighlightMesh, NestingBaseMesh, NestingHighlightMesh};
use crate::resources::{
    EdgeMeshMaterials, HoverState, LayoutPositions, QueryPanelState, SceneGraphRes, SelectionState,
    VisibilityToggles,
};

// ── geometry helpers ─────────────────────────────────────────────────────────

/// Builds a `LineList` mesh from a flat vertex list, where each consecutive pair of
/// entries forms one line segment.
pub(crate) fn line_list_mesh(positions: Vec<[f32; 3]>) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh
}

/// Converts a `Vec3` into the raw array form the mesh position attribute expects.
pub(crate) fn v3(v: Vec3) -> [f32; 3] {
    [v.x, v.y, v.z]
}

fn build_edge_positions(layer: &scene_graph::Layer, layout: &LayoutPositions) -> Vec<[f32; 3]> {
    let mut positions = Vec::new();
    let offset = Vec3::Y * 0.02;
    for node in layer.all_nodes() {
        for edge in &node.edges {
            if let (Some(s), Some(d)) = (layout.anchor(edge.src), layout.anchor(edge.dst)) {
                positions.push(v3(s + offset));
                positions.push(v3(d + offset));
            }
        }
    }
    positions
}

fn build_nesting_positions(
    sg: &scene_graph::SceneGraph,
    layout: &LayoutPositions,
    layer_visible: &std::collections::HashMap<u32, bool>,
) -> Vec<[f32; 3]> {
    let mut positions = Vec::new();
    for layer in sg.layers() {
        if !layer_visible.get(&layer.id).copied().unwrap_or(true) {
            continue;
        }
        for node in layer.all_nodes() {
            let Some(pid) = node.pid else { continue };
            let parent_layer = pid.layer_id();
            if !layer_visible.get(&parent_layer).copied().unwrap_or(true) {
                continue;
            }
            if let (Some(c), Some(p)) = (layout.anchor(node.id), layout.anchor(pid)) {
                positions.push(v3(c));
                positions.push(v3(p));
            }
        }
    }
    positions
}

/// The set of node IDs that should stay fully drawn, or `None` when no hover,
/// selection or query state is active and everything is drawn.
///
/// Priority matches `update_node_visibility`: query results, then hover subtree,
/// then selection subtree.
pub(crate) fn active_node_set<'a>(
    hover: &'a HoverState,
    selection: &'a SelectionState,
    qps: &'a QueryPanelState,
) -> Option<&'a HashSet<NodeId>> {
    if !qps.highlighted_nodes.is_empty() {
        Some(&qps.highlighted_nodes)
    } else if hover.hovered_node.is_some() {
        Some(&hover.subtree_ids)
    } else if selection.selected_node.is_some() {
        Some(&selection.subtree_ids)
    } else {
        None
    }
}

// ── startup systems ───────────────────────────────────────────────────────────

/// Builds one base + one (empty) highlight `LineList` mesh entity per layer for
/// intra-layer edges. Replaces the per-frame `draw_intra_layer_edges` gizmo loop.
pub fn build_edge_meshes(
    mut commands: Commands,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    theme: Res<Theme>,
    mut edge_mats: ResMut<EdgeMeshMaterials>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count();

    for layer in sg.layers() {
        let hue = layer_hue(layer.id, num_layers);

        let base_mat = materials.add(StandardMaterial {
            base_color: theme.edge_base(hue),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        let highlight_mat = materials.add(StandardMaterial {
            base_color: theme.edge_highlight(hue),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        edge_mats.base_by_layer.insert(layer.id, base_mat.clone());
        edge_mats.highlight_by_layer.insert(layer.id, highlight_mat.clone());

        let positions = build_edge_positions(layer, &layout);
        commands.spawn((
            Mesh3d(meshes.add(line_list_mesh(positions))),
            MeshMaterial3d(base_mat),
            Transform::IDENTITY,
            EdgeBaseMesh { layer_id: layer.id },
        ));
        commands.spawn((
            Mesh3d(meshes.add(line_list_mesh(vec![]))),
            MeshMaterial3d(highlight_mat),
            Transform::IDENTITY,
            EdgeHighlightMesh { layer_id: layer.id },
            Visibility::Hidden,
        ));
    }
}

/// Builds one base + one (empty) highlight `LineList` mesh for all cross-layer
/// nesting links. Replaces the per-frame `draw_nesting_lines` gizmo loop.
pub fn build_nesting_mesh(
    mut commands: Commands,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    theme: Res<Theme>,
    mut edge_mats: ResMut<EdgeMeshMaterials>,
) {
    let base_mat = materials.add(StandardMaterial {
        base_color: theme.nesting_base(),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    let highlight_mat = materials.add(StandardMaterial {
        base_color: theme.nesting_highlight(),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });
    edge_mats.nesting_base = Some(base_mat.clone());
    edge_mats.nesting_highlight = Some(highlight_mat.clone());

    let positions = build_nesting_positions(&sg_res.0, &layout, &vis.layer_visible);
    commands.spawn((
        Mesh3d(meshes.add(line_list_mesh(positions))),
        MeshMaterial3d(base_mat),
        Transform::IDENTITY,
        NestingBaseMesh,
    ));
    commands.spawn((
        Mesh3d(meshes.add(line_list_mesh(vec![]))),
        MeshMaterial3d(highlight_mat),
        Transform::IDENTITY,
        NestingHighlightMesh,
        Visibility::Hidden,
    ));
}

// ── update systems ────────────────────────────────────────────────────────────

/// Toggles visibility of base vs. highlight meshes based on hover/selection/query
/// state and per-layer visibility toggles. Runs only when any of these change.
pub fn update_edge_nesting_visibility(
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    vis: Res<VisibilityToggles>,
    mut base_query: Query<
        (&EdgeBaseMesh, &mut Visibility),
        (Without<EdgeHighlightMesh>, Without<NestingBaseMesh>, Without<NestingHighlightMesh>),
    >,
    mut highlight_query: Query<
        (&EdgeHighlightMesh, &mut Visibility),
        (Without<EdgeBaseMesh>, Without<NestingBaseMesh>, Without<NestingHighlightMesh>),
    >,
    mut nesting_base_query: Query<
        &mut Visibility,
        (With<NestingBaseMesh>, Without<EdgeBaseMesh>, Without<EdgeHighlightMesh>, Without<NestingHighlightMesh>),
    >,
    mut nesting_highlight_query: Query<
        &mut Visibility,
        (With<NestingHighlightMesh>, Without<EdgeBaseMesh>, Without<EdgeHighlightMesh>, Without<NestingBaseMesh>),
    >,
) {
    if !hover.is_changed()
        && !selection.is_changed()
        && !qps.is_changed()
        && !vis.is_changed()
    {
        return;
    }

    let state_active = hover.hovered_node.is_some()
        || selection.selected_node.is_some()
        || !qps.highlighted_nodes.is_empty();

    for (tag, mut visibility) in &mut base_query {
        let layer_vis = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        *visibility = if layer_vis && !state_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for (tag, mut visibility) in &mut highlight_query {
        let layer_vis = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        *visibility = if layer_vis && state_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut visibility in &mut nesting_base_query {
        *visibility = if vis.show_nesting && !state_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut visibility in &mut nesting_highlight_query {
        *visibility = if vis.show_nesting && state_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Rebuilds edge highlight overlay meshes when hover/selection/query state changes.
pub fn rebuild_edge_highlight(
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut highlight_query: Query<(&EdgeHighlightMesh, &Mesh3d)>,
) {
    if !hover.is_changed() && !selection.is_changed() && !qps.is_changed() {
        return;
    }

    let active = active_node_set(&hover, &selection, &qps);

    for (tag, mesh3d) in &mut highlight_query {
        let Some(mesh) = meshes.get_mut(&mesh3d.0) else { continue };

        let positions = match active {
            None => vec![],
            Some(set) => {
                let Ok(layer) = sg_res.0.layer(tag.layer_id) else { continue };
                let offset = Vec3::Y * 0.02;
                let mut pts: Vec<[f32; 3]> = Vec::new();
                for node in layer.all_nodes() {
                    for edge in &node.edges {
                        if set.contains(&edge.src) && set.contains(&edge.dst) {
                            if let (Some(s), Some(d)) =
                                (layout.anchor(edge.src), layout.anchor(edge.dst))
                            {
                                pts.push(v3(s + offset));
                                pts.push(v3(d + offset));
                            }
                        }
                    }
                }
                pts
            }
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}

/// Rebuilds nesting highlight overlay mesh when hover/selection/query state changes.
pub fn rebuild_nesting_highlight(
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut highlight_query: Query<(&NestingHighlightMesh, &Mesh3d)>,
) {
    if !hover.is_changed() && !selection.is_changed() && !qps.is_changed() {
        return;
    }

    let active = active_node_set(&hover, &selection, &qps);

    for (_, mesh3d) in &mut highlight_query {
        let Some(mesh) = meshes.get_mut(&mesh3d.0) else { continue };

        let positions = match active {
            None => vec![],
            Some(set) => {
                let mut pts: Vec<[f32; 3]> = Vec::new();
                for layer in sg_res.0.layers() {
                    for node in layer.all_nodes() {
                        let Some(pid) = node.pid else { continue };
                        if set.contains(&node.id) && set.contains(&pid) {
                            if let (Some(c), Some(p)) =
                                (layout.anchor(node.id), layout.anchor(pid))
                            {
                                pts.push(v3(c));
                                pts.push(v3(p));
                            }
                        }
                    }
                }
                pts
            }
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}

/// Rebuilds base edge and nesting mesh vertex positions after the z-separation
/// slider changes the layout. Runs only when `LayoutPositions` is mutated.
pub fn rebuild_meshes_on_layout_change(
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
    sg_res: Res<SceneGraphRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    edge_query: Query<(&EdgeBaseMesh, &Mesh3d)>,
    nesting_query: Query<(&NestingBaseMesh, &Mesh3d)>,
) {
    if !layout.is_changed() && !vis.is_changed() {
        return;
    }
    let sg = &sg_res.0;

    if layout.is_changed() {
        for (tag, mesh3d) in &edge_query {
            let Some(mesh) = meshes.get_mut(&mesh3d.0) else { continue };
            let Ok(layer) = sg.layer(tag.layer_id) else { continue };
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, build_edge_positions(layer, &layout));
        }
    }

    let nesting_pos = build_nesting_positions(sg, &layout, &vis.layer_visible);
    for (_, mesh3d) in &nesting_query {
        let Some(mesh) = meshes.get_mut(&mesh3d.0) else { continue };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, nesting_pos.clone());
    }
}
