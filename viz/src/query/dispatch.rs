use std::collections::VecDeque;

use scene_graph::{Coordinate, Feature, NodeId, Observer, PosQueryCount};

use bevy::prelude::*;

use crate::resources::{QueryPanelState, QueryScope, QueryType, SceneGraphRes, SelectionState};

/// Executes the pending query whenever `QueryPanelState.submitted` is true.
///
/// Reads all form fields from `QueryPanelState`, maps them to the appropriate
/// `SceneGraph` API call, and writes result node/edge IDs back into
/// `highlighted_nodes` / `highlighted_edges`. Any parse or runtime error is
/// written to `QueryPanelState.error` so the UI can display it.
pub fn execute_query(
    mut qps: ResMut<QueryPanelState>,
    selection: Res<SelectionState>,
    sg_res: Res<SceneGraphRes>,
) {
    if !qps.submitted {
        return;
    }
    qps.submitted = false;

    let sg = &sg_res.0;
    let result = run_query(&qps, &selection, sg);

    match result {
        Ok((mut nodes, edges)) => {
            // Expand every direct result node to include its full descendant subgraph
            // so that the highlight covers the complete subtree, not just the top node.
            let roots: Vec<NodeId> = nodes.iter().copied().collect();
            for root in roots {
                let mut queue = VecDeque::from([root]);
                while let Some(nid) = queue.pop_front() {
                    if let Ok(node) = sg.node(nid) {
                        for &child_id in node.children() {
                            if nodes.insert(child_id) {
                                queue.push_back(child_id);
                            }
                        }
                    }
                }
            }
            qps.highlighted_nodes = nodes;
            qps.highlighted_edges = edges;
            qps.error = None;
        }
        Err(msg) => {
            qps.error = Some(msg);
        }
    }
}

type QueryResult = Result<
    (
        std::collections::HashSet<NodeId>,
        std::collections::HashSet<(NodeId, NodeId)>,
    ),
    String,
>;

fn run_query(
    qps: &QueryPanelState,
    selection: &SelectionState,
    sg: &scene_graph::SceneGraph,
) -> QueryResult {
    let mut nodes = std::collections::HashSet::new();
    let mut edges = std::collections::HashSet::new();

    match qps.query_type {
        QueryType::NodesByFeature => {
            let key = qps.feature_key.trim();
            let val = qps.feature_value.trim();
            if key.is_empty() {
                return Err("Key must not be empty".into());
            }
            let feat = Feature::new(key, val);
            let results: Vec<_> = match qps.scope {
                QueryScope::Layer => {
                    let lid = selection
                        .selected_layer
                        .ok_or("No layer selected for Layer scope")?;
                    sg.layer(lid)
                        .map_err(|e| e.to_string())?
                        .nodes_matching(&[&feat])
                        .into_iter()
                        .map(|n| n.id)
                        .collect()
                }
                _ => sg
                    .nodes_matching(&[&feat])
                    .into_iter()
                    .map(|n| n.id)
                    .collect(),
            };
            nodes.extend(results);
        }

        QueryType::NodesByKey => {
            let key = qps.feature_key.trim();
            if key.is_empty() {
                return Err("Key must not be empty".into());
            }
            let results: Vec<_> = match qps.scope {
                QueryScope::Layer => {
                    let lid = selection
                        .selected_layer
                        .ok_or("No layer selected for Layer scope")?;
                    sg.layer(lid)
                        .map_err(|e| e.to_string())?
                        .nodes_having(&[key])
                        .into_iter()
                        .map(|n| n.id)
                        .collect()
                }
                _ => sg
                    .nodes_having(&[key])
                    .into_iter()
                    .map(|n| n.id)
                    .collect(),
            };
            nodes.extend(results);
        }

        QueryType::EdgesByLabel => {
            let label = qps.edge_label.trim();
            if label.is_empty() {
                return Err("Label must not be empty".into());
            }
            let result_edges: Vec<_> = match qps.scope {
                QueryScope::Layer => {
                    let lid = selection
                        .selected_layer
                        .ok_or("No layer selected for Layer scope")?;
                    sg.layer(lid)
                        .map_err(|e| e.to_string())?
                        .edges_matching(label)
                        .into_iter()
                        .map(|e| (e.src, e.dst))
                        .collect()
                }
                _ => sg
                    .edges_matching(label)
                    .into_iter()
                    .flatten()
                    .map(|e| (e.src, e.dst))
                    .collect(),
            };
            edges.extend(result_edges);
            // Also highlight the endpoint nodes for visibility.
            for (src, dst) in &edges {
                nodes.insert(*src);
                nodes.insert(*dst);
            }
        }

        QueryType::EdgesFrom => {
            let nid = parse_node_id(&qps.root_node)?;
            for e in sg.edges_from(nid) {
                edges.insert((e.src, e.dst));
                nodes.insert(e.src);
                nodes.insert(e.dst);
            }
        }

        QueryType::EdgesTo => {
            let nid = parse_node_id(&qps.root_node)?;
            for e in sg.edges_to(nid) {
                edges.insert((e.src, e.dst));
                nodes.insert(e.src);
                nodes.insert(e.dst);
            }
        }

        QueryType::NearestNode => {
            let pos = parse_coord(&qps.coord)?;
            let count = if qps.use_all {
                PosQueryCount::All
            } else {
                let n: usize = qps
                    .count
                    .trim()
                    .parse()
                    .map_err(|_| "Count must be a positive integer".to_string())?;
                PosQueryCount::Count(n)
            };
            match qps.scope {
                QueryScope::Graph => {
                    for (node, _dist) in sg.nearest_nodes(&pos, count) {
                        nodes.insert(node.id);
                    }
                }
                QueryScope::Layer => todo!(),
                QueryScope::Node => {
                    let root = selection
                        .selected_node
                        .ok_or("No node selected for Node scope")?;
                    for (node, _dist) in sg
                        .nearest_in_subtree(&pos, root, count)
                        .map_err(|e| e.to_string())?
                    {
                        nodes.insert(node.id);
                    }
                }
            }
        }

        QueryType::NodesWithin => {
            let pos = parse_coord(&qps.coord)?;
            let radius: f32 = qps
                .radius
                .trim()
                .parse()
                .map_err(|_| "Radius must be a number".to_string())?;
            let count = if qps.use_all {
                PosQueryCount::All
            } else {
                let n: usize = qps
                    .count
                    .trim()
                    .parse()
                    .map_err(|_| "Count must be a positive integer".to_string())?;
                PosQueryCount::Count(n)
            };
            for (node, _dist) in sg.nodes_within(&pos, radius, count) {
                nodes.insert(node.id);
            }
        }

        QueryType::VisibleSubgraph => {
            let pos = parse_coord_named(&qps.fov_pos, "position")?;
            let yaw = parse_degrees(&qps.fov_yaw, "yaw")?;
            let pitch = parse_degrees(&qps.fov_pitch, "pitch")?;
            let roll = parse_degrees(&qps.fov_roll, "roll")?;
            let half_angle = parse_degrees(&qps.fov_half_angle, "half-angle")?;
            let near: f32 = qps
                .fov_near
                .trim()
                .parse()
                .map_err(|_| "Near must be a number".to_string())?;
            let far: f32 = qps
                .fov_far
                .trim()
                .parse()
                .map_err(|_| "Far must be a number".to_string())?;
            let root = parse_node_id(&qps.root_node)?;

            let observer = Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far);
            let subgraph = sg
                .visible_subgraph(&observer, root)
                .map_err(|e| e.to_string())?;

            for layer in subgraph.layers() {
                for node in layer.all_nodes() {
                    nodes.insert(node.id);
                }
            }
        }
    }

    Ok((nodes, edges))
}

/// Parses `"layer:node"` into a [`NodeId`].
fn parse_node_id(s: &str) -> Result<NodeId, String> {
    let s = s.trim();
    let (layer_str, node_str) = s
        .split_once(':')
        .ok_or_else(|| format!("Expected 'layer:node', got '{s}'"))?;
    let layer: u32 = layer_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid layer id '{layer_str}'"))?;
    let node: u32 = node_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid node id '{node_str}'"))?;
    Ok(NodeId::new(node, layer))
}

/// Parses three string fields into a [`Coordinate`].
fn parse_coord(fields: &[String; 3]) -> Result<Coordinate, String> {
    parse_coord_named(fields, "coordinate")
}

fn parse_coord_named(fields: &[String; 3], label: &str) -> Result<Coordinate, String> {
    let x: f32 = fields[0]
        .trim()
        .parse()
        .map_err(|_| format!("{label} x must be a number"))?;
    let y: f32 = fields[1]
        .trim()
        .parse()
        .map_err(|_| format!("{label} y must be a number"))?;
    let z: f32 = fields[2]
        .trim()
        .parse()
        .map_err(|_| format!("{label} z must be a number"))?;
    Ok(Coordinate::new(x, y, z))
}

/// Parses a degree string and converts to radians.
fn parse_degrees(s: &str, label: &str) -> Result<f32, String> {
    let deg: f32 = s
        .trim()
        .parse()
        .map_err(|_| format!("{label} must be a number (degrees)"))?;
    Ok(deg.to_radians())
}
