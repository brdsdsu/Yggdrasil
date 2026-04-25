use bevy::prelude::*;

use crate::resources::{HoverState, LayoutPositions, SceneGraphRes};

/// Number of dash segments drawn per nesting line.
/// Every other segment is skipped to produce a dashed appearance.
const NESTING_DASH_SEGMENTS: u32 = 10;

/// Draws a line between two visual positions in a dimmed variant of the layer color.
///
/// When a node is hovered, only edges whose source and destination are both inside
/// `HoverState.subtree_ids` are drawn; all others are skipped so non-subtree
/// relationships fade away without needing per-edge material state.
pub fn draw_intra_layer_edges(
    mut gizmos: Gizmos,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    hover: Res<HoverState>,
) {
    let sg = &sg_res.0;
    let num_layers = sg.layers_count();
    let hovering = hover.hovered_node.is_some();

    for layer in sg.layers() {
        // Dimmed, semi-transparent version of the layer color for edge lines.
        let hue = (layer.id as f32 / num_layers.max(1) as f32) * 360.0;
        let edge_color = Color::hsla(hue, 0.75, 0.35, if hovering { 0.15 } else { 0.85 });

        for node in layer.all_nodes() {
            for edge in &node.edges {
                // During hover, skip edges not connecting two subtree nodes.
                if hovering
                    && (!hover.subtree_ids.contains(&edge.src)
                        || !hover.subtree_ids.contains(&edge.dst))
                {
                    continue;
                }

                let Some(&src_pos) = layout.0.get(&edge.src) else {
                    continue;
                };
                let Some(&dst_pos) = layout.0.get(&edge.dst) else {
                    continue;
                };

                // Slightly raise the line above the layer plane to avoid z-fighting
                // with sphere surfaces.
                let offset = Vec3::Y * 0.02;
                gizmos.line(src_pos + offset, dst_pos + offset, edge_color);
            }
        }
    }
}

/// Draws dashed lines from each node to its parent node across layers.
///
/// Nesting lines are drawn in a neutral semi-transparent white so they are
/// visually distinct from intra-layer edges. During hover, only connections
/// that link two nodes both inside the hover subtree are drawn.
pub fn draw_nesting_lines(
    mut gizmos: Gizmos,
    sg_res: Res<SceneGraphRes>,
    layout: Res<LayoutPositions>,
    hover: Res<HoverState>,
) {
    let sg = &sg_res.0;
    let hovering = hover.hovered_node.is_some();
    let nesting_color = Color::srgba(1.0, 1.0, 1.0, if hovering { 0.08 } else { 0.45 });
    let nesting_color_active = Color::srgba(1.0, 1.0, 1.0, 0.9);

    for layer in sg.layers() {
        for node in layer.all_nodes() {
            let Some(pid) = node.pid else { continue };

            let Some(&child_pos) = layout.0.get(&node.id) else {
                continue;
            };
            let Some(&parent_pos) = layout.0.get(&pid) else {
                continue;
            };

            let in_subtree = hover.subtree_ids.contains(&node.id)
                && hover.subtree_ids.contains(&pid);

            let color = if !hovering {
                nesting_color
            } else if in_subtree {
                nesting_color_active
            } else {
                // Skip non-subtree nesting lines entirely during hover.
                continue;
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
