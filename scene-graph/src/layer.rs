use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
};

use serde::{Deserialize, Serialize};

use crate::IntoNode;

use super::error::{Result, SceneGraphError};
use super::pos::{PosIndex, PosQueryCount, PositionQuery};
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
    pos_index: Option<PosIndex>,
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
            prev_coords: self.nodes[ptr].coordinates.clone(),
            layer: self,
            node_ptr: ptr,
        })
    }

    /// Add a new node to the layer.
    /// Returns the ID of the newly added node.
    ///
    /// Fails without adding the node if this layer is indexed and the index refuses the
    /// node's coordinates.
    pub fn push_node<N: IntoNode>(&mut self, node: N) -> Result<NodeId> {
        let node_id = NodeId::new(node.nid(), self.id);
        let node = node.into_node(node_id);
        if let Some(pos_index) = &mut self.pos_index {
            for coord in &node.coordinates {
                pos_index
                    .add_rebalancing(&(*coord).into(), node.id)
                    .map_err(|e| SceneGraphError::PosIndexInsertion(e.to_string()))?;
            }
        }
        let ptr = match self.nodes.binary_search_by_key(&node.id, |n| n.id) {
            Ok(ptr) => ptr,
            Err(ptr) => ptr,
        };
        let nid = node.id;
        self.nodes.insert(ptr, node);
        Ok(nid)
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

    /// Returns the IDs of layers that are direct children of this layer in the layer DAG.
    pub fn child_layer_ids(&self) -> &[u32] {
        &self.children
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
    pub fn observable_nodes(&self, observer: &Observer) -> Result<Self> {
        let nodes = self
            .nodes
            .iter()
            .filter(|n| !n.coordinates.is_empty())
            .filter(|n| n.coordinates.iter().any(|c| observer.observers(c)))
            .cloned()
            .collect::<Vec<Node>>();

        let pos_index = self.clone_pos_index_for(&nodes)?;

        let mut l = Self {
            id: self.id,
            nodes,
            pos_index,
            children: self.children.clone(),
        };

        // prune edges to out-of-view nodes
        l.prune();
        Ok(l)
    }

    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nearest_nodes(
        &self,
        pos: &super::Coordinate,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)> {
        match &self.pos_index {
            Some(pos_index) => pos_index.nearest_nodes(pos, count),
            None => self.nodes.nearest_nodes(pos, count),
        }
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)> {
        match &self.pos_index {
            Some(pos_index) => pos_index.nodes_within(pos, radius, count),
            None => self.nodes.nodes_within(pos, radius, count),
        }
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
                    self.push_node(node.clone())?;
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
            pos_index: Some(PosIndex::default()),
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
        if let Some(pos_index) = &mut self.pos_index {
            for coord in &node.coordinates {
                pos_index.remove(&(*coord).into(), node.id);
            }
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
    pub(crate) fn subset(&self, nodes: &[NodeId]) -> Result<Self> {
        let nodes: Vec<Node> = self
            .nodes
            .iter()
            .filter(|n| nodes.contains(&n.id))
            .cloned()
            .collect();
        let pos_index = self.clone_pos_index_for(&nodes)?;
        let mut l = Self {
            id: self.id,
            nodes,
            pos_index,
            children: self.children.clone(),
        };
        l.prune();
        Ok(l)
    }

    /// Build a new position index holding the coordinates of `nodes`, mirroring whether this
    /// layer is indexed: `None` if it is not, so derived layers keep the original's choice.
    fn clone_pos_index_for(&self, nodes: &[Node]) -> Result<Option<PosIndex>> {
        if self.pos_index.is_none() {
            return Ok(None);
        }
        let mut pos_index = PosIndex::default();
        for node in nodes {
            for coord in &node.coordinates {
                pos_index
                    .add_rebalancing(&(*coord).into(), node.id)
                    .map_err(|e| SceneGraphError::PosIndexInsertion(e.to_string()))?;
            }
        }
        Ok(Some(pos_index))
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
    prev_coords: Vec<super::Coordinate>,
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
        // Logic for updating the position index when a Node's coordinates are modified:
        // 1. If the layer does not have a position index, do nothing.
        // 2. If the Node's coordinates have not changed, do nothing.
        // 3. Calculate the difference between the previous coordinates and the new coordinates.
        //    - For each previous coordinate that is no longer in the new coordinates, remove it from the position index.
        //    - For each new coordinate that was not in the previous coordinates, add it to the position index.
        if self.layer.pos_index.is_none() {
            return;
        }

        let node = &**self;
        let new_coords = &node.coordinates;
        let node_id = node.id;

        if self.prev_coords == *new_coords {
            return;
        }
        // calculate diff between prev_coords and new_coords, and update the pos_index accordingly
        let mut new_coords_ptr = 0;
        let mut to_be_removed_coords = Vec::new();
        for prev_coord in &self.prev_coords {
            // A prev_coord that lines up with the coordinate under the pointer is already in the
            // pos_index and is left alone. Anything else the node no longer holds, so it is
            // removed — including every leftover prev_coord once `new_coords` is exhausted, which
            // is what shortening the list looks like from here.
            if new_coords_ptr < new_coords.len() && *prev_coord == new_coords[new_coords_ptr] {
                // found prev_coord in new_coords, advancing the pointer
                new_coords_ptr += 1;
            } else {
                // prev_coord is no longer in the new_coords, so it should be removed from the pos_index
                to_be_removed_coords.push(*prev_coord);
            }
        }
        // what remains in the `new_coords` is new and must be added to the `pos_index`
        let to_be_added_coords = new_coords
            .iter()
            .skip(new_coords_ptr)
            .cloned()
            .collect::<Vec<_>>();

        if let Some(pos_index) = &mut self.layer.pos_index {
            for c in to_be_removed_coords.iter() {
                pos_index.remove(&(*c).into(), node_id);
            }

            for c in to_be_added_coords {
                // A drop has nowhere to return an error to, and silently skipping the insert
                // would leave the index disagreeing with the layer it indexes. The only
                // reason this can fail is a leaf the tree cannot split, so it panics here
                // for the same reason the index does elsewhere: a wrong answer is worse.
                pos_index
                    .add_rebalancing(&c.into(), node_id)
                    .expect("position index refused the new coordinates of a node");
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
        // Observer at origin, looking down +Z
        let pos = Coordinate::new(0.0, 0.0, 0.0);
        let direction = Coordinate::Z;

        // Cone View Frustum: half-angle=35°, near=0.6, far=6.0
        let half_angle = 35_f32.to_radians();
        let near = 0.6;
        let far = 6.0;

        Observer::new_looking_along(pos, direction, half_angle, near, far).unwrap()
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
            layer
                .push_node(AnonNode::new(i as u32, Vec::new(), vec![*p], false, None))
                .unwrap();
        }
        // Node with no coordinates
        layer
            .push_node(AnonNode::new(
                pts.len() as u32,
                Vec::new(),
                Vec::new(),
                false,
                None,
            ))
            .unwrap();

        // fully connecting nodes to each other
        for src in 0..layer.nodes.len() {
            for dst in 0..layer.nodes.len() {
                let src = layer.node_id(src as u32);
                let dst = layer.node_id(dst as u32);
                layer.add_edge(src, dst, "connect").unwrap();
            }
        }

        let cone = cone();
        let observed_layer = layer.observable_nodes(&cone).unwrap();
        assert_eq!(observed_layer.nodes.len(), 3);
        for node in observed_layer.nodes {
            // all nodes should have coordinates to be observable
            assert!(!node.coordinates.is_empty());
            // all nodes should be within the cone frustum
            node.coordinates
                .iter()
                .for_each(|c| assert!(cone.observers(c)));
            // nodes 0,1,2 are inside, rest are outside, so only edges to 0,1,2 should remain
            assert_eq!(
                node.edges.iter().map(|e| e.dst).collect::<Vec<_>>(),
                (0..=2).map(|i| layer.node_id(i)).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn nearest_query() -> Result<()> {
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
            l.push_node(AnonNode::new(i as u32, Vec::new(), vec![*p], false, None))?;
        }

        let query_pos = Coordinate::new(0.6, 0.6, 0.6);

        let nearest_nodes = l.nearest_nodes(&query_pos, PosQueryCount::Count(3));
        assert_eq!(nearest_nodes.len(), 3);
        // first 3 nearest nodes should be [1.0,1.0,1.0], [0.0,0.0,0.0], [2.0,2.0,2.0]
        assert_eq!(l.node(nearest_nodes[0].0)?.coordinates[0], pts[1]);
        assert_eq!(l.node(nearest_nodes[1].0)?.coordinates[0], pts[0]);
        assert_eq!(l.node(nearest_nodes[2].0)?.coordinates[0], pts[2]);

        Ok(())
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
            l.push_node(AnonNode::new(i as u32, Vec::new(), vec![*p], false, None))
                .unwrap();
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

    /// A node holds a list of coordinates, so a coordinate edit is not only a move: the list
    /// can also grow and shrink. The index has one entry per coordinate, so every one the node
    /// gave up has to be dropped from it — a shortened list is the case that leaves stale
    /// entries behind if only the coordinates that line up are compared.
    #[test]
    fn coordinate_edits_leave_no_stale_index_entries() -> Result<()> {
        let a = Coordinate::new(0.0, 0.0, 0.0);
        let b = Coordinate::new(1.0, 1.0, 1.0);
        let c = Coordinate::new(2.0, 2.0, 2.0);

        // `entries_at` counts index entries at a coordinate, which is what a stale entry shows
        // up as: a hit at a coordinate the node no longer holds.
        let entries_at = |l: &Layer, at: &Coordinate| l.nodes_within(at, 1e-6, PosQueryCount::All).len();

        let mut l = Layer::with_pos_index(0);
        let nid = l.push_node(AnonNode::new(0, Vec::new(), vec![a, b, c], false, None))?;
        assert_eq!(entries_at(&l, &c), 1);

        // dropping the tail of the list
        l.node_mut(nid)?.coordinates = vec![a, b];
        assert_eq!(entries_at(&l, &c), 0);
        assert_eq!(entries_at(&l, &a), 1);
        assert_eq!(entries_at(&l, &b), 1);

        // growing the list again
        l.node_mut(nid)?.coordinates = vec![a, b, c];
        assert_eq!(entries_at(&l, &c), 1);

        // dropping from the middle
        l.node_mut(nid)?.coordinates = vec![a, c];
        assert_eq!(entries_at(&l, &b), 0);
        assert_eq!(entries_at(&l, &a), 1);
        assert_eq!(entries_at(&l, &c), 1);

        // clearing the list entirely
        l.node_mut(nid)?.coordinates = Vec::new();
        assert_eq!(entries_at(&l, &a), 0);
        assert_eq!(entries_at(&l, &c), 0);

        Ok(())
    }

    /// Reordering the list is not a change to the point set the index holds, however the diff
    /// chooses to reach that state.
    #[test]
    fn reordering_coordinates_preserves_the_indexed_point_set() -> Result<()> {
        let a = Coordinate::new(0.0, 0.0, 0.0);
        let b = Coordinate::new(1.0, 1.0, 1.0);

        let mut l = Layer::with_pos_index(0);
        let nid = l.push_node(AnonNode::new(0, Vec::new(), vec![a, b], false, None))?;

        l.node_mut(nid)?.coordinates = vec![b, a];
        assert_eq!(l.nodes_within(&a, 1e-6, PosQueryCount::All).len(), 1);
        assert_eq!(l.nodes_within(&b, 1e-6, PosQueryCount::All).len(), 1);

        Ok(())
    }
}
