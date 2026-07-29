use std::collections::HashMap;

use bevy::math::Vec3;
use rand::{Rng, RngExt};
use scene_graph::{Bounds, NodeId, SceneGraph};

/// Computes the default layer separation (applied to the Y axis).
///
/// The separation must exceed the full y-range of the scene data so that no node
/// in layer N can visually overlap with a node in layer N+1. We use 1.5× the
/// y-range plus a minimum padding of 1.0, which gives a 50% visual gap between
/// the top of one layer and the bottom of the next.
///
/// Falls back to `1.0` when the scene contains no coordinate nodes.
pub fn compute_z_separation(sg: &SceneGraph) -> f32 {
    let (min, max) = global_bounds(sg);
    let y_range = max.y - min.y;
    y_range * 1.5 + 1.0
}

/// Assigns a 3D visual position to every node in the scene graph.
///
/// Layer hierarchy is encoded on the Y axis so that higher-layer nodes appear
/// above lower-layer nodes when viewed with a standard Y-up camera:
///
/// ```text
/// vis_y = node_coordinate_y  +  layer.id * layer_separation
/// ```
///
/// X and Z are taken directly from the node's scene coordinates.
///
/// ## Node placement rules
///
/// 1. **Has coordinate** — use the node's XZ directly; shift Y by the layer offset.
/// 2. **No coordinate, has children** — place at the mean XZ of already-positioned
///    children (which are in lower-numbered layers, processed first).
/// 3. **No coordinate, no children** — place randomly within the global XZ footprint,
///    retrying up to 10 times to avoid overlapping with nearby nodes.
///
/// Layers are processed in ascending ID order so that children are always positioned
/// before their parents, making rule 2 reliable.
pub fn compute_layout(sg: &SceneGraph, z_separation: f32) -> HashMap<NodeId, Vec3> {
    let (global_min, global_max) = global_bounds(sg);
    // Minimum XZ distance between randomly placed nodes to reduce visual overlap.
    let min_spacing = {
        let dx = global_max.x - global_min.x;
        let dz = global_max.z - global_min.z;
        (dx.hypot(dz) * 0.05).max(0.5)
    };

    let mut positions: HashMap<NodeId, Vec3> = HashMap::new();
    let mut rng = rand::rng();

    // Sort by layer ID to guarantee child-before-parent processing order.
    let mut layers: Vec<_> = sg.layers().collect();
    layers.sort_by_key(|l| l.id);

    for layer in layers {
        let layer_y = layer.id as f32 * z_separation;

        for node in layer.all_nodes() {
            let vis_pos = if let Some(coord) = node.coordinates {
                // Rule 1: coordinate node — XZ from scene, Y offset by layer.
                Vec3::new(coord.x, coord.y + layer_y, coord.z)
            } else {
                let child_positions: Vec<Vec3> = node
                    .children()
                    .iter()
                    .filter_map(|cid| positions.get(cid))
                    .copied()
                    .collect();

                if !child_positions.is_empty() {
                    // Rule 2: no coordinate, but children are already positioned.
                    let n = child_positions.len() as f32;
                    let mean_x = child_positions.iter().map(|p| p.x).sum::<f32>() / n;
                    let mean_z = child_positions.iter().map(|p| p.z).sum::<f32>() / n;
                    Vec3::new(mean_x, layer_y, mean_z)
                } else {
                    // Rule 3: no coordinate, no children — random with overlap avoidance.
                    place_randomly(
                        &mut rng,
                        &positions,
                        global_min,
                        global_max,
                        layer_y,
                        min_spacing,
                    )
                }
            };

            positions.insert(node.id, vis_pos);
        }
    }

    positions
}

/// Computes the global axis-aligned bounding box over all coordinate nodes.
///
/// Returns `(Vec3::ZERO, Vec3::ONE)` as a unit fallback when no coordinate
/// nodes exist, ensuring downstream calculations always have a valid range.
fn global_bounds(sg: &SceneGraph) -> (Vec3, Vec3) {
    let coordinates = sg
        .layers()
        .flat_map(|l| l.all_nodes())
        .filter_map(|n| n.coordinates)
        .collect::<Vec<_>>();
    let bounds = Bounds::of(coordinates);
    match bounds {
        Bounds::Unbounded => (Vec3::ZERO, Vec3::ONE),
        Bounds::Bounded(bounding_box) => {
            let min = bounding_box.min();
            let max = bounding_box.max();
            // TODO: This wierd conversion is only due to bevy's glam and scene-graph's
            // glam version mismatch.
            let min = Vec3::new(min.x, min.y, min.z);
            let max = Vec3::new(max.x, max.y, max.z);
            (min, max)
        }
    }
}

/// Picks a random XZ position within `[global_min, global_max]` at the given `y`.
///
/// Retries up to 10 times to find a spot that is at least `min_spacing` away from
/// all already-placed nodes. Returns the best candidate found regardless of overlap
/// after the retry budget is exhausted.
fn place_randomly(
    rng: &mut impl Rng,
    positions: &HashMap<NodeId, Vec3>,
    global_min: Vec3,
    global_max: Vec3,
    y: f32,
    min_spacing: f32,
) -> Vec3 {
    let mut candidate = random_xz(rng, global_min, global_max, y);
    for _ in 0..10 {
        let too_close = positions
            .values()
            .any(|p| (p.x - candidate.x).hypot(p.z - candidate.z) < min_spacing);
        if !too_close {
            break;
        }
        candidate = random_xz(rng, global_min, global_max, y);
    }
    candidate
}

/// Samples a uniformly random point within the XZ footprint of `[global_min, global_max]`
/// at the specified `y` value.
fn random_xz(rng: &mut impl Rng, global_min: Vec3, global_max: Vec3, y: f32) -> Vec3 {
    Vec3::new(
        rng.random_range(global_min.x..=global_max.x),
        y,
        rng.random_range(global_min.z..=global_max.z),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scene_graph::{Feature, SceneGraph};

    /// Builds a simple 2-layer scene graph:
    /// - Layer 0: two coordinate nodes at (1,2,3) and (3,4,5)
    /// - Layer 1: one coordinate node at (2,3,4), nested under one of the layer-0 nodes
    fn two_layer_graph() -> SceneGraph {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        let l1 = sg.new_layer().id;
        sg.layer_mut(l1).unwrap().add_child(l0).unwrap();

        let n0 = sg.new_coordinates(1.0, 2.0, 3.0, vec![]);
        let n0id = sg.layer_mut(l0).unwrap().push_node(n0).unwrap();

        let n1 = sg.new_coordinates(3.0, 4.0, 5.0, vec![]);
        let n1id = sg.layer_mut(l0).unwrap().push_node(n1).unwrap();

        let n2 = sg.new_coordinates(2.0, 3.0, 4.0, vec![]);
        let n2id = sg.layer_mut(l1).unwrap().push_node(n2).unwrap();

        sg.nest(n0id).under(n2id).unwrap();
        sg.nest(n1id).under(n2id).unwrap();

        sg
    }

    #[test]
    fn test_coord_node_y_includes_layer_offset() {
        let sg = two_layer_graph();
        let y_sep = 10.0;
        let positions = compute_layout(&sg, y_sep);

        // Layer 0 node at (1,2,3): vis_y = 2 + 0*10 = 2, vis_x = 1, vis_z = 3
        let layer0_nodes: Vec<_> = sg.layer(0).unwrap().all_nodes().collect();
        let n0 = &layer0_nodes[0];
        let pos = positions[&n0.id];
        assert!((pos.y - (n0.coordinates.unwrap().y + 0.0 * y_sep)).abs() < 1e-5);
        assert!((pos.x - n0.coordinates.unwrap().x).abs() < 1e-5);
        assert!((pos.z - n0.coordinates.unwrap().z).abs() < 1e-5);

        // Layer 1 node at (2,3,4): vis_y = 3 + 1*10 = 13
        let layer1_nodes: Vec<_> = sg.layer(1).unwrap().all_nodes().collect();
        let n2 = &layer1_nodes[0];
        let pos2 = positions[&n2.id];
        assert!((pos2.y - (n2.coordinates.unwrap().y + 1.0 * y_sep)).abs() < 1e-5);
    }

    #[test]
    fn test_no_coord_with_children_uses_mean_xz() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        let l1 = sg.new_layer().id;
        sg.layer_mut(l1).unwrap().add_child(l0).unwrap();

        // Two children at (0,0,0) and (4,0,4)
        let c0 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
        let c0id = sg.layer_mut(l0).unwrap().push_node(c0).unwrap();

        let c1 = sg.new_coordinates(4.0, 0.0, 4.0, vec![]);
        let c1id = sg.layer_mut(l0).unwrap().push_node(c1).unwrap();

        // Parent node has no coordinates
        let parent = sg.new_node(vec![Feature::new("type", "group")]);
        let pid = sg.layer_mut(l1).unwrap().push_node(parent).unwrap();

        sg.nest(c0id).under(pid).unwrap();
        sg.nest(c1id).under(pid).unwrap();

        let positions = compute_layout(&sg, 10.0);

        let parent_pos = positions[&pid];
        // Expected mean XZ = (0+4)/2=2, (0+4)/2=2; y = layer 1 offset = 1*10 = 10
        assert!((parent_pos.x - 2.0).abs() < 1e-5);
        assert!((parent_pos.z - 2.0).abs() < 1e-5);
        assert!((parent_pos.y - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_no_coord_no_children_within_bounds() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        // Coordinate nodes to establish a bounding box
        let c = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
        sg.layer_mut(l0).unwrap().push_node(c).unwrap();
        let c2 = sg.new_coordinates(10.0, 0.0, 10.0, vec![]);
        sg.layer_mut(l0).unwrap().push_node(c2).unwrap();

        // Isolated node with no coordinates and no children
        let isolated = sg.new_node(vec![Feature::new("type", "ghost")]);
        let iid = sg.layer_mut(l0).unwrap().push_node(isolated).unwrap();

        let positions = compute_layout(&sg, 1.0);
        let pos = positions[&iid];

        // Must be within the global XZ footprint; y = layer 0 offset = 0
        assert!(pos.x >= 0.0 && pos.x <= 10.0);
        assert!(pos.z >= 0.0 && pos.z <= 10.0);
        assert!((pos.y - 0.0).abs() < 1e-5);
    }
}
