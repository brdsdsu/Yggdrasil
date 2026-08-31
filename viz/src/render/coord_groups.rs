use bevy::prelude::*;

use crate::components::CoordGroupMesh;
use crate::render::edges::{active_node_set, line_list_mesh, v3};
use crate::resources::{
    EdgeMeshMaterials, HoverState, LayoutPositions, QueryPanelState, SceneGraphRes, SelectionState,
    VisibilityToggles,
};

/// Builds the spoke vertex list for one layer.
///
/// Only nodes with at least two coordinates contribute: a single-coordinate node's
/// spoke would be a zero-length segment, since its anchor *is* its only coordinate.
/// `include` decides which nodes are drawn, and is kept in lockstep with the
/// sphere-visibility rule so spokes never outlive the spheres they connect.
pub(crate) fn build_spoke_positions(
    sg: &scene_graph::SceneGraph,
    layer_id: u32,
    layout: &LayoutPositions,
    include: impl Fn(scene_graph::NodeId) -> bool,
) -> Vec<[f32; 3]> {
    let mut positions = Vec::new();
    let Ok(layer) = sg.layer(layer_id) else {
        return positions;
    };

    for node in layer.all_nodes() {
        let coords = layout.coords(node.id);
        if coords.len() < 2 || !include(node.id) {
            continue;
        }
        let Some(anchor) = layout.anchor(node.id) else {
            continue;
        };
        for &coord in coords {
            positions.push(v3(coord));
            positions.push(v3(anchor));
        }
    }
    positions
}

/// The material used for one layer's spokes.
///
/// Deliberately subordinate to the lines that carry graph meaning: intra-layer edges
/// use the layer hue at saturation 0.75, nesting lines use white at alpha 0.18–0.45.
/// Low saturation plus low alpha reads as "same node", not "relationship".
fn spoke_material(hue: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: Color::hsla(hue, 0.35, 0.5, 0.35),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// Spawns one [`CoordGroupMesh`] entity per layer, pre-filled with every spoke in
/// that layer.
pub fn build_coord_group_meshes(
    mut commands: Commands,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut edge_mats: ResMut<EdgeMeshMaterials>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count().max(1) as f32;

    for layer in sg.layers() {
        let hue = (layer.id as f32 / num_layers) * 360.0;
        let mat = materials.add(spoke_material(hue));
        edge_mats.coord_group_by_layer.insert(layer.id, mat.clone());

        let layer_visible = vis.layer_visible.get(&layer.id).copied().unwrap_or(true);
        let positions = build_spoke_positions(sg, layer.id, &layout, |_| true);

        commands.spawn((
            Mesh3d(meshes.add(line_list_mesh(positions))),
            MeshMaterial3d(mat),
            Transform::IDENTITY,
            CoordGroupMesh { layer_id: layer.id },
            if vis.show_coord_groups && layer_visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ));
    }
}

/// Rebuilds each layer's spoke mesh so it always matches the spheres currently drawn.
///
/// A node's spokes are included exactly when its spheres are, using the same
/// `layer_visible && in_active_set` rule as
/// [`update_node_visibility`](crate::render::nodes::update_node_visibility). That is
/// what keeps both `viz/CLAUDE.md` invariants intact for free: hiding a layer removes
/// its spokes along with its spheres.
pub fn rebuild_coord_groups(
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    vis: Res<VisibilityToggles>,
    layout: Res<LayoutPositions>,
    sg_res: Res<SceneGraphRes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&CoordGroupMesh, &Mesh3d, &mut Visibility)>,
) {
    if !hover.is_changed()
        && !selection.is_changed()
        && !qps.is_changed()
        && !vis.is_changed()
        && !layout.is_changed()
    {
        return;
    }

    let active = active_node_set(&hover, &selection, &qps);

    for (tag, mesh3d, mut visibility) in &mut query {
        let layer_visible = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        *visibility = if vis.show_coord_groups && layer_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        let Some(mesh) = meshes.get_mut(&mesh3d.0) else {
            continue;
        };
        let positions = if layer_visible {
            build_spoke_positions(&sg_res.0, tag.layer_id, &layout, |nid| {
                active.is_none_or(|set| set.contains(&nid))
            })
        } else {
            Vec::new()
        };
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{UpAxis, compute_layout};
    use scene_graph::{Bounds, Coordinate, SceneGraph};

    /// A one-layer graph holding a 3-coordinate node, a 1-coordinate node and a
    /// coordinate-less node, returned with the multi-coordinate node's id.
    fn mixed_graph() -> (SceneGraph, scene_graph::NodeId) {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;

        let coords = vec![
            Coordinate::new(0.0, 0.0, 0.0),
            Coordinate::new(4.0, 0.0, 0.0),
            Coordinate::new(0.0, 0.0, 4.0),
        ];
        let mut multi = sg.new_node(vec![]);
        multi.bounds = Some(Bounds::of(&coords));
        multi.coordinates = coords;
        let multi_id = sg.layer_mut(l0).unwrap().push_node(multi).unwrap();

        let single = sg.new_coordinates(9.0, 9.0, 9.0, vec![]);
        sg.layer_mut(l0).unwrap().push_node(single).unwrap();

        let bare = sg.new_node(vec![]);
        sg.layer_mut(l0).unwrap().push_node(bare).unwrap();

        (sg, multi_id)
    }

    #[test]
    fn one_spoke_per_coordinate_of_a_multi_coordinate_node() {
        let (sg, multi_id) = mixed_graph();
        let layout = LayoutPositions(compute_layout(&sg, 1.0, UpAxis::Y));

        let positions = build_spoke_positions(&sg, 0, &layout, |_| true);

        // Only the 3-coordinate node contributes: 3 spokes, 2 vertices each. The
        // 1-coordinate and coordinate-less nodes are skipped.
        assert_eq!(positions.len(), 6);

        let anchor = v3(layout.anchor(multi_id).unwrap());
        // Every odd vertex is the anchor end of a spoke.
        for pair in positions.chunks(2) {
            assert_eq!(pair[1], anchor);
        }
        // Every even vertex is one of the node's own coordinate positions.
        let coords: Vec<[f32; 3]> = layout.coords(multi_id).iter().copied().map(v3).collect();
        for pair in positions.chunks(2) {
            assert!(coords.contains(&pair[0]));
        }
    }

    #[test]
    fn single_coordinate_nodes_produce_no_spokes() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        for i in 0..5 {
            let n = sg.new_coordinates(i as f32, 0.0, 0.0, vec![]);
            sg.layer_mut(l0).unwrap().push_node(n).unwrap();
        }
        let layout = LayoutPositions(compute_layout(&sg, 1.0, UpAxis::Y));

        assert!(build_spoke_positions(&sg, 0, &layout, |_| true).is_empty());
    }

    #[test]
    fn excluded_nodes_contribute_no_spokes() {
        let (sg, multi_id) = mixed_graph();
        let layout = LayoutPositions(compute_layout(&sg, 1.0, UpAxis::Y));

        // Mirrors an active set that does not contain the multi-coordinate node, which
        // is how a hover or query on some other node hides its spokes along with its
        // spheres.
        let positions = build_spoke_positions(&sg, 0, &layout, |nid| nid != multi_id);
        assert!(positions.is_empty());
    }
}
