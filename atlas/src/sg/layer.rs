use std::{
    collections::HashSet,
    num::NonZero,
    ops::{Deref, DerefMut},
    result,
};

use kiddo::{SquaredEuclidean, float::kdtree::KdTree};

use super::{Edge, Node, Observer};
use crate::error::{AtlasError, Result};

/// A Layer in the Scene Graph containing multiple Nodes and their Edges.
/// Each Layer is a well-defined Graph structure representing a specific aspect of the scene,
/// such as semantic relationships or physical connections between objects.
#[derive(Debug, Clone)]
pub struct Layer {
    /// List of nodes in this layer.
    pub(super) nodes: Vec<Node>,
    pos_index: Option<KdTree<f32, usize, 3, 32, u32>>,
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
    pub fn node_mut(&mut self, id: usize) -> Result<NodeMut<'_>> {
        self.nodes
            .iter_mut()
            .enumerate()
            .find(|(_, node)| node.id == id)
            .map(|(ptr, n)| (ptr, n.coordinates))
            .map(|(node_ptr, prev_coords)| NodeMut {
                prev_coords,
                layer: self,
                node_ptr,
            })
            .ok_or(AtlasError::NodeNotFound)
    }

    /// Add a new node to the layer.
    pub fn push_node(&mut self, node: Node) {
        if let Some(pos_index) = &mut self.pos_index
            && let Some(coords) = node.coordinates
        {
            pos_index.add(&coords.into(), node.id);
        }
        self.nodes.push(node);
    }

    /// Add an edge from source node to destination node with a description.
    /// Ensures both source and destination nodes exist in the layer.
    pub fn add_edge(&mut self, src: usize, dst: usize, desc: &str) -> Result<()> {
        // Ensure destination node exists
        let _ = self.node(dst)?;
        let mut src_node = self.node_mut(src)?;
        src_node.edges.push(Edge::new(src, dst, desc));
        Ok(())
    }

    /// Delete an edge from source node to destination node.
    /// Returns an error if the edge does not exist.
    pub fn del_edge(&mut self, src: usize, dst: usize) -> Result<()> {
        let mut src_node = self.node_mut(src)?;
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

        let mut l = Self { nodes, pos_index };

        // prune edges to out-of-view nodes
        l.prune();
        l
    }

    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    /// ```rust 
    /// # use atlas::{Layer, Node, PosQueryCount};
    /// # use atlas::Coordinate;
    ///
    /// let mut layer = Layer::with_pos_index();
    /// layer.push_node(Node::new(1, vec![], Some(Coordinate::new(0.0, 0.0, 0.0))));
    /// layer.push_node(Node::new(2, vec![], Some(Coordinate::new(1.0, 1.0, 1.0))));
    /// layer.push_node(Node::new(3, vec![], Some(Coordinate::new(2.0, 2.0, 2.0))));
    ///
    /// let nearest = layer.nearest_nodes(&Coordinate::new(0.6, 0.6, 0.6), PosQueryCount::Count(2));
    ///
    /// assert_eq!(nearest.len(), 2);
    ///
    /// assert_eq!(nearest[0].id, 2);
    /// assert_eq!(nearest[0].coordinates.unwrap(), Coordinate::new(1.0, 1.0, 1.0));
    ///
    /// assert_eq!(nearest[1].id, 1);
    /// assert_eq!(nearest[1].coordinates.unwrap(), Coordinate::new(0.0, 0.0, 0.0));
    /// ```
    pub fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<&Node> {
        let count = count.as_capped_usize(self.nodes.len());
        match &self.pos_index {
            Some(pos_index) => {
                let results = pos_index.nearest_n::<SquaredEuclidean>(&(*pos).into(), count);
                results
                    .into_iter()
                    .filter_map(|n| self.node(n.item).ok())
                    .collect()
            }
            None => {
                let mut distances = self
                    .nodes
                    .iter()
                    .filter(|n| n.coordinates.is_some())
                    .map(|n| (n, n.coordinates.unwrap().distance_squared(*pos)))
                    .collect::<Vec<(&Node, f32)>>();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances.into_iter().take(count).map(|(n, _)| n).collect()
            }
        }
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    /// ```rust
    /// # use atlas::{Layer, Node, PosQueryCount};
    /// # use atlas::Coordinate;
    /// 
    /// let mut layer = Layer::with_pos_index();
    /// layer.push_node(Node::new(1, vec![], Some(Coordinate::new(0.0, 0.0, 0.0))));
    /// layer.push_node(Node::new(2, vec![], Some(Coordinate::new(1.0, 1.0, 1.0))));
    /// layer.push_node(Node::new(3, vec![], Some(Coordinate::new(2.0, 2.0, 2.0))));
    /// 
    /// let nearby = layer.nodes_within(&Coordinate::new(0.0, 0.0, 0.0), 1.8, PosQueryCount::All);
    /// 
    /// assert_eq!(nearby.len(), 2);
    /// ```
    pub fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<&Node> {
        let count = count.as_capped_usize(self.nodes.len());
        match &self.pos_index {
            Some(pos_index) => {
                let results = pos_index.nearest_n_within::<SquaredEuclidean>(
                    &(*pos).into(),
                    radius * radius,
                    unsafe { NonZero::new_unchecked(std::cmp::max(1, count)) },
                    false,
                );
                results
                    .iter()
                    .filter_map(|n| self.node(n.item).ok())
                    .collect()
            }
            None => self
                .nodes
                .iter()
                .filter(|n| n.coordinates.is_some())
                .map(|n| (n, n.coordinates.unwrap().distance_squared(*pos)))
                .filter(|(_, d)| *d <= radius * radius)
                .map(|(n, _)| n)
                .take(count)
                .collect::<Vec<&Node>>(),
        }
    }
}

impl Layer {
    /// Merge another layer into this one.
    /// Nodes with the same ID will be merged, while new nodes will be added.
    /// Deleting Nodes and edges is not supported in this operation.
    pub fn merge(&mut self, mergee: Layer) -> std::result::Result<(), AtlasError> {
        let node_ids: HashSet<usize> = mergee.nodes.iter().map(|n| n.id).collect();
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
                Err(AtlasError::NodeNotFound) => {
                    drop(existing_node);
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
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            pos_index: None,
        }
    }
    pub fn with_pos_index() -> Self {
        Self {
            nodes: Vec::new(),
            pos_index: Some(KdTree::new()),
        }
    }

    /// Delete a node by its ID, removing all associated edges in the layer.
    pub(super) fn del_node(&mut self, id: usize) -> Result<Node> {
        let index = self
            .nodes
            .iter()
            .position(|node| node.id == id)
            .ok_or(AtlasError::NodeNotFound)?;
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
    pub(super) fn retain_nodes(&mut self, retain_nodes: &[usize]) {
        self.nodes.retain(|node| retain_nodes.contains(&node.id));
        self.prune();
    }
}

impl Default for Layer {
    fn default() -> Self {
        Self::new()
    }
}

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

pub enum PosQueryCount {
    All,
    Count(usize),
}
impl PosQueryCount {
    fn as_capped_usize(&self, cap: usize) -> usize {
        match self {
            PosQueryCount::All => cap,
            PosQueryCount::Count(k) => *k,
        }
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
