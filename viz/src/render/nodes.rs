use bevy::prelude::*;

use crate::components::{LayerTag, NodeSphere};
use crate::resources::{
    HoverState, LayoutPositions, QueryPanelState, SelectionState, VisibilityToggles,
};

/// Toggles each node sphere's `Visibility` based on the combined rule:
///   visible = layer_is_visible AND (no_state_active OR node_in_active_set)
///
/// Replaces both the former `apply_layer_visibility` and `update_node_appearance`
/// systems. Runs only when any of the governing resources change.
pub fn update_node_visibility(
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    vis: Res<VisibilityToggles>,
    mut query: Query<(&NodeSphere, &LayerTag, &mut Visibility)>,
) {
    if !hover.is_changed()
        && !selection.is_changed()
        && !qps.is_changed()
        && !vis.is_changed()
    {
        return;
    }

    let active_set = if !qps.highlighted_nodes.is_empty() {
        Some(&qps.highlighted_nodes)
    } else if hover.hovered_node.is_some() {
        Some(&hover.subtree_ids)
    } else if selection.selected_node.is_some() {
        Some(&selection.subtree_ids)
    } else {
        None
    };

    for (sphere, tag, mut visibility) in &mut query {
        let layer_vis = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        let in_active = active_set.map_or(true, |s| s.contains(&sphere.node_id));
        *visibility = if layer_vis && in_active {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Draws a bright yellow circle ring around the selected node using Gizmos.
///
/// The ring sits on the node's anchor, which for a multi-coordinate node is the
/// medoid of its coordinates — so the ring always lands on one of the node's spheres
/// rather than floating in the gap between them.
pub fn draw_selection_ring(
    mut gizmos: Gizmos,
    selection: Res<SelectionState>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
) {
    let Some(nid) = selection.selected_node else {
        return;
    };
    let Some(pos) = layout.anchor(nid) else {
        return;
    };

    let radius = (vis.z_separation * 0.08).max(0.2);
    gizmos.circle(
        Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        radius,
        Color::srgb(1.0, 0.9, 0.0),
    );
}
