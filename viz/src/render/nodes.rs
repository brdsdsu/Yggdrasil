use bevy::prelude::*;

use crate::components::{LayerTag, NodeSphere, Opacity};
use crate::resources::{
    HoverState, LayoutPositions, MaterialHandles, QueryPanelState, SelectionState, VisibilityToggles,
};

/// Shows or hides each node sphere based on `VisibilityToggles.layer_visible`.
///
/// Runs every frame so that toggling a layer checkbox in the UI takes effect
/// immediately without requiring a scene rebuild.
pub fn apply_layer_visibility(
    vis: Res<VisibilityToggles>,
    mut query: Query<(&LayerTag, &mut Visibility)>,
) {
    for (tag, mut visibility) in &mut query {
        let visible = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Unified per-frame material update.
///
/// Priority order for opacity:
/// 1. Query results active → result nodes full, others faded.
/// 2. Hover active → subtree nodes full, others faded.
/// 3. Default → all full opacity.
///
/// Emissive highlight is applied to the selected node (bright yellow) and to
/// query-result nodes (dim yellow) independently of opacity.
pub fn update_node_appearance(
    time: Res<Time>,
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    qps: Res<QueryPanelState>,
    mat_handles: Res<MaterialHandles>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(&NodeSphere, &mut Opacity)>,
) {
    let dt = time.delta_secs();
    let hovering = hover.hovered_node.is_some();
    let has_results = !qps.highlighted_nodes.is_empty();

    for (sphere, mut opacity) in &mut query {
        let nid = sphere.node_id;
        let in_results = qps.highlighted_nodes.contains(&nid);
        let in_hover_subtree = hover.subtree_ids.contains(&nid);
        let in_selection_subtree = selection.subtree_ids.contains(&nid);
        let is_selected = selection.selected_node == Some(nid);
        let selected = selection.selected_node.is_some();

        opacity.target = if has_results {
            // Query results override everything.
            if in_results { 1.0 } else { 0.15 }
        } else if hovering {
            // Hover overrides selection fade while the cursor is over a node.
            if in_hover_subtree { 1.0 } else { 0.15 }
        } else if selected {
            // Selection fade: only selected node + descendants stay visible.
            if in_selection_subtree { 1.0 } else { 0.15 }
        } else {
            1.0
        };

        let t = (dt * 8.0).min(1.0);
        opacity.current += (opacity.target - opacity.current) * t;

        let Some(handle) = mat_handles.0.get(&nid) else { continue };
        let Some(mat) = materials.get_mut(handle) else { continue };

        mat.base_color.set_alpha(opacity.current);

        mat.emissive = if is_selected {
            LinearRgba::new(0.6, 0.6, 0.0, 1.0)
        } else if in_results {
            LinearRgba::new(0.3, 0.3, 0.0, 1.0)
        } else {
            LinearRgba::BLACK
        };
    }
}

/// Draws a bright yellow circle ring around the selected node using Gizmos.
///
/// The ring sits in the XY plane at the node's visual position and scales
/// with the layer z-separation so it remains visible at any scene scale.
pub fn draw_selection_ring(
    mut gizmos: Gizmos,
    selection: Res<SelectionState>,
    layout: Res<LayoutPositions>,
    vis: Res<VisibilityToggles>,
) {
    let Some(nid) = selection.selected_node else {
        return;
    };
    let Some(&pos) = layout.0.get(&nid) else {
        return;
    };

    let radius = (vis.z_separation * 0.08).max(0.2);
    gizmos.circle(
        Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        radius,
        Color::srgb(1.0, 0.9, 0.0),
    );
}
