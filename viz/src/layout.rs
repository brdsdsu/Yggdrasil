use std::collections::HashMap;

use bevy::math::Vec3;
use rand::{Rng, RngExt};
use scene_graph::{Bounds, Coordinate, NodeId, SceneGraph};

/// Which axis of the scene data points up.
///
/// The renderer is Y-up and overloads the visual Y axis for layer separation, but a scene
/// graph does not have to arrive that way. Everything from a Spark-DSG pipeline —
/// Kimera, Hydra, the uHumans2 corpora — follows ROS REP-103, where **Z is up** and X/Y
/// are the floor plane. Loading such a scene under a Y-up reading stands the room on its
/// side: the floor plan's Y axis is drawn vertically, and the (small) height range becomes
/// depth.
///
/// The scene graph itself carries no convention, so this cannot be detected reliably — a
/// flat scene is flat on whichever axis it is flat on, and guessing from extents would
/// silently flip a room that happens to be tall and narrow. It is a declaration by the
/// caller instead, defaulting to the convention the data in the wild actually uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    /// Scene Y is up, matching the renderer. Coordinates pass through unchanged.
    Y,
    /// Scene Z is up (ROS REP-103), as Spark-DSG exports it. The default.
    #[default]
    Z,
}

impl UpAxis {
    /// Maps a scene coordinate into the renderer's Y-up frame, before the layer offset.
    ///
    /// The Z-up case is a -90 degree rotation about X, `(x, y, z) -> (x, z, -y)`. Its
    /// determinant is +1, so the scene keeps its handedness; the mirror `(x, z, y)` would
    /// render every floor plan reversed.
    pub fn to_visual(&self, c: Coordinate) -> Vec3 {
        match self {
            Self::Y => Vec3::new(c.x, c.y, c.z),
            Self::Z => Vec3::new(c.x, c.z, -c.y),
        }
    }

    /// Maps an axis-aligned box into the renderer's frame, as `(min, max)`.
    ///
    /// An axis permutation maps an AABB onto an AABB exactly, but it can exchange which
    /// corner is minimal — Z-up negates Y — so the mapped corners are re-sorted rather
    /// than assumed to stay in order.
    pub fn to_visual_aabb(&self, min: Coordinate, max: Coordinate) -> (Vec3, Vec3) {
        let (a, b) = (self.to_visual(min), self.to_visual(max));
        (a.min(b), a.max(b))
    }
}

/// The visual positions assigned to a single node.
///
/// A node holds a list of coordinates rather than a single one, so it renders as
/// one sphere per coordinate. `anchor` designates which single point the graph
/// edges, nesting lines and the selection ring attach to.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeLayout {
    /// One visual position per entry in `Node::coordinates`, in the same order.
    /// Empty for nodes that carry no coordinates at all.
    pub coords: Vec<Vec3>,
    /// The single point that graph edges, nesting lines and the selection ring
    /// attach to. When `coords` is non-empty this is its medoid, so the anchor
    /// always coincides with a rendered sphere.
    pub anchor: Vec3,
}

/// Computes the default layer separation (applied to the Y axis).
///
/// The separation must exceed the full height of the scene data so that no node
/// in layer N can visually overlap with a node in layer N+1. We use 1.5× the
/// height plus a minimum padding of 1.0, which gives a 50% visual gap between
/// the top of one layer and the bottom of the next.
///
/// The height is measured **after** `up` has been applied, so it is the scene's real
/// vertical extent. Read off the wrong axis it is a floor-plan dimension instead — for the
/// office corpus, 43 m rather than 3 m, which spreads the layers twelve times too far
/// apart and (through `suggest_node_radius`'s fallback) draws every point as a sphere
/// taller than the room.
///
/// Falls back to `1.0` when the scene contains no coordinate nodes.
pub fn compute_z_separation(sg: &SceneGraph, up: UpAxis) -> f32 {
    let (min, max) = global_bounds(sg, up);
    let y_range = max.y - min.y;
    y_range * 1.5 + 1.0
}

/// Assigns visual positions to every node in the scene graph.
///
/// Layer hierarchy is encoded on the Y axis so that higher-layer nodes appear
/// above lower-layer nodes when viewed with a standard Y-up camera:
///
/// ```text
/// vis_y = node_coordinate_y  +  layer.id * layer_separation
/// ```
///
/// Scene coordinates reach the visual frame through `up` first, so X and Z are the
/// floor plane and Y is height regardless of which axis the source data called up.
///
/// ## Node placement rules
///
/// 1. **Has coordinates** — one position per coordinate, mapped through `up` and
///    shifted on Y by the layer offset. The anchor is the medoid of those positions.
/// 2. **No coordinates, has children** — anchor at the mean XZ of already-positioned
///    children's anchors (children are in lower-numbered layers, processed first).
/// 3. **No coordinates, no children** — anchor placed randomly within the global XZ
///    footprint, retrying up to 10 times to avoid overlapping with nearby nodes.
///
/// Layers are processed in ascending ID order so that children are always positioned
/// before their parents, making rule 2 reliable.
pub fn compute_layout(
    sg: &SceneGraph,
    z_separation: f32,
    up: UpAxis,
) -> HashMap<NodeId, NodeLayout> {
    let (global_min, global_max) = global_bounds(sg, up);
    // Minimum XZ distance between randomly placed nodes to reduce visual overlap.
    let min_spacing = {
        let dx = global_max.x - global_min.x;
        let dz = global_max.z - global_min.z;
        (dx.hypot(dz) * 0.05).max(0.5)
    };

    let mut positions: HashMap<NodeId, NodeLayout> = HashMap::new();
    let mut rng = rand::rng();

    // Sort by layer ID to guarantee child-before-parent processing order.
    let mut layers: Vec<_> = sg.layers().collect();
    layers.sort_by_key(|l| l.id);

    for layer in layers {
        let layer_y = layer.id as f32 * z_separation;

        for node in layer.all_nodes() {
            // Rule 1: coordinate node — XZ from the scene's floor plane, Y offset by layer.
            let coords: Vec<Vec3> = node
                .coordinates
                .iter()
                .map(|c| {
                    let v = up.to_visual(*c);
                    Vec3::new(v.x, v.y + layer_y, v.z)
                })
                .collect();

            let anchor = match medoid(&coords) {
                Some(anchor) => anchor,
                None => {
                    let child_anchors: Vec<Vec3> = node
                        .children()
                        .iter()
                        .filter_map(|cid| positions.get(cid))
                        .map(|l| l.anchor)
                        .collect();

                    if !child_anchors.is_empty() {
                        // Rule 2: no coordinates, but children are already positioned.
                        let n = child_anchors.len() as f32;
                        let mean_x = child_anchors.iter().map(|p| p.x).sum::<f32>() / n;
                        let mean_z = child_anchors.iter().map(|p| p.z).sum::<f32>() / n;
                        Vec3::new(mean_x, layer_y, mean_z)
                    } else {
                        // Rule 3: no coordinates, no children — random with overlap avoidance.
                        place_randomly(
                            &mut rng,
                            &positions,
                            global_min,
                            global_max,
                            layer_y,
                            min_spacing,
                        )
                    }
                }
            };

            positions.insert(node.id, NodeLayout { coords, anchor });
        }
    }

    positions
}

/// Picks the initial node sphere radius.
///
/// `fallback` is the layer-separation-derived default, which sizes spheres against the
/// whole scene's height. That cannot resolve the coordinates *inside* a single node: at
/// that size a multi-coordinate node renders as one blob of overlapping spheres with its
/// grouping spokes buried inside. When any node holds several coordinates, shrink the
/// radius to a fraction of the typical spread of a node's coordinates about its anchor.
///
/// Returns `fallback` untouched when every node has at most one coordinate, so
/// single-coordinate scenes keep the sizing they have always had.
pub fn suggest_node_radius(layout: &HashMap<NodeId, NodeLayout>, fallback: f32) -> f32 {
    let mut spreads: Vec<f32> = layout
        .values()
        .filter(|l| l.coords.len() > 1)
        .map(|l| {
            let total: f32 = l.coords.iter().map(|c| c.distance(l.anchor)).sum();
            total / l.coords.len() as f32
        })
        .filter(|spread| *spread > 0.0)
        .collect();

    if spreads.is_empty() {
        return fallback;
    }

    spreads.sort_by(f32::total_cmp);
    let median = spreads[spreads.len() / 2];
    // A fifth of the mean coordinate-to-anchor distance leaves visible space between the
    // spheres of one node, so the spokes joining them are not swallowed. Never grow the
    // radius — only pull it down from `fallback`.
    fallback.min(median * 0.2).max(0.01)
}

/// Returns the element of `coords` closest to their arithmetic mean, or `None`
/// when `coords` is empty.
///
/// The medoid is used rather than the mean itself so that a node's anchor is always
/// one of its own coordinates. The mean of a node whose coordinates form two distant
/// clusters lands in the empty space between them, which would leave every edge
/// endpoint and the selection ring detached from any visible sphere.
fn medoid(coords: &[Vec3]) -> Option<Vec3> {
    let mean = coords.iter().copied().sum::<Vec3>() / coords.len().max(1) as f32;
    coords
        .iter()
        .copied()
        .min_by(|a, b| a.distance_squared(mean).total_cmp(&b.distance_squared(mean)))
}

/// Computes the global axis-aligned bounding box over every coordinate in the scene,
/// **in the visual frame** — so `.y` is height and `.x`/`.z` are the floor plane, whatever
/// `up` the source data uses.
///
/// Returns `(Vec3::ZERO, Vec3::ONE)` as a unit fallback when no coordinate
/// nodes exist, ensuring downstream calculations always have a valid range.
fn global_bounds(sg: &SceneGraph, up: UpAxis) -> (Vec3, Vec3) {
    let coordinates = sg
        .layers()
        .flat_map(|l| l.all_nodes())
        .flat_map(|n| n.coordinates.iter().copied())
        .collect::<Vec<Coordinate>>();
    let bounds = Bounds::of(&coordinates);
    match bounds {
        Bounds::Unbounded => (Vec3::ZERO, Vec3::ONE),
        // `to_visual_aabb` also absorbs the conversion between scene-graph's glam and
        // bevy's, which are different versions of the same type.
        Bounds::Bounded(bounding_box) => up.to_visual_aabb(bounding_box.min(), bounding_box.max()),
    }
}

/// Picks a random XZ position within `[global_min, global_max]` at the given `y`.
///
/// Retries up to 10 times to find a spot that is at least `min_spacing` away from
/// all already-placed anchors. Returns the best candidate found regardless of overlap
/// after the retry budget is exhausted.
fn place_randomly(
    rng: &mut impl Rng,
    positions: &HashMap<NodeId, NodeLayout>,
    global_min: Vec3,
    global_max: Vec3,
    y: f32,
    min_spacing: f32,
) -> Vec3 {
    let mut candidate = random_xz(rng, global_min, global_max, y);
    for _ in 0..10 {
        let too_close = positions
            .values()
            .any(|l| (l.anchor.x - candidate.x).hypot(l.anchor.z - candidate.z) < min_spacing);
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

    /// Every test below passes `UpAxis::Y` explicitly. The default is `Z`, which rotates
    /// coordinates on the way in — these cases assert the placement rules themselves, so
    /// they use the pass-through axis and leave the rotation to its own tests.
    ///
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

    /// Pushes a node holding every coordinate in `coords` into `layer`.
    ///
    /// `new_coordinates` only ever produces a single-coordinate node, so multi-coordinate
    /// nodes have to be assembled from `new_node` directly.
    fn push_multi_coord(sg: &mut SceneGraph, layer: u32, coords: Vec<Coordinate>) -> NodeId {
        let mut node = sg.new_node(vec![]);
        node.bounds = Some(Bounds::of(&coords));
        node.coordinates = coords;
        sg.layer_mut(layer).unwrap().push_node(node).unwrap()
    }

    #[test]
    fn test_coord_node_y_includes_layer_offset() {
        let sg = two_layer_graph();
        let y_sep = 10.0;
        let positions = compute_layout(&sg, y_sep, UpAxis::Y);

        // Layer 0 node at (1,2,3): vis_y = 2 + 0*10 = 2, vis_x = 1, vis_z = 3
        let layer0_nodes: Vec<_> = sg.layer(0).unwrap().all_nodes().collect();
        let n0 = &layer0_nodes[0];
        let c0 = n0.coordinates[0];
        let pos = positions[&n0.id].anchor;
        assert!((pos.y - (c0.y + 0.0 * y_sep)).abs() < 1e-5);
        assert!((pos.x - c0.x).abs() < 1e-5);
        assert!((pos.z - c0.z).abs() < 1e-5);

        // Layer 1 node at (2,3,4): vis_y = 3 + 1*10 = 13
        let layer1_nodes: Vec<_> = sg.layer(1).unwrap().all_nodes().collect();
        let n2 = &layer1_nodes[0];
        let pos2 = positions[&n2.id].anchor;
        assert!((pos2.y - (n2.coordinates[0].y + 1.0 * y_sep)).abs() < 1e-5);
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

        let positions = compute_layout(&sg, 10.0, UpAxis::Y);

        let parent_layout = &positions[&pid];
        // Expected mean XZ = (0+4)/2=2, (0+4)/2=2; y = layer 1 offset = 1*10 = 10
        assert!(parent_layout.coords.is_empty());
        assert!((parent_layout.anchor.x - 2.0).abs() < 1e-5);
        assert!((parent_layout.anchor.z - 2.0).abs() < 1e-5);
        assert!((parent_layout.anchor.y - 10.0).abs() < 1e-5);
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

        let positions = compute_layout(&sg, 1.0, UpAxis::Y);
        let pos = positions[&iid].anchor;

        // Must be within the global XZ footprint; y = layer 0 offset = 0
        assert!(pos.x >= 0.0 && pos.x <= 10.0);
        assert!(pos.z >= 0.0 && pos.z <= 10.0);
        assert!((pos.y - 0.0).abs() < 1e-5);
    }

    #[test]
    fn multi_coord_node_gets_one_position_per_coordinate() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        let l1 = sg.new_layer().id;
        sg.layer_mut(l1).unwrap().add_child(l0).unwrap();

        let coords = vec![
            Coordinate::new(0.0, 1.0, 2.0),
            Coordinate::new(3.0, 4.0, 5.0),
            Coordinate::new(6.0, 7.0, 8.0),
        ];
        // Pushed into layer 1 so the layer offset is non-zero and actually observable.
        let nid = push_multi_coord(&mut sg, l1, coords.clone());

        let y_sep = 10.0;
        let layout = &compute_layout(&sg, y_sep, UpAxis::Y)[&nid];

        assert_eq!(layout.coords.len(), coords.len());
        for (vis, raw) in layout.coords.iter().zip(&coords) {
            assert!((vis.x - raw.x).abs() < 1e-5);
            assert!((vis.y - (raw.y + y_sep)).abs() < 1e-5);
            assert!((vis.z - raw.z).abs() < 1e-5);
        }
    }

    #[test]
    fn multi_coord_anchor_is_one_of_the_nodes_own_coordinates() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;

        // Two tight clusters far apart: the mean lands in the empty gap between them,
        // so an anchor equal to the mean would not coincide with any rendered sphere.
        let coords = vec![
            Coordinate::new(0.0, 0.0, 0.0),
            Coordinate::new(0.5, 0.0, 0.0),
            Coordinate::new(100.0, 0.0, 0.0),
            Coordinate::new(100.5, 0.0, 0.0),
        ];
        let nid = push_multi_coord(&mut sg, l0, coords);

        let layout = &compute_layout(&sg, 1.0, UpAxis::Y)[&nid];
        assert!(
            layout.coords.contains(&layout.anchor),
            "anchor {:?} is not one of the node's own coordinates {:?}",
            layout.anchor,
            layout.coords,
        );
    }

    #[test]
    fn single_coordinate_scenes_keep_the_fallback_node_radius() {
        let sg = two_layer_graph();
        let layout = compute_layout(&sg, 10.0, UpAxis::Y);

        assert_eq!(suggest_node_radius(&layout, 1.5), 1.5);
    }

    #[test]
    fn multi_coordinate_scenes_shrink_the_node_radius_below_the_cluster_spread() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        push_multi_coord(
            &mut sg,
            l0,
            vec![
                Coordinate::new(-1.0, 0.0, 0.0),
                Coordinate::new(1.0, 0.0, 0.0),
                Coordinate::new(0.0, 0.0, -1.0),
                Coordinate::new(0.0, 0.0, 1.0),
            ],
        );
        let layout = compute_layout(&sg, 1.0, UpAxis::Y);

        // The fallback would draw spheres wider than the whole cluster.
        let radius = suggest_node_radius(&layout, 1.5);
        assert!(radius < 1.5, "radius {radius} was not pulled below the fallback");
        assert!(radius > 0.0);
    }

    #[test]
    fn single_coord_node_anchor_equals_its_only_coordinate() {
        let sg = two_layer_graph();
        let positions = compute_layout(&sg, 10.0, UpAxis::Y);

        for layer in sg.layers() {
            for node in layer.all_nodes() {
                let layout = &positions[&node.id];
                assert_eq!(layout.coords.len(), 1);
                assert_eq!(layout.coords[0], layout.anchor);
            }
        }
    }

    #[test]
    fn z_up_rotates_coordinates_into_the_renderers_frame() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        // Two metres up in a Z-up scene, four metres along the floor's y axis.
        let n = sg.new_coordinates(1.0, 4.0, 2.0, vec![]);
        let nid = sg.layer_mut(l0).unwrap().push_node(n).unwrap();

        let pos = compute_layout(&sg, 10.0, UpAxis::Z)[&nid].anchor;

        // Height becomes visual y; the floor axis becomes depth, negated so the scene
        // keeps its handedness.
        assert_eq!(pos, Vec3::new(1.0, 2.0, -4.0));
    }

    #[test]
    fn z_up_layer_separation_follows_the_height_not_the_floor_plan() {
        let mut sg = SceneGraph::default();
        let l0 = sg.new_layer().id;
        // A 100 m x 100 m floor plan, 2 m tall — the shape of a real building export,
        // where reading the separation off a floor axis overshoots by fifty times.
        for c in [
            Coordinate::new(0.0, 0.0, 1.0),
            Coordinate::new(100.0, 100.0, 3.0),
        ] {
            let n = sg.new_coordinates(c.x, c.y, c.z, vec![]);
            sg.layer_mut(l0).unwrap().push_node(n).unwrap();
        }

        assert_eq!(compute_z_separation(&sg, UpAxis::Z), 2.0 * 1.5 + 1.0);
        assert_eq!(compute_z_separation(&sg, UpAxis::Y), 100.0 * 1.5 + 1.0);
    }

    #[test]
    fn z_up_bounds_map_onto_the_same_box_the_coordinates_land_in() {
        let coords = [
            Coordinate::new(-1.0, 2.0, 5.0),
            Coordinate::new(3.0, 8.0, 7.0),
        ];
        let (min, max) = UpAxis::Z.to_visual_aabb(coords[0], coords[1]);

        // Negating y swaps which corner is minimal, so the result must be re-sorted.
        assert_eq!(min, Vec3::new(-1.0, 5.0, -8.0));
        assert_eq!(max, Vec3::new(3.0, 7.0, -2.0));

        // And the mapped box must still contain the mapped corners.
        for c in coords {
            let v = UpAxis::Z.to_visual(c);
            assert!(v.cmpge(min).all() && v.cmple(max).all());
        }
    }

}
