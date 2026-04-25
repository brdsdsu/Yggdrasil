use bevy::prelude::*;
use scene_graph::Bounds;

use crate::resources::{HoverState, SceneGraphRes, VisibilityToggles};

/// Draws axis-aligned bounding box wireframes for nodes that have `Bounds::Bounded`.
///
/// Wireframes are only drawn when `VisibilityToggles.show_aabbs` is enabled.
/// When a node is hovered, only nodes inside the hover subtree have their AABBs drawn.
/// When nothing is hovered, all bounded nodes are drawn.
///
/// For each bounded node on layer `L`, one wireframe is drawn per layer in `[0, L]`:
/// - At layer 0 the box uses the raw data z-extent (where the leaf coordinates live).
/// - At each higher layer the box is a flat slab centered on that layer's visual z-center,
///   preserving the same XY footprint so the spatial extent is visible at every tier.
/// Each slab is colored with that sub-layer's hue so layers remain visually distinct.
pub fn draw_aabb_wireframes(
    mut gizmos: Gizmos,
    sg_res: Res<SceneGraphRes>,
    vis: Res<VisibilityToggles>,
    hover: Res<HoverState>,
) {
    if !vis.show_aabbs {
        return;
    }

    let sg = &sg_res.0;
    let num_layers = sg.layers_count();
    let hovering = hover.hovered_node.is_some();

    for layer in sg.layers() {
        // Skip AABBs for nodes belonging to a hidden layer.
        if !vis.layer_visible.get(&layer.id).copied().unwrap_or(true) {
            continue;
        }

        for node in layer.all_nodes() {
            // During hover, only draw AABBs for nodes inside the active subtree.
            if hovering && !hover.subtree_ids.contains(&node.id) {
                continue;
            }

            let Some(Bounds::Bounded(bb)) = node.bounds else {
                continue;
            };

            // Convert scene-graph Vec3 to Bevy Vec3 explicitly to guard against
            // cross-crate glam version differences.
            let bb_min = Vec3::new(bb.min().x, bb.min().y, bb.min().z);
            let bb_max = Vec3::new(bb.max().x, bb.max().y, bb.max().z);

            let xy_center = Vec2::new((bb_min.x + bb_max.x) * 0.5, (bb_min.y + bb_max.y) * 0.5);
            let xy_size = Vec2::new(
                (bb_max.x - bb_min.x).max(0.1),
                (bb_max.y - bb_min.y).max(0.1),
            );

            // Draw one box per layer from 0 up to this node's own layer,
            // skipping any sub-layer that is currently hidden.
            for sub_id in 0..=layer.id {
                if !vis.layer_visible.get(&sub_id).copied().unwrap_or(true) {
                    continue;
                }

                let hue = (sub_id as f32 / num_layers.max(1) as f32) * 360.0;
                let color = Color::hsla(hue, 0.9, 0.7, 0.8);

                let (center_z, size_z) = if sub_id == 0 {
                    // Layer 0: use the actual z-extent from the raw bounding box.
                    (
                        (bb_min.z + bb_max.z) * 0.5,
                        (bb_max.z - bb_min.z).max(0.1),
                    )
                } else {
                    // Higher layers: flat slab centered on that layer's visual z.
                    (sub_id as f32 * vis.z_separation, 0.5)
                };

                gizmos.cuboid(
                    Transform::from_translation(Vec3::new(xy_center.x, xy_center.y, center_z))
                        .with_scale(Vec3::new(xy_size.x, xy_size.y, size_z)),
                    color,
                );
            }
        }
    }
}
