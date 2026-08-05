use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use scene_graph::{Bounds, Coordinate, Feature, NodeId, SceneGraph};

/// The source from which the scene graph is loaded.
pub enum SceneInput {
    /// Deserialize a `SceneGraph` from a JSON file produced by `serde_json::to_string`.
    File(PathBuf),
    /// Use the built-in 4-layer demo scene graph (ported from `scene-graph/examples/demo.rs`).
    Demo,
    /// Use the built-in demo scene with the same layer topology, but with each object's
    /// point cloud collapsed into a single multi-coordinate node.
    DemoMulti,
}

/// How an object's point cloud is represented in the point cloud layer.
#[derive(Clone, Copy, PartialEq)]
enum PointCloudMode {
    /// One single-coordinate node per point.
    NodePerPoint,
    /// One node per object, holding every one of that object's points as coordinates.
    NodePerObject,
}

/// Number of point cloud points owned by each object.
const POINTS_PER_OBJECT: usize = 10;
/// Side length of the cube an object's points are drawn from.
const RANGE: f32 = 10.0;
/// Jitter radius around an object's center in [`PointCloudMode::NodePerObject`].
///
/// Sized against the default node radius, which the app derives from the layer
/// separation as roughly `0.075 * scene_y_range` — about 1.5 units for this scene.
/// A tighter cluster than this renders as a single blob of overlapping spheres with
/// the spokes buried inside it.
const CLUSTER_RADIUS: f32 = 2.5;

/// Loads a `SceneGraph` from the given source.
///
/// Returns an error if the file cannot be opened or the JSON cannot be parsed.
pub fn load(input: SceneInput) -> Result<SceneGraph, Box<dyn std::error::Error>> {
    match input {
        SceneInput::File(path) => {
            let file = File::open(&path)?;
            let sg: SceneGraph = serde_json::from_reader(BufReader::new(file))?;
            Ok(sg)
        }
        SceneInput::Demo => Ok(create_demo_graph(PointCloudMode::NodePerPoint)?),
        SceneInput::DemoMulti => Ok(create_demo_graph(PointCloudMode::NodePerObject)?),
    }
}

/// Builds the canonical 4-layer demo scene graph used for development and testing.
///
/// Layer topology:
/// ```text
/// root_layer (layer 3)
/// ├── dynamic_objects_layer (layer 2)  — Human, Dog, Cat; edges: holding, looking
/// │         └── point_cloud_layer (layer 0)  — 30 points in [10,20]³
/// └── static_objects_layer (layer 1)   — car, tree, building, bicycle, bench, sign, lamp
///           └── point_cloud_layer (layer 0)  — 70 points in [0,10]³
/// ```
///
/// Point cloud nodes carry coordinates; semantic and root nodes do not.
///
/// `mode` only changes how those 100 points are distributed across nodes in the point
/// cloud layer — the layer DAG, object nodes, nesting and semantic edges are identical
/// either way. [`PointCloudMode::NodePerPoint`] yields 111 nodes (1 root + 7 static +
/// 3 dynamic + 100 points); [`PointCloudMode::NodePerObject`] yields 21 nodes holding
/// the same 100 coordinates, 10 per node.
fn create_demo_graph(mode: PointCloudMode) -> scene_graph::Result<SceneGraph> {
    let mut sg = SceneGraph::default();

    let pc_layer = sg.new_layer().id;
    let static_layer = sg.new_layer().id;
    let dynamic_layer = sg.new_layer().id;
    let root_layer = sg.new_layer().id;

    // Wire up the layer DAG.
    sg.layer_mut(root_layer)?.add_child(dynamic_layer)?;
    sg.layer_mut(root_layer)?.add_child(static_layer)?;
    sg.layer_mut(dynamic_layer)?.add_child(pc_layer)?;
    sg.layer_mut(static_layer)?.add_child(pc_layer)?;

    // Root node — no coordinates, sits at the top of the hierarchy.
    let root = sg.new_node(vec![Feature::new("name", "root")]);
    let root_id = sg.layer_mut(root_layer)?.push_node(root)?;

    // `point_cloud_id` stays globally sequential over 0..100 across all objects, so
    // feature queries against the demo behave the same in both modes.
    let mut next_pc_id = 0;

    // 7 static object nodes with points in [0,10]³, each owning 10 points.
    let static_types = ["car", "tree", "building", "bicycle", "bench", "sign", "lamp"];
    for &obj_type in &static_types {
        let node = sg.new_node(vec![Feature::new("type", obj_type)]);
        let obj_id = sg.layer_mut(static_layer)?.push_node(node)?;
        let coords = object_points(0.0, mode);
        attach_point_cloud(&mut sg, pc_layer, obj_id, &coords, &mut next_pc_id, mode)?;
        sg.nest(obj_id).under(root_id)?;
    }

    // 3 dynamic object nodes with points in [10,20]³, each owning 10 points.
    let dynamic_types = ["Human", "Dog", "Cat"];
    let mut dyn_ids = Vec::with_capacity(dynamic_types.len());
    for &obj_type in &dynamic_types {
        let node = sg.new_bounded_node(vec![Feature::new("type", obj_type)]);
        let obj_id = sg.layer_mut(dynamic_layer)?.push_node(node)?;
        dyn_ids.push(obj_id);
        let coords = object_points(RANGE, mode);
        attach_point_cloud(&mut sg, pc_layer, obj_id, &coords, &mut next_pc_id, mode)?;
        sg.nest(obj_id).under(root_id)?;
    }

    // Semantic edges between dynamic nodes.
    sg.layer_mut(dynamic_layer)?.add_edge(dyn_ids[0], dyn_ids[1], "holding")?;
    sg.layer_mut(dynamic_layer)?.add_edge(dyn_ids[1], dyn_ids[2], "looking")?;

    Ok(sg)
}

/// Samples one object's [`POINTS_PER_OBJECT`] points inside the cube
/// `[origin, origin + RANGE]³`.
///
/// [`PointCloudMode::NodePerPoint`] scatters them uniformly over the whole cube, as the
/// demo has always done. [`PointCloudMode::NodePerObject`] clusters them around a random
/// center instead: those points become the coordinates of a single node, and a uniform
/// scatter would fan that node's spokes across the entire cube, overlapping every other
/// object and making the grouping unreadable.
fn object_points(origin: f32, mode: PointCloudMode) -> Vec<Coordinate> {
    let uniform = || rand::random::<f32>() * RANGE + origin;
    match mode {
        PointCloudMode::NodePerPoint => (0..POINTS_PER_OBJECT)
            .map(|_| Coordinate::new(uniform(), uniform(), uniform()))
            .collect(),
        PointCloudMode::NodePerObject => {
            // Inset the center by the jitter radius so the whole cluster stays in the cube.
            let span = RANGE - 2.0 * CLUSTER_RADIUS;
            let axis_center = || rand::random::<f32>() * span + origin + CLUSTER_RADIUS;
            let center = Coordinate::new(axis_center(), axis_center(), axis_center());

            let jitter = || (rand::random::<f32>() * 2.0 - 1.0) * CLUSTER_RADIUS;
            (0..POINTS_PER_OBJECT)
                .map(|_| {
                    Coordinate::new(
                        center.x + jitter(),
                        center.y + jitter(),
                        center.z + jitter(),
                    )
                })
                .collect()
        }
    }
}

/// Attaches `coords` to `obj_id` as point cloud children in `pc_layer`.
///
/// [`PointCloudMode::NodePerPoint`] creates one single-coordinate node per point.
/// [`PointCloudMode::NodePerObject`] creates a single node holding all of them — built
/// from `new_node` and then filled in directly, since `new_coordinates` can only ever
/// produce a one-coordinate node. `push_node` inserts every coordinate into the layer's
/// position index, so spatial queries see all of them.
///
/// `next_pc_id` is advanced by the number of points consumed either way, keeping
/// `point_cloud_id` numbering aligned between the two modes.
fn attach_point_cloud(
    sg: &mut SceneGraph,
    pc_layer: u32,
    obj_id: NodeId,
    coords: &[Coordinate],
    next_pc_id: &mut usize,
    mode: PointCloudMode,
) -> scene_graph::Result<()> {
    match mode {
        PointCloudMode::NodePerPoint => {
            for c in coords {
                let node = sg.new_coordinates(
                    c.x,
                    c.y,
                    c.z,
                    vec![Feature::new("point_cloud_id", *next_pc_id)],
                );
                *next_pc_id += 1;
                let pc_id = sg.layer_mut(pc_layer)?.push_node(node)?;
                sg.nest(pc_id).under(obj_id)?;
            }
        }
        PointCloudMode::NodePerObject => {
            let mut node = sg.new_node(vec![
                Feature::new("point_cloud_id", *next_pc_id),
                Feature::new("point_count", coords.len()),
            ]);
            *next_pc_id += coords.len();
            node.coordinates = coords.to_vec();
            node.bounds = Some(Bounds::of(coords));
            let pc_id = sg.layer_mut(pc_layer)?.push_node(node)?;
            sg.nest(pc_id).under(obj_id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The historical demo: 1 root + 7 static + 3 dynamic + 100 single-coordinate points.
    #[test]
    fn node_per_point_demo_has_111_nodes_each_with_one_coordinate() {
        let sg = create_demo_graph(PointCloudMode::NodePerPoint).unwrap();

        let nodes: Vec<_> = sg.layers().flat_map(|l| l.all_nodes()).collect();
        assert_eq!(nodes.len(), 111);
        assert_eq!(sg.layer(0).unwrap().all_nodes().count(), 100);
        assert!(nodes.iter().all(|n| n.coordinates.len() <= 1));
    }

    /// The multi-coordinate variant: same topology above layer 0, but the 100 points are
    /// held by 10 nodes instead of 100.
    #[test]
    fn node_per_object_demo_holds_the_same_points_in_ten_nodes() {
        let sg = create_demo_graph(PointCloudMode::NodePerObject).unwrap();

        let nodes: Vec<_> = sg.layers().flat_map(|l| l.all_nodes()).collect();
        assert_eq!(nodes.len(), 21);

        let pc_nodes: Vec<_> = sg.layer(0).unwrap().all_nodes().collect();
        assert_eq!(pc_nodes.len(), 10);
        assert!(pc_nodes
            .iter()
            .all(|n| n.coordinates.len() == POINTS_PER_OBJECT));

        let total_coords: usize = nodes.iter().map(|n| n.coordinates.len()).sum();
        assert_eq!(total_coords, 100);

        // Layer counts above the point cloud layer are unchanged.
        assert_eq!(sg.layer(1).unwrap().all_nodes().count(), 7);
        assert_eq!(sg.layer(2).unwrap().all_nodes().count(), 3);
        assert_eq!(sg.layer(3).unwrap().all_nodes().count(), 1);
    }

    /// Every multi-coordinate node carries bounds covering its own points, so the AABB
    /// overlay can show each cluster's extent.
    #[test]
    fn node_per_object_points_are_clustered_and_bounded() {
        let sg = create_demo_graph(PointCloudMode::NodePerObject).unwrap();

        for node in sg.layer(0).unwrap().all_nodes() {
            let Some(Bounds::Bounded(bb)) = node.bounds else {
                panic!("multi-coordinate node {:?} has no bounding box", node.id);
            };
            let extent = bb.max() - bb.min();
            // A cluster spans at most twice the jitter radius on every axis, which is
            // well under the 10-unit cube a uniform scatter would fill.
            assert!(
                extent.max_element() <= 2.0 * CLUSTER_RADIUS + 1e-4,
                "cluster extent {extent:?} exceeds the jitter radius",
            );
            for c in &node.coordinates {
                assert!(c.x >= bb.min().x && c.x <= bb.max().x);
                assert!(c.y >= bb.min().y && c.y <= bb.max().y);
                assert!(c.z >= bb.min().z && c.z <= bb.max().z);
            }
        }
    }
}
