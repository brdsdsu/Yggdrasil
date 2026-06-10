use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};

use kiddo::float::kdtree::KdTree;
use serde::{Deserialize, Serialize};

use crate::IntoNode;

use super::error::{Result, SceneGraphError};
use super::pos::{PosQueryCount, PositionQuery};
use super::{Edge, Node, NodeId, Observer};

/// A Layer in the Scene Graph containing multiple Nodes and their Edges, as weel as the
/// relationships with other layers in the Scene Graph.
/// Each Layer is a well-defined Graph structure representing a specific aspect of the scene,
/// such as semantic relationships or physical connections between objects.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Layer {
    /// Unique identifier for the layer.
    pub id: u32,
    /// List of nodes in this layer.
    pub(crate) nodes: Vec<Node>,
    /// List of child layer IDs.
    pub(crate) children: Vec<u32>,
    /// Optional spatial index for node positions.
    #[serde(skip)]
    pos_index: Option<KdTree<f32, NodeId, 3, 128, u32>>,
}

/// Node Access and Modification
impl Layer {
    /// Get a reference to a node by its ID.
    pub fn node(&self, id: NodeId) -> Result<&Node> {
        // Nodes are inserted in order, so binary search is safe here
        let ptr = self
            .nodes
            .binary_search_by_key(&id, |node| node.id)
            .map_err(|_| SceneGraphError::NodeNotFound)?;
        Ok(&self.nodes[ptr])
    }

    /// Get a mutable reference to a node by its ID.
    pub fn node_mut(&mut self, id: NodeId) -> Result<NodeMut<'_>> {
        // Nodes are inserted in order, so binary search is safe here
        let ptr = self
            .nodes
            .binary_search_by_key(&id, |node| node.id)
            .map_err(|_| SceneGraphError::NodeNotFound)?;
        Ok(NodeMut {
            prev_coords: self.nodes[ptr].coordinates,
            layer: self,
            node_ptr: ptr,
        })
    }

    /// Add a new node to the layer.
    /// Returns the ID of the newly added node.
    pub fn push_node<N: IntoNode>(&mut self, node: N) -> NodeId {
        let node_id = NodeId::new(node.nid(), self.id);
        let node = node.into_node(node_id);
        if let Some(pos_index) = &mut self.pos_index
            && let Some(coords) = node.coordinates
        {
            pos_index.add(&coords.into(), node.id);
        }
        let ptr = match self.nodes.binary_search_by_key(&node.id, |n| n.id) {
            Ok(ptr) => ptr,
            Err(ptr) => ptr,
        };
        let nid = node.id;
        self.nodes.insert(ptr, node);
        nid
    }

    /// Add an edge from source node to destination node with a description.
    /// Ensures both source and destination nodes exist in the layer.
    pub fn add_edge(&mut self, src: NodeId, dst: NodeId, desc: &str) -> Result<()> {
        // Ensure destination node exists
        let _ = self.node(dst)?;

        let mut src_node = self.node_mut(src)?;
        src_node.edges.push(Edge::new(src, dst, desc));
        Ok(())
    }

    /// Delete an edge from source node to destination node.
    /// Returns an error if the edge does not exist.
    pub fn del_edge(&mut self, src: NodeId, dst: NodeId) -> Result<()> {
        let mut src_node = self.node_mut(src)?;
        let index = src_node
            .edges
            .iter()
            .position(|edge| edge.dst == dst)
            .ok_or(SceneGraphError::EdgeNotFound)?;
        src_node.edges.swap_remove(index);
        Ok(())
    }

    /// Add a child layer by its ID.
    pub fn add_child(&mut self, child_id: u32) -> Result<()> {
        if child_id >= self.id {
            return Err(SceneGraphError::InvalidLayersForNesting(child_id, self.id));
        }
        match self.children.binary_search(&child_id) {
            Ok(_) => (),
            Err(index) => self.children.insert(index, child_id),
        };
        Ok(())
    }
}

/// Query
impl Layer {
    /// Get List of all nodes in the layer.
    pub fn all_nodes(&self) -> impl ExactSizeIterator<Item = &Node> {
        self.nodes.iter()
    }

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
    pub fn edges_from(&self, src: NodeId) -> Vec<&Edge> {
        match self.node(src) {
            Ok(n) => n.edges.iter().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: NodeId) -> Vec<&Edge> {
        self.nodes
            .iter()
            .flat_map(|n| n.edges.iter().filter(|e| e.dst == dst))
            .collect()
    }

    /// Get a new Layer containing only nodes within the observer's field of view.
    /// The check is done using the nodes' coordinates and nodes without coordinates are ignored.
    pub fn observable_nodes(&self, observer: &Observer) -> Self {
        let nodes = self
            .nodes
            .iter()
            .filter(|n| n.coordinates.is_some())
            .filter(|n| observer.observers(&n.coordinates.unwrap()))
            .cloned()
            .collect::<Vec<Node>>();

        let pos_index = match self.pos_index {
            Some(_) => {
                let mut pos_index = KdTree::new();
                for node in &nodes {
                    if let Some(coords) = node.coordinates {
                        pos_index.add(&coords.into(), node.id);
                    }
                }
                Some(pos_index)
            }
            None => None,
        };

        let mut l = Self {
            id: self.id,
            nodes,
            pos_index,
            children: self.children.clone(),
        };

        // prune edges to out-of-view nodes
        l.prune();
        l
    }

    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nearest_nodes(
        &self,
        pos: &super::Coordinate,
        count: PosQueryCount,
    ) -> Vec<(&Node, f32)> {
        match &self.pos_index {
            Some(pos_index) => pos_index.nearest_nodes(pos, count),
            None => self.nodes.nearest_nodes(pos, count),
        }
        .into_iter()
        .filter_map(|(id, dist)| self.node(id).map(|node| (node, dist)).ok())
        .collect()
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(&Node, f32)> {
        match &self.pos_index {
            Some(pos_index) => pos_index.nodes_within(pos, radius, count),
            None => self.nodes.nodes_within(pos, radius, count),
        }
        .into_iter()
        .filter_map(|(id, dist)| self.node(id).map(|node| (node, dist)).ok())
        .collect()
    }
}

impl Layer {
    /// Merge another layer into this one.
    /// Nodes with the same ID will be merged, while new nodes will be added.
    /// Deleting Nodes and edges is not supported in this operation.
    pub fn merge(&mut self, mergee: Layer) -> std::result::Result<(), SceneGraphError> {
        let node_ids: HashSet<_> = mergee.nodes.iter().map(|n| n.id).collect();
        for node in mergee.nodes {
            let existing_node = self.node_mut(node.id);
            match existing_node {
                Ok(mut existing_node) => {
                    existing_node.features = node.features;
                    existing_node.coordinates = node.coordinates;
                    // Merging edges:
                    // Since mergee layer defines the connectivity between two nodes that are both
                    // present in the mergee layer, for each edge such as A->B in the existing_node
                    // node, the edge is retained only if B does not exist in the mergee layer.
                    // Then all edges from the mergee node are added to the existing_node.
                    existing_node.edges.retain(|e| !node_ids.contains(&e.dst));
                    existing_node.edges.extend(node.edges);
                }
                Err(SceneGraphError::NodeNotFound) => {
                    drop(existing_node);
                    self.push_node(node.clone());
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl Layer {
    /// Create a new empty Layer without position indexing.
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: Layer::nearest_nodes
    /// [`nodes_within`]: Layer::nodes_within
    pub(super) fn new(id: u32) -> Self {
        Self {
            id,
            nodes: Vec::new(),
            children: Vec::new(),
            pos_index: None,
        }
    }

    /// Create a new empty Layer with position indexing enabled.
    /// This allows for efficient spatial queries on node positions.
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: Layer::nearest_nodes
    /// [`nodes_within`]: Layer::nodes_within
    pub(super) fn with_pos_index(id: u32) -> Self {
        Self {
            id,
            nodes: Vec::new(),
            children: Vec::new(),
            pos_index: Some(KdTree::new()),
        }
    }

    /// Delete a node by its ID, removing all associated edges in the layer.
    pub(super) fn del_node(&mut self, id: NodeId) -> Result<Node> {
        let index = self
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or(SceneGraphError::NodeNotFound)?;
        let node = self.nodes.remove(index);
        if let Some(pos_index) = &mut self.pos_index
            && let Some(coords) = node.coordinates
        {
            pos_index.remove(&coords.into(), node.id);
        }
        self.nodes
            .iter_mut()
            .for_each(|node| node.edges.retain(|edge| edge.dst != id));
        Ok(node)
    }

    /// Retain only the nodes specified in the retain_nodes list.
    /// All other nodes and their associated edges will be removed from the layer.
    pub(crate) fn retain_nodes(&mut self, retain_nodes: &[NodeId]) {
        self.nodes.retain(|node| retain_nodes.contains(&node.id));
        self.prune();
    }

    /// Prune edges that point to non-existing nodes in the layer.
    pub(crate) fn prune(&mut self) {
        let node_ids: Vec<_> = self.nodes.iter().map(|n| n.id).collect();
        self.nodes
            .iter_mut()
            .for_each(|n| n.edges.retain(|e| node_ids.contains(&e.dst)));
    }

    /// Create a subset Layer containing only the specified nodes.
    pub(crate) fn subset(&self, nodes: &[NodeId]) -> Self {
        let nodes: Vec<Node> = self
            .nodes
            .iter()
            .filter(|n| nodes.contains(&n.id))
            .cloned()
            .collect();
        let pos_index = match self.pos_index {
            Some(_) => {
                let mut pos_index = KdTree::new();
                for node in &nodes {
                    if let Some(coords) = node.coordinates {
                        pos_index.add(&coords.into(), node.id);
                    }
                }
                Some(pos_index)
            }
            None => None,
        };
        let mut l = Self {
            id: self.id,
            nodes,
            pos_index,
            children: self.children.clone(),
        };
        l.prune();
        l
    }
}

impl Layer {
    /// Get an unsynced mutable reference to the Node, preventing syncronization between the
    /// `Layer` and the `Node`.
    ///
    /// # Safety
    /// This function is unsafe because it allows modification of the Node without updating
    /// the Layer's position index. The caller must ensure that any changes to the Node's
    /// coordinates are properly reflected in the Layer's position index to maintain consistency.
    /// Get a mutable reference to a node by its ID.
    pub(crate) unsafe fn node_mut_unsynced(&mut self, id: NodeId) -> Result<&mut Node> {
        // Nodes are inserted in order, so binary search is safe here
        let ptr = self
            .nodes
            .binary_search_by_key(&id, |node| node.id)
            .map_err(|_| SceneGraphError::NodeNotFound)?;
        Ok(&mut self.nodes[ptr])
    }

    /// Get NodeId of a given node in the layer.
    pub fn node_id(&self, id: u32) -> NodeId {
        NodeId::new(id, self.id)
    }
}

/// A mutable reference to a Node within a Layer.
/// Tracks previous coordinates to update the position index on drop.
pub struct NodeMut<'a> {
    prev_coords: Option<super::Coordinate>,
    layer: &'a mut Layer,
    node_ptr: usize,
}

impl Deref for NodeMut<'_> {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.layer.nodes[self.node_ptr]
    }
}

impl DerefMut for NodeMut<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.layer.nodes[self.node_ptr]
    }
}

impl Drop for NodeMut<'_> {
    fn drop(&mut self) {
        let node = &**self;
        let new_coords = node.coordinates;
        let node_id = node.id;

        if self.prev_coords == new_coords {
            return;
        }
        if let Some(pos_index) = &mut self.layer.pos_index {
            if let Some(pos) = self.prev_coords {
                pos_index.remove(&pos.into(), node_id);
            }
            if let Some(pos) = new_coords {
                pos_index.add(&pos.into(), node_id);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Coordinate;
    use crate::node::AnonNode;

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
        let mut layer = Layer::default();
        for (i, p) in pts.iter().enumerate() {
            layer.push_node(AnonNode::new(i as u32, Vec::new(), Some(*p), false, None));
        }
        // Node with no coordinates
        layer.push_node(AnonNode::new(
            pts.len() as u32,
            Vec::new(),
            None,
            false,
            None,
        ));

        // fully connecting nodes to each other
        for src in 0..layer.nodes.len() {
            for dst in 0..layer.nodes.len() {
                let src = layer.node_id(src as u32);
                let dst = layer.node_id(dst as u32);
                layer.add_edge(src, dst, "connect").unwrap();
            }
        }

        let cone = cone();
        let observed_layer = layer.observable_nodes(&cone);
        assert_eq!(observed_layer.nodes.len(), 3);
        for node in observed_layer.nodes {
            // all nodes should have coordinates to be observable
            assert!(node.coordinates.is_some());
            // all nodes should be within the cone frustum
            assert!(cone.observers(&node.coordinates.unwrap()));
            // nodes 0,1,2 are inside, rest are outside, so only edges to 0,1,2 should remain
            assert_eq!(
                node.edges.iter().map(|e| e.dst).collect::<Vec<_>>(),
                (0..=2).map(|i| layer.node_id(i)).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn nearest_query() {
        let mut l = Layer::with_pos_index(0);
        assert!(l.pos_index.is_some());

        let pts = [
            Coordinate::new(0.0, 0.0, 0.0),
            Coordinate::new(1.0, 1.0, 1.0),
            Coordinate::new(2.0, 2.0, 2.0),
            Coordinate::new(3.0, 3.0, 3.0),
            Coordinate::new(4.0, 4.0, 4.0),
        ];
        for (i, p) in pts.iter().enumerate() {
            l.push_node(AnonNode::new(i as u32, Vec::new(), Some(*p), false, None));
        }

        let query_pos = Coordinate::new(0.6, 0.6, 0.6);

        let nearest_nodes = l.nearest_nodes(&query_pos, PosQueryCount::Count(3));
        assert_eq!(nearest_nodes.len(), 3);
        // first 3 nearest nodes should be [1.0,1.0,1.0], [0.0,0.0,0.0], [2.0,2.0,2.0]
        assert_eq!(nearest_nodes[0].0.coordinates.unwrap(), pts[1]);
        assert_eq!(nearest_nodes[1].0.coordinates.unwrap(), pts[0]);
        assert_eq!(nearest_nodes[2].0.coordinates.unwrap(), pts[2]);
    }

    #[test]
    fn radius_query() {
        let mut l = Layer::with_pos_index(0);
        assert!(l.pos_index.is_some());

        let pts = [
            Coordinate::new(0.0, 0.0, 0.0),
            Coordinate::new(1.0, 1.0, 1.0),
            Coordinate::new(2.0, 2.0, 2.0),
            Coordinate::new(3.0, 3.0, 3.0),
            Coordinate::new(4.0, 4.0, 4.0),
        ];
        for (i, p) in pts.iter().enumerate() {
            l.push_node(AnonNode::new(i as u32, Vec::new(), Some(*p), false, None));
        }

        let query_pos = Coordinate::new(2.0, 2.0, 2.0);

        // only matching [1.0,1.0,1.0], [2.0,2.0,2.0], [3.0,3.0,3.0]
        let nodes_within = l.nodes_within(&query_pos, 2.0, PosQueryCount::All);
        assert_eq!(nodes_within.len(), 3);

        // only matching [1.0,1.0,1.0], [2.0,2.0,2.0], [3.0,3.0,3.0], but limit to 2 results
        let nodes_within_limited = l.nodes_within(&query_pos, 4.0, PosQueryCount::Count(2));
        assert_eq!(nodes_within_limited.len(), 2);

        // radius large enough to include all nodes
        let nodes_within_none = l.nodes_within(&query_pos, 4.0, PosQueryCount::All);
        assert_eq!(nodes_within_none.len(), 5);
    }
}
