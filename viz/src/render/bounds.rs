use std::collections::VecDeque;

use bevy::prelude::*;
use scene_graph::Bounds;

use crate::resources::{HoverState, SceneGraphRes, VisibilityToggles};

/// Collects the given layer ID plus all layer IDs reachable by following
/// `child_layer_ids()` downward in the layer DAG (BFS).
fn descendant_layer_ids(sg: &scene_graph::SceneGraph, root_layer_id: u32) -> Vec<u32> {
    let mut result = Vec::new();
    let mut queue = VecDeque::from([root_layer_id]);
    while let Some(lid) = queue.pop_front() {
        result.push(lid);
        if let Ok(layer) = sg.layer(lid) {
            for &child_id in layer.child_layer_ids() {
                queue.push_back(child_id);
            }
        }
    }
    result
}

/// Draws axis-aligned bounding box wireframes for nodes that have `Bounds::Bounded`.
///
/// Wireframes are only drawn when `VisibilityToggles.show_aabbs` is enabled.
/// When a node is hovered, only nodes inside the hover subtree have their AABBs drawn.
/// When nothing is hovered, all bounded nodes are drawn.
///
/// For each bounded node on layer `L`, one wireframe slab is drawn per layer that is
/// `L` itself or a descendant of `L` in the layer DAG (not simply all layers 0..=L).
/// This correctly handles non-linear layer topologies where some lower-ID layers are
/// not actually reachable from `L`.
///
/// Each slab is colored with that sub-layer's hue so layers remain visually distinct.
/// The XZ plane is the horizontal footprint; Y is the layer separation axis.
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
        if !vis.layer_visible.get(&layer.id).copied().unwrap_or(true) {
            continue;
        }

        // Precompute descendant layers for this layer (including itself).
        let descendant_ids = descendant_layer_ids(sg, layer.id);

        for node in layer.all_nodes() {
            if hovering && !hover.subtree_ids.contains(&node.id) {
                continue;
            }

            let Some(Bounds::Bounded(bb)) = node.bounds else {
                continue;
            };

            let bb_min = Vec3::new(bb.min().x, bb.min().y, bb.min().z);
            let bb_max = Vec3::new(bb.max().x, bb.max().y, bb.max().z);

            // Horizontal footprint is XZ; layer separation is on the Y axis.
            let xz_center = Vec2::new((bb_min.x + bb_max.x) * 0.5, (bb_min.z + bb_max.z) * 0.5);
            let xz_size = Vec2::new(
                (bb_max.x - bb_min.x).max(0.1),
                (bb_max.z - bb_min.z).max(0.1),
            );

            // The leaf layer (lowest ID among descendants) holds the actual
            // coordinate data, so its slab uses the raw y-extent from the
            // bounding box. All other layers get a flat slab at their visual y.
            let leaf_layer_id = descendant_ids.iter().copied().min().unwrap_or(layer.id);

            // Draw one slab per descendant layer (skip hidden layers).
            for &sub_id in &descendant_ids {
                if !vis.layer_visible.get(&sub_id).copied().unwrap_or(true) {
                    continue;
                }

                let hue = (sub_id as f32 / num_layers.max(1) as f32) * 360.0;
                let color = Color::hsla(hue, 0.9, 0.7, 0.8);

                let (center_y, size_y) = if sub_id == leaf_layer_id {
                    ((bb_min.y + bb_max.y) * 0.5, (bb_max.y - bb_min.y).max(0.1))
                } else {
                    (sub_id as f32 * vis.z_separation, 0.5)
                };

                gizmos.cuboid(
                    Transform::from_translation(Vec3::new(xz_center.x, center_y, xz_center.y))
                        .with_scale(Vec3::new(xz_size.x, size_y, xz_size.y)),
                    color,
                );
            }
        }
    }
}
