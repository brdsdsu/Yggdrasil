use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use scene_graph::{Feature, SceneGraph};

/// The source from which the scene graph is loaded.
pub enum SceneInput {
    /// Deserialize a `SceneGraph` from a JSON file produced by `serde_json::to_string`.
    File(PathBuf),
    /// Use the built-in 4-layer demo scene graph (ported from `scene-graph/examples/demo.rs`).
    Demo,
}

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
        SceneInput::Demo => Ok(create_demo_graph()?),
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
fn create_demo_graph() -> scene_graph::Result<SceneGraph> {
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

    // 100 point cloud nodes: first 70 in [0,10]³, next 30 in [10,20]³.
    let mut pc_ids = Vec::with_capacity(100);
    for i in 0..70 {
        let x = rand::random::<f32>() * 10.0;
        let y = rand::random::<f32>() * 10.0;
        let z = rand::random::<f32>() * 10.0;
        let node = sg.new_coordinates(x, y, z, vec![Feature::new("point_cloud_id", i)]);
        pc_ids.push(sg.layer_mut(pc_layer)?.push_node(node)?);
    }
    for i in 0..30 {
        let x = rand::random::<f32>() * 10.0 + 10.0;
        let y = rand::random::<f32>() * 10.0 + 10.0;
        let z = rand::random::<f32>() * 10.0 + 10.0;
        let node = sg.new_coordinates(x, y, z, vec![Feature::new("point_cloud_id", 70 + i)]);
        pc_ids.push(sg.layer_mut(pc_layer)?.push_node(node)?);
    }

    // 7 static object nodes, each owning 10 point cloud nodes.
    let static_types = ["car", "tree", "building", "bicycle", "bench", "sign", "lamp"];
    for (i, &obj_type) in static_types.iter().enumerate() {
        let node = sg.new_node(vec![Feature::new("type", obj_type)]);
        let obj_id = sg.layer_mut(static_layer)?.push_node(node)?;
        for &pc_id in &pc_ids[i * 10..(i + 1) * 10] {
            sg.nest(pc_id).under(obj_id)?;
        }
        sg.nest(obj_id).under(root_id)?;
    }

    // 3 dynamic object nodes, each owning 10 point cloud nodes.
    let dynamic_types = ["Human", "Dog", "Cat"];
    let mut dyn_ids = Vec::with_capacity(3);
    let offset = static_types.len() * 10;
    for (i, &obj_type) in dynamic_types.iter().enumerate() {
        let node = sg.new_bounded_node(vec![Feature::new("type", obj_type)]);
        let obj_id = sg.layer_mut(dynamic_layer)?.push_node(node)?;
        dyn_ids.push(obj_id);
        for &pc_id in &pc_ids[offset + i * 10..offset + (i + 1) * 10] {
            sg.nest(pc_id).under(obj_id)?;
        }
        sg.nest(obj_id).under(root_id)?;
    }

    // Semantic edges between dynamic nodes.
    sg.layer_mut(dynamic_layer)?.add_edge(dyn_ids[0], dyn_ids[1], "holding")?;
    sg.layer_mut(dynamic_layer)?.add_edge(dyn_ids[1], dyn_ids[2], "looking")?;

    Ok(sg)
}
