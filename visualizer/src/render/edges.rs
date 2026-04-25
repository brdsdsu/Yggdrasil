use bevy::prelude::*;

use crate::resources::{HoverState, LayoutPositions, SceneGraphRes, SelectionState, VisibilityToggles};

/// Number of dash segments drawn per nesting line.
/// Every other segment is skipped to produce a dashed appearance.
const NESTING_DASH_SEGMENTS: u32 = 10;

/// Draws intra-layer edges as solid lines in a dimmed layer color.
///
/// Visibility priority (highest first):
/// 1. Layer hidden → skip entirely.
/// 2. Hover active → only edges with both endpoints in the hover subtree are drawn.
/// 3. Selection active → only edges with both endpoints in the selection subtree are drawn.
/// 4. Default → all edges drawn.
pub fn draw_intra_layer_edges(
    mut gizmos: Gizmos,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    vis: Res<VisibilityToggles>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count();
    let hovering = hover.hovered_node.is_some();
    let selected = selection.selected_node.is_some();

    for layer in sg.layers() {
        if !vis.layer_visible.get(&layer.id).copied().unwrap_or(true) {
            continue;
        }

        let hue = (layer.id as f32 / num_layers.max(1) as f32) * 360.0;
        let faded = hovering || selected;
        let edge_color = Color::hsla(hue, 0.75, 0.35, if faded { 0.15 } else { 0.85 });

        for node in layer.all_nodes() {
            for edge in &node.edges {
                let active = if hovering {
                    hover.subtree_ids.contains(&edge.src)
                        && hover.subtree_ids.contains(&edge.dst)
                } else if selected {
                    selection.subtree_ids.contains(&edge.src)
                        && selection.subtree_ids.contains(&edge.dst)
                } else {
                    true
                };

                if !active {
                    continue;
                }

                let Some(&src_pos) = layout.0.get(&edge.src) else { continue };
                let Some(&dst_pos) = layout.0.get(&edge.dst) else { continue };

                let offset = Vec3::Y * 0.02;
                gizmos.line(src_pos + offset, dst_pos + offset, edge_color);
            }
        }
    }
}

/// Draws dashed nesting lines from each node to its parent across layers.
///
/// Visibility priority (highest first):
/// 1. Either endpoint's layer hidden → skip.
/// 2. Hover active → only connections inside the hover subtree are drawn.
/// 3. Selection active → only connections inside the selection subtree are drawn.
/// 4. Default → all nesting lines drawn.
pub fn draw_nesting_lines(
    mut gizmos: Gizmos,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    hover: Res<HoverState>,
    selection: Res<SelectionState>,
    vis: Res<VisibilityToggles>,
) {
    let sg = &sg_res.0;
    let hovering = hover.hovered_node.is_some();
    let selected = selection.selected_node.is_some();

    let faded = hovering || selected;
    let color_dim = Color::srgba(1.0, 1.0, 1.0, if faded { 0.08 } else { 0.45 });
    let color_active = Color::srgba(1.0, 1.0, 1.0, 0.9);

    for layer in sg.layers() {
        if !vis.layer_visible.get(&layer.id).copied().unwrap_or(true) {
            continue;
        }

        for node in layer.all_nodes() {
            let Some(pid) = node.pid else { continue };

            let parent_layer_id = pid.layer_id();
            if !vis.layer_visible.get(&parent_layer_id).copied().unwrap_or(true) {
                continue;
            }

            let Some(&child_pos) = layout.0.get(&node.id) else { continue };
            let Some(&parent_pos) = layout.0.get(&pid) else { continue };

            let in_active_set = if hovering {
                hover.subtree_ids.contains(&node.id) && hover.subtree_ids.contains(&pid)
            } else if selected {
                selection.subtree_ids.contains(&node.id)
                    && selection.subtree_ids.contains(&pid)
            } else {
                true
            };

            let color = if faded {
                if in_active_set { color_active } else { continue }
            } else {
                color_dim
            };

            draw_dashed_line(&mut gizmos, child_pos, parent_pos, color);
        }
    }
}

/// Draws a dashed line between `start` and `end` by rendering every other segment.
fn draw_dashed_line(gizmos: &mut Gizmos, start: Vec3, end: Vec3, color: Color) {
    for i in (0..NESTING_DASH_SEGMENTS).step_by(2) {
        let t0 = i as f32 / NESTING_DASH_SEGMENTS as f32;
        let t1 = (i + 1) as f32 / NESTING_DASH_SEGMENTS as f32;
        gizmos.line(start.lerp(end, t0), start.lerp(end, t1), color);
    }
}
