use std::collections::HashSet;

use super::{Edge, Node, Observer};
use crate::error::{AtlasError, Result};

/// A Layer in the Scene Graph containing multiple Nodes and their Edges.
/// Each Layer is a well-defined Graph structure representing a specific aspect of the scene,
/// such as semantic relationships or physical connections between objects.
#[derive(Debug, Clone)]
pub struct Layer {
    /// List of nodes in this layer.
    pub(super) nodes: Vec<Node>,
}

/// Node Access and Modification
impl Layer {
    /// Get a reference to a node by its ID.
    pub fn node(&self, id: usize) -> Result<&Node> {
        self.nodes
            .iter()
            .find(|node| node.id == id)
            .ok_or(AtlasError::NodeNotFound)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn node_mut(&mut self, id: usize) -> Result<&mut Node> {
        self.nodes
            .iter_mut()
            .find(|node| node.id == id)
            .ok_or(AtlasError::NodeNotFound)
    }

    /// Add a new node to the layer.
    pub fn push_node(&mut self, node: Node) {
        self.nodes.push(node);
    }

    /// Add an edge from source node to destination node with a description.
    /// Ensures both source and destination nodes exist in the layer.
    pub fn add_edge(&mut self, src: usize, dst: usize, desc: &str) -> Result<()> {
        // Ensure destination node exists
        let _ = self.node(dst)?;
        let src_node = self.node_mut(src)?;
        src_node.edges.push(Edge::new(src, dst, desc));
        Ok(())
    }

    /// Delete an edge from source node to destination node.
    /// Returns an error if the edge does not exist.
    pub fn del_edge(&mut self, src: usize, dst: usize) -> Result<()> {
        let src_node = self.node_mut(src)?;
        let index = src_node
            .edges
            .iter()
            .position(|edge| edge.dst == dst)
            .ok_or(AtlasError::EdgeNotFound)?;
        src_node.edges.swap_remove(index);
        Ok(())
    }
}

/// Query
impl Layer {
    /// Get List of all nodes matching a specific node features.
    pub fn nodes_having(&self, keys: &[&str]) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| keys.iter().all(|key| node.has_feature(key)))
            .collect()
    }

    /// Get List of all nodes matching a specific node features.
    pub fn nodes_matching(&self, features: &[&super::node::Feature]) -> Vec<&Node> {
        self.nodes
            .iter()
            .filter(|node| features.iter().all(|f| node.match_feature(f)))
            .collect()
    }

    /// Get List of all edges matching a specific description.
    pub fn edges_matching(&self, desc: &str) -> Vec<&Edge> {
        self.nodes
            .iter()
            .flat_map(|n| n.edges.iter().filter(|e| e.desc == desc))
            .collect()
    }

    /// Get List of all edges from a specific source node.
    pub fn edges_from(&self, src: usize) -> Vec<&Edge> {
        match self.node(src) {
            Ok(n) => n.edges.iter().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: usize) -> Vec<&Edge> {
        self.nodes
            .iter()
            .flat_map(|n| n.edges.iter().filter(|e| e.dst == dst))
            .collect()
    }

    /// Get a new Layer containing only nodes within the observer's field of view.
    /// The check is done using the nodes' coordinates and nodes without coordinates are ignored.
    pub fn observable_nodes(&self, observer: Observer) -> Self {
        let nodes = self
            .nodes
            .iter()
            .filter(|n| n.coordinates.is_some())
            .filter(|n| observer.observers(&n.coordinates.unwrap()))
            .cloned()
            .collect::<Vec<Node>>();
        let mut l = Self { nodes };

        // prune edges to out-of-view nodes
        l.prune();
        l
    }
}

impl Layer {
    /// Merge another layer into this one.
    /// Nodes with the same ID will be merged, while new nodes will be added.
    /// Deleting Nodes and edges is not supported in this operation.
    pub fn merge(&mut self, l2: Layer) -> std::result::Result<(), AtlasError> {
        for node in l2.nodes {
            match self.node_mut(node.id) {
                Ok(existing_node) => {
                    existing_node.merge(node)?;
                }
                Err(AtlasError::NodeNotFound) => {
                    self.push_node(node.clone());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Prune edges that point to non-existing nodes in the layer.
    pub(super) fn prune(&mut self) {
        let node_ids: Vec<usize> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .iter_mut()
            .for_each(|n| n.edges.retain(|e| node_ids.contains(&e.dst)));
    }
}

impl Layer {
    pub(super) fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Delete a node by its ID, removing all associated edges in the layer.
    pub(super) fn del_node(&mut self, id: usize) -> Result<Node> {
        let index = self
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or(AtlasError::NodeNotFound)?;
        let node = self.nodes.remove(index);
        self.nodes
            .iter_mut()
            .for_each(|node| node.edges.retain(|edge| edge.dst != id));
        Ok(node)
    }

    /// Retain only the nodes specified in the retain_nodes list.
    /// All other nodes and their associated edges will be removed from the layer.
    pub(super) fn retain_nodes(&mut self, retain_nodes: &[usize]) {
        self.nodes.retain(|node| retain_nodes.contains(&node.id));
        self.prune();
    }
}


#[cfg(test)]
mod test {
    use super::super::Coordinate;
    use super::*;

    fn cone() -> Observer {
        // Observer at origin, yaw=30°, pitch=5°, roll=0°
        let pos = Coordinate::new(0.0, 0.0, 0.0);
        let yaw = 0_f32.to_radians();
        let pitch = 0_f32.to_radians();
        let roll = 0_f32.to_radians();

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        Observer::from_ypr(pos, yaw, pitch, roll, half_angle, near, far)
    }

    #[test]
    fn fov_query() {
        let pts = [
            Coordinate::new(0.0, 0.0, 1.0), // inside
            Coordinate::new(0.0, 0.0, 1.0), // inside
            Coordinate::new(0.0, 0.0, 1.0), // inside
            Coordinate::new(6.0, 6.0, 6.0), // outside
            Coordinate::new(6.0, 6.0, 6.0), // outside
            Coordinate::new(6.0, 6.0, 6.0), // outside
            Coordinate::new(6.0, 6.0, 6.0), // outside
        ];
        let mut layer = Layer::new();
        for (i, p) in pts.iter().enumerate() {
            layer.push_node(Node::new(i, Vec::new(), Some(*p)));
        }
        // Node with no coordinates
        layer.push_node(Node::new(pts.len(), Vec::new(), None));

        // fully connecting nodes to each other
        for src in 0..layer.nodes.len() {
            for dst in 0..layer.nodes.len() {
                layer.add_edge(src, dst, "connect").unwrap();
            }
        }

        let cone = cone();
        let observed_layer = layer.observable_nodes(cone);
        assert_eq!(observed_layer.nodes.len(), 3);
        for node in observed_layer.nodes {
            // all nodes should have coordinates to be observable
            assert!(node.coordinates.is_some());
            // all nodes should be within the cone frustum
            assert!(cone.observers(&node.coordinates.unwrap()));
            // nodes 0,1,2 are inside, rest are outside, so only edges to 0,1,2 should remain
            assert_eq!(
                node.edges.iter().map(|e| e.dst).collect::<Vec<_>>(),
                [0, 1, 2]
            )
        }
    }
}
