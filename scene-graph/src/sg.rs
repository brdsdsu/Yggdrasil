use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::{
    AnonNode, BoundingBox, NodeId,
    error::{Result, SceneGraphError},
    fov::Observer,
    layer::{Layer, NodeMut},
    node::{Coordinate, Edge, Feature, Node},
    pos::{PosQueryCount, PositionQuery},
};

/// A hierarchical representation of objects and their relationships in a 3D environment.
/// The scene graph is organized into layers, where each layer contains nodes representing objects.
/// The layers together form a DAG Structure, with the top layer representing the root layer
/// of the scene.
/// Within each layer, nodes can have edges connecting them to other nodes in the same layer,
/// forming a graph structure.
/// Nodes on a layer can also have parent-child relationships with nodes of layers that are
/// children of the layer they belong to.
///
/// The SceneGraph is optimized for executing spatical queries that are used in 3D environments
/// such as navigation, localization, and mapping.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SceneGraph {
    /// Layers of the scene graph, where each layer is either a Semantic or a Physical
    /// representation of the scene.
    pub(crate) layers: Vec<Layer>,

    /// Counter to assign unique IDs to nodes.
    node_counter: u32,

    /// Counter to assign unique IDs to layers.
    layer_counter: u32,
}

impl SceneGraph {
    /// Create a new layer without position indexing and add it to the scene graph.
    ///
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: SceneGraph::nearest_nodes
    /// [`nodes_within`]: SceneGraph::nodes_within
    pub fn new_layer(&mut self) -> &mut Layer {
        let lid = self.layer_counter;
        self.layer_counter += 1;
        self.layers.push(Layer::new(lid));
        self.layers.last_mut().unwrap()
    }

    /// Create a new layer with position indexing enabled and add it to the scene graph.
    ///
    /// This allows for efficient spatial queries on node positions.
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: SceneGraph::nearest_nodes
    /// [`nodes_within`]: SceneGraph::nodes_within
    pub fn new_indexed_layer(&mut self) -> &mut Layer {
        let lid = self.layer_counter;
        self.layer_counter += 1;
        self.layers.push(Layer::with_pos_index(lid));
        self.layers.last_mut().unwrap()
    }

    /// Get the number of layers in the scene graph.
    pub fn layers_count(&self) -> usize {
        self.layers.len()
    }
}

/// SceneGraph Update
impl SceneGraph {
    /// Merge another SceneGraph into this one.
    /// This Process will not delete any nodes or edges, but will apply any change in nodes
    /// features and/or edges between two nodes that exist in both SceneGraphs.
    pub fn merge(&mut self, m: SceneGraph) -> Result<()> {
        for mergee_node in m.layers.iter().flat_map(|l| l.nodes.iter()) {
            if let Some(pid) = mergee_node.pid {
                self.nest(mergee_node.id).under(pid)?;
            }
        }
        self.layers
            .iter_mut()
            .zip(m.layers)
            .try_for_each(|(l1, l2)| l1.merge(l2))
    }
}

/// Layer Accessors
impl SceneGraph {
    /// Get a mutable reference to the top layer.
    pub fn top_layer_mut(&mut self) -> Result<&mut Layer> {
        self.layers.last_mut().ok_or(SceneGraphError::LayerNotFound)
    }

    /// Get an immutable reference to the top layer.
    pub fn top_layer(&self) -> Result<&Layer> {
        self.layers.last().ok_or(SceneGraphError::LayerNotFound)
    }

    /// Get an immutable reference to a layer by its index.
    pub fn layer(&self, id: u32) -> Result<&Layer> {
        let layer_index = self
            .layers
            .binary_search_by_key(&id, |l| l.id)
            .or(Err(SceneGraphError::LayerNotFound))?;
        Ok(&self.layers[layer_index])
    }

    /// Get a mutable reference to a layer by its index.
    pub fn layer_mut(&mut self, id: u32) -> Result<&mut Layer> {
        let layer_index = self
            .layers
            .binary_search_by_key(&id, |l| l.id)
            .or(Err(SceneGraphError::LayerNotFound))?;
        Ok(&mut self.layers[layer_index])
    }

    /// Get the layer index of a node by its ID.
    #[inline]
    pub fn layer_of(&self, nid: NodeId) -> u32 {
        nid.layer_id()
    }
}

/// Node Accessors
impl SceneGraph {
    /// Get an immutable reference to a node by its ID.
    pub fn node(&self, nid: NodeId) -> Result<&Node> {
        let lid = nid.layer_id();
        self.layer(lid)?.node(nid)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn node_mut(&mut self, nid: NodeId) -> Result<NodeMut<'_>> {
        let lid = nid.layer_id();
        self.layer_mut(lid)?.node_mut(nid)
    }
}

/// Node Manipulation
impl SceneGraph {
    /// Create a new Metric Node with specified coordinates and features.
    pub fn new_coordinates(&mut self, x: f32, y: f32, z: f32, features: Vec<Feature>) -> AnonNode {
        let node = AnonNode::new(
            self.node_counter,
            features,
            Some(Coordinate::new(x, y, z)),
            false,
            Some(BoundingBox::point(Coordinate::new(x, y, z))),
        );
        self.node_counter += 1;
        node
    }

    /// Create a new Semantic Node with specified features.
    pub fn new_node(&mut self, features: Vec<Feature>) -> AnonNode {
        let node = AnonNode::new(self.node_counter, features, None, false, None);
        self.node_counter += 1;
        node
    }

    /// Create a new Indexed Semantic Node with specified features.
    /// This node will have position indexing enabled and will track of the coordinates of its
    /// sub-tree for efficient spatial queries.
    pub fn new_indexed_node(&mut self, features: Vec<Feature>) -> AnonNode {
        let node = AnonNode::new(
            self.node_counter,
            features,
            None,
            true,
            Some(Default::default()),
        );
        self.node_counter += 1;
        node
    }

    /// Delete a node by its ID from the Scene Graph.
    /// This will also recursively delete all child nodes of the specified node.
    /// If the node has a parent, it will be removed from the parent's list of children.
    pub fn del_node(&mut self, nid: NodeId) -> Result<()> {
        // Remove node from its parent's children list
        let lid = nid.layer_id();
        let layer = self.layer_mut(lid)?;

        if let Some(pid) = layer.node(nid)?.pid {
            // remove nodes subtree of points from the parent node's position index all the way up
            // to the root.
            self.bubble_up_points_removal(pid, self.pos_subtree(nid))?;

            // SAFETY: Operations done on the parent:
            // 1. Removing a child from it's children List
            //
            // So no layer-level logic is violated
            unsafe { self.node_mut_unsynced(pid)? }.remove_child(nid)?;

            // update the bounding box of the parent node and all the way up to the root.
            self.bubble_up_bounds_removal(pid, self.node(nid)?.bounds)?;
        }

        // Recursively delete the node and its children
        fn del_node_recursive(sg: &mut SceneGraph, lid: u32, nid: NodeId) -> Result<()> {
            let layer = sg.layer_mut(lid)?;
            let children = layer.del_node(nid)?.children;
            for child_id in children {
                del_node_recursive(sg, lid - 1, child_id)?;
            }
            Ok(())
        }
        del_node_recursive(self, lid, nid)
    }

    /// Nest a node under another node, establishing a parent-child relationship.
    /// The `nestee` node will become a child of the `nester` node.
    /// Both nodes must exist in the scene graph.
    /// The `nester` node must be on the layer immediately above the `nestee` node.
    /// If the `nestee` node already has a parent, it will be removed from its current parent's list of children.
    /// The `nester` node will have the `nestee` node added to its list of children.
    ///
    /// ```rust
    /// # use scene_graph::SceneGraph;
    /// # use scene_graph::Result;
    ///
    /// # fn main() -> Result<()> {
    /// # let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let node1 = sg.new_node(vec![]);
    /// let node2 = sg.new_node(vec![]);
    ///
    /// // Create a layer and add nodes to it
    /// let lid0 = sg.new_layer().id;
    /// let layer = sg.new_layer();
    /// layer.add_child(lid0)?;
    /// let id1 = sg.layer_mut(0)?.push_node(node1);
    /// let id2 = sg.layer_mut(1)?.push_node(node2);
    ///
    /// // Nest node1 under node2
    /// sg.nest(id1).under(id2)?;
    ///
    /// assert_eq!(sg.node(id2)?.children, vec![id1]);
    /// assert_eq!(sg.node(id1)?.pid, Some(id2));
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn nest(&mut self, nid: NodeId) -> NestUnder<'_> {
        NestUnder {
            sg: self,
            nestee: nid,
        }
    }
}

/// Query
impl SceneGraph {
    /// Get List of all nodes having a specific set of features.
    pub fn nodes_having(&self, keys: &[&str]) -> Vec<Vec<&Node>> {
        self.layers.iter().map(|l| l.nodes_having(keys)).collect()
    }

    /// Get List of all nodes matching a specific set of features.
    pub fn nodes_matching(&self, features: &[&Feature]) -> Vec<Vec<&Node>> {
        self.layers
            .iter()
            .map(|l| l.nodes_matching(features))
            .collect()
    }

    /// Get a subgraph containing nodes within the field of view of an observer and are descendants of the specified root node.
    /// The check is done using the nodes' coordinates and nodes without coordinates are pruned.
    /// nodes from upper layers that have no descendants within the field of view are also pruned.
    pub fn visible_subgraph(&self, observer: Observer, root_node_id: NodeId) -> Result<Self> {
        let mut subgraph = self.subgraph(root_node_id)?;
        if subgraph.layers.is_empty() {
            return Ok(SceneGraph::default());
        }

        let mut should_retain_nodes = HashMap::new();
        for layer in &subgraph.layers {
            should_retain_nodes.insert(layer.id, HashSet::new());
        }
        let mut layers = Vec::new();
        for layer in &subgraph.layers {
            let observed = should_retain_nodes.remove(&layer.id).unwrap_or_default();
            let observed: Vec<_> = layer
                .observable_nodes(observer)
                .nodes
                .iter()
                .map(|n| n.id)
                .chain(observed.into_iter())
                .collect();
            let l = layer.subset(&observed);
            for node in &l.nodes {
                if let Some(pid) = node.pid {
                    let parent_layer_id = subgraph.layer_of(pid);
                    should_retain_nodes
                        .entry(parent_layer_id)
                        .and_modify(|nodes| {
                            nodes.insert(pid);
                        });
                }
            }
            layers.push(l);
        }

        subgraph.layers = layers;
        Ok(subgraph)
    }

    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    ///
    /// ```rust
    /// # use scene_graph::SceneGraph;
    /// # use scene_graph::{Coordinate, PosQueryCount};
    ///
    /// let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let n1 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
    /// let n2 = sg.new_coordinates(1.0, 1.0, 1.0, vec![]);
    /// let n3 = sg.new_coordinates(2.0, 2.0, 2.0, vec![]);
    ///
    /// // Create a layer and add nodes to it
    /// sg.new_indexed_layer();
    /// sg.layer_mut(0).unwrap().push_node(n1);
    /// sg.layer_mut(0).unwrap().push_node(n2);
    /// sg.layer_mut(0).unwrap().push_node(n3);
    ///
    /// let coord = Coordinate::new(0.6, 0.6, 0.6);
    /// let nearest_nodes = sg.nearest_nodes(&coord, PosQueryCount::Count(2));
    /// assert_eq!(nearest_nodes.len(), 2);
    /// assert_eq!(nearest_nodes[0].0.coordinates.unwrap(), Coordinate::new(1.0, 1.0, 1.0));
    /// assert_eq!(nearest_nodes[1].0.coordinates.unwrap(), Coordinate::new(0.0, 0.0, 0.0));
    /// ```
    pub fn nearest_nodes(&self, coord: &Coordinate, count: PosQueryCount) -> Vec<(&Node, f32)> {
        self.layers
            .iter()
            .flat_map(|l| l.nearest_nodes(coord, count))
            .collect()
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    /// ```rust
    ///
    /// # use scene_graph::SceneGraph;
    /// # use scene_graph::{Coordinate, PosQueryCount};
    ///
    /// let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let n1 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
    /// let n2 = sg.new_coordinates(1.0, 1.0, 1.0, vec![]);
    /// let n3 = sg.new_coordinates(2.0, 2.0, 2.0, vec![]);
    ///
    /// // Create a layer and add nodes to it
    /// sg.new_indexed_layer();
    /// sg.layer_mut(0).unwrap().push_node(n1);
    /// sg.layer_mut(0).unwrap().push_node(n2);
    /// sg.layer_mut(0).unwrap().push_node(n3);
    ///
    /// let coord = Coordinate::new(0.3, 0.3, 0.3);
    /// let nodes_within_radius = sg.nodes_within(&coord, 2.0, PosQueryCount::All);
    /// assert_eq!(nodes_within_radius.len(), 2);
    /// assert!(nodes_within_radius.iter().any(|n| n.0.coordinates.unwrap() == Coordinate::new(0.0, 0.0, 0.0)));
    /// assert!(nodes_within_radius.iter().any(|n| n.0.coordinates.unwrap() == Coordinate::new(1.0, 1.0, 1.0)));
    /// ```
    pub fn nodes_within(
        &self,
        coord: &Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(&Node, f32)> {
        self.layers
            .iter()
            .flat_map(|l| l.nodes_within(coord, radius, count))
            .collect()
    }

    /// Retrieve at most `count` nearest nodes to the given position within the sub-tree rooted at the specified node ID.
    /// If this function is called frequently, consider creating the Node with indexing enabled.
    /// ```rust
    ///
    /// # use scene_graph::SceneGraph;
    /// # use scene_graph::{Coordinate, PosQueryCount};
    /// # use scene_graph::Result;
    ///
    /// # fn main() -> Result<()> {
    /// let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let n1 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
    /// let n2 = sg.new_coordinates(1.0, 1.0, 1.0, vec![]);
    /// let n3 = sg.new_coordinates(2.0, 2.0, 2.0, vec![]);
    ///
    /// // Create a layer and add nodes to it
    /// let first_layer_id = sg.new_indexed_layer().id;
    /// let n1_id = sg.layer_mut(0)?.push_node(n1);
    /// let n2_id = sg.layer_mut(0)?.push_node(n2);
    /// let n3_id = sg.layer_mut(0)?.push_node(n3);
    ///
    /// // create new semantic layer and nest all nodes under a new parent node
    /// let semantic_layer = sg.new_layer();
    /// semantic_layer.add_child(first_layer_id)?;
    /// let parent_node = sg.new_indexed_node(vec![]);
    /// let pid = sg.top_layer_mut()?.push_node(parent_node);
    /// sg.nest(n1_id).under(pid)?;
    /// sg.nest(n2_id).under(pid)?;
    /// sg.nest(n3_id).under(pid)?;
    ///
    /// let coord = Coordinate::new(0.6, 0.6, 0.6);
    /// let nearest_in_subtree = sg.nearest_in_subtree(&coord, pid, PosQueryCount::Count(2))?;
    /// assert_eq!(nearest_in_subtree.len(), 2);
    /// assert_eq!(nearest_in_subtree[0].0.coordinates.unwrap(), Coordinate::new(1.0, 1.0, 1.0));
    /// assert_eq!(nearest_in_subtree[1].0.coordinates.unwrap(), Coordinate::new(0.0, 0.0, 0.0));
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn nearest_in_subtree(
        &self,
        coord: &Coordinate,
        root: NodeId,
        count: PosQueryCount,
    ) -> Result<Vec<(&Node, f32)>> {
        match &self.node(root)?.pos_index {
            Some(pos_index) => pos_index.nearest_nodes(coord, count),
            None => self.pos_subtree(root).nearest_nodes(coord, count),
        }
        .into_iter()
        .map(|(id, distance)| Ok((self.node(id)?, distance)))
        .collect()
    }

    /// Get List of all edges matching a specific description.
    pub fn edges_matching(&self, desc: &str) -> Vec<Vec<&Edge>> {
        self.layers.iter().map(|l| l.edges_matching(desc)).collect()
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
        self.layers.iter().flat_map(|l| l.edges_to(dst)).collect()
    }
}

impl SceneGraph {
    /// Get list of all coordinates in the sub-tree rooted at the specified node ID.
    fn pos_subtree(&self, root: NodeId) -> Vec<(Coordinate, NodeId)> {
        let mut coords = Vec::new();
        let node = match self.node(root) {
            Ok(n) => n,
            Err(_) => return coords,
        };
        if let Some(c) = &node.coordinates {
            coords.push((*c, node.id));
        }
        if let Some(pos_index) = &node.pos_index {
            coords.extend(pos_index.iter().map(|(id, c)| (Coordinate::from(c), id)));
            return coords;
        }
        for cid in &node.children {
            coords.extend(self.pos_subtree(*cid));
        }
        coords
    }

    /// Get an unsynced mutable reference to the Node, preventing syncronization between the
    /// `Layer` and the `Node`.
    ///
    /// # SAFETY
    /// This function is unsafe because it allows modification of the Node without updating
    /// the Layer's position index. The caller must ensure that any changes to the Node's
    /// coordinates are properly reflected in the Layer's position index to maintain consistency.
    /// Get a mutable reference to a node by its ID.
    unsafe fn node_mut_unsynced(&mut self, id: NodeId) -> Result<&mut Node> {
        self.layers
            .iter_mut()
            .find_map(|layer| unsafe { layer.node_mut_unsynced(id) }.ok())
            .ok_or(SceneGraphError::NodeNotFound)
    }

    /// Create a subgraph rooted at the specified node ID.
    /// The subgraph includes the specified node and all its descendants.
    /// If the node is not found, an error is returned.
    fn subgraph(&self, root_node_id: NodeId) -> Result<SceneGraph> {
        let mut subgraph = SceneGraph::default();
        let root_layer_id = self.layer_of(root_node_id);
        let mut layers = HashSet::new();

        // BFS to get all layers accissable from root_layer_id downwards
        let mut layers_queue = VecDeque::new();
        layers_queue.push_back(root_layer_id);
        while let Some(layer_id) = layers_queue.pop_front() {
            let layer = self.layer(layer_id)?;
            layers.insert(layer_id);
            for &clid in layer.children.iter() {
                layers_queue.push_back(clid);
            }
        }
        let mut layers = layers.into_iter().collect::<Vec<u32>>();
        layers.sort();
        let mut layers = layers
            .iter()
            .map(|&lid| (lid, HashSet::new()))
            .collect::<Vec<_>>();

        // layers are in topological order from root downwards. so by the time we reach to a layer,
        // all its parents have been processed.
        let last_index = layers.len() - 1;
        layers[last_index].1.insert(root_node_id);
        while let Some((lid, nodes)) = layers.pop() {
            if nodes.is_empty() {
                continue;
            }
            let layer = self.layer(lid)?;
            let nodes: Vec<_> = nodes.into_iter().collect();
            let l = layer.subset(&nodes);
            for nid in nodes {
                for cid in &l.node(nid)?.children {
                    let clid = self.layer_of(*cid);
                    let ptr = layers
                        .binary_search_by_key(&clid, |(lid, _)| *lid)
                        .map_err(|_| SceneGraphError::LayerNotFound)?;
                    layers[ptr].1.insert(*cid);
                }
            }
            subgraph.layers.push(l);
        }
        subgraph.layers.reverse();
        subgraph.layer_counter = self.layer_counter;
        subgraph.node_counter = self.node_counter;
        Ok(subgraph)
    }

    fn bounds_of(&self, nid: NodeId) -> Result<Option<BoundingBox>> {
        let node = self.node(nid)?;
        if let Some(bounds) = node.bounds {
            Ok(Some(bounds))
        } else {
            self.build_bounding_box(nid)
        }
    }

    fn build_bounding_box(&self, nid: NodeId) -> Result<Option<BoundingBox>> {
        let node = self.node(nid)?;
        let bounds_iter = node
            .children
            .iter()
            .map(|&cid| self.bounds_of(cid))
            .filter_map(Result::ok);
        let x = bounds_iter.chain(vec![node.coordinates.map(BoundingBox::point)]);

        let bounds = x.fold(None, |acc: Option<BoundingBox>, b| match acc {
            Some(mut acc) => {
                if let Some(b) = b {
                    acc.merge(&b);
                }
                Some(acc)
            }
            None => b,
        });

        if let Some(coords) = node.coordinates {
            match bounds {
                Some(mut b) => {
                    b.merge(&BoundingBox::point(coords));
                    return Ok(Some(b));
                }
                None => return Ok(Some(BoundingBox::point(coords))),
            }
        }

        Ok(bounds)
    }

    /// Propagate the deletion of points in the child subtree to the parent node and all the way up
    /// to the root.
    fn bubble_up_points_removal(
        &mut self,
        pid: NodeId,
        pos_subtree: Vec<(glam::Vec3, NodeId)>,
    ) -> Result<()> {
        // SAFETY: Operations done on the parent:
        // 1. Removal of points from it's index
        //
        // So no layer-level logic is violated
        let parent = unsafe { self.node_mut_unsynced(pid)? };

        if let Some(pos_index) = parent.pos_index.as_mut() {
            for (c, nid) in &pos_subtree {
                pos_index.remove(&(*c).into(), *nid);
            }
        }

        match self.node(pid)?.pid {
            Some(ppid) => self.bubble_up_points_removal(ppid, pos_subtree),
            None => Ok(()),
        }
    }

    /// Propagate the deletion of a cihld node's bounds to the parent node and all the way up to
    /// the root.
    ///
    /// Invariantes:
    /// - This function should only be called when the child node is removed from the parent's
    ///   subtree since it might try to rebuild the bounding box using the subtree.
    fn bubble_up_bounds_removal(
        &mut self,
        pid: NodeId,
        deleted: Option<BoundingBox>,
    ) -> Result<()> {
        match deleted {
            Some(deleted) => {
                if let Some(bounds) = self.node(pid)?.bounds {
                    // recalculate the bounding box of the parent if it shares border with the deleted bounds.
                    if bounds.shares_border_with(&deleted) {
                        let bounds = self.build_bounding_box(pid)?;
                        unsafe {
                            // SAFETY: Operations done on the parent:
                            // 1. Updating the bounding box of the parent if it exists
                            //
                            // So no layer-level logic is violated
                            let parent = self.node_mut_unsynced(pid)?;
                            parent.bounds = bounds;
                        }
                        // propagate the new bounds to the ancestors if available
                        match self.node(pid)?.pid {
                            Some(ppid) => self.bubble_up_bounds_removal(ppid, bounds),
                            None => Ok(()),
                        }
                    } else {
                        // no change made to the bounds, propagation haults
                        Ok(())
                    }
                } else {
                    match self.node(pid)?.pid {
                        Some(ppid) => self.bubble_up_bounds_removal(ppid, Some(deleted)),
                        None => Ok(()),
                    }
                }
            }
            None => Ok(()),
        }
    }

    /// Propagate the new positions of the child subtree to the parent node and all the way up to
    /// the root.
    fn bubble_up_points_addition(
        &mut self,
        parent: NodeId,
        pos: Vec<(glam::Vec3, NodeId)>,
    ) -> Result<()> {
        // SAFETY: Operations done to the nester node:
        // 1. Update position index with the new child subtree
        //
        // So no layer-level logic is violated
        let node = unsafe { self.node_mut_unsynced(parent)? };
        let pos_index = node.pos_index.as_mut().unwrap();
        pos.iter()
            .for_each(|(coords, id)| pos_index.add(&(*coords).into(), *id));

        match node.pid {
            Some(pid) => self.bubble_up_points_addition(pid, pos),
            None => Ok(()),
        }
    }

    /// Propagate the new bounds of the child to the parent node and all the way up to the root.
    fn bubble_up_bounds_addition(
        &mut self,
        parent: NodeId,
        new_bounds: Option<BoundingBox>,
    ) -> Result<()> {
        if new_bounds.is_none() {
            return Ok(());
        }
        // SAFETY: Operations done to the nester node:
        // 1. Update bounding box with the new child
        //
        // So no layer-level logic is violated
        let node = unsafe { self.node_mut_unsynced(parent)? };

        if let Some(node_bounds) = node.bounds.as_mut()
            && let Some(new_bounds) = new_bounds
        {
            node_bounds.merge(&new_bounds);
        }

        match node.pid {
            Some(pid) => self.bubble_up_bounds_addition(pid, new_bounds),
            None => Ok(()),
        }
    }
}

/// An intermediate struct to facilitate the nesting of one node under another in a SceneGraph.
/// Refer to the `nest` method in `SceneGraph` for usage example.
///
/// [`nest`](SceneGraph::nest)
pub struct NestUnder<'a> {
    sg: &'a mut SceneGraph,
    nestee: NodeId,
}

impl<'a> NestUnder<'a> {
    /// Complete the nesting operation by specifying the `nester` node under which the `nestee` node
    /// Refer to the `nest` method in `SceneGraph` for usage example.
    ///
    /// [`nest`](SceneGraph::nest)
    pub fn under(&mut self, nester: NodeId) -> Result<&mut SceneGraph> {
        let nester_layer_id = self.sg.layer_of(nester);
        let nestee_layer_id = self.sg.layer_of(self.nestee);
        let nester_layer = self.sg.layer(nester_layer_id)?;
        if !nester_layer.children.contains(&nestee_layer_id) {
            return Err(SceneGraphError::InvalidLayersForNesting(
                nestee_layer_id,
                nester_layer_id,
            ));
        }

        let nestee = unsafe { self.sg.node_mut_unsynced(self.nestee)? };
        match nestee.pid {
            // Remove from old parent
            Some(parent_id) => {
                nestee.pid = Some(nester);
                self.sg.node_mut(parent_id)?.remove_child(self.nestee)?;
            }
            None => {
                nestee.pid = Some(nester);
            }
        }

        // SAFETY: Operations done to the nester node:
        // 1. Adding a new child to it's children list
        // 2. Update position index with the new child subtree
        let node = unsafe { self.sg.node_mut_unsynced(nester)? };
        node.add_child(self.nestee);

        // propagete the new positions to the nester and all its ancestors with position index enabled.
        self.sg.bubble_up_points_addition(nester,self.sg.pos_subtree(self.nestee))?;

        // propagate the new bounds to the nester and all its ancestors.
        self.sg.bubble_up_bounds_addition(nester, self.sg.bounds_of(self.nestee)?)?;

        Ok(self.sg)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn subgraph() -> Result<()> {
        // creating a scene graph with 3 layers, with the following structure:
        // layers structure:
        // Layer 2
        //   ├── Layer 1
        //   └────└── Layer 0
        //
        // Nodes structure:
        // n2
        // ├── n1
        // └────── n0
        // m2
        // ├── m1
        // └────── m0
        let mut sg = SceneGraph::default();
        sg.new_layer();
        sg.new_layer();
        sg.new_layer();

        sg.top_layer_mut()?.add_child(0)?;
        sg.top_layer_mut()?.add_child(1)?;
        sg.layer_mut(1)?.add_child(0)?;

        let n0 = sg.new_node(vec![Feature::new("name", "n0")]);
        let n1 = sg.new_node(vec![Feature::new("name", "n1")]);
        let n2 = sg.new_node(vec![Feature::new("name", "n2")]);

        let m0 = sg.new_node(vec![Feature::new("name", "m0")]);
        let m1 = sg.new_node(vec![Feature::new("name", "m1")]);
        let m2 = sg.new_node(vec![Feature::new("name", "m2")]);

        let n0 = sg.layer_mut(0)?.push_node(n0);
        let n1 = sg.layer_mut(1)?.push_node(n1);
        let n2 = sg.layer_mut(2)?.push_node(n2);

        let m0 = sg.layer_mut(0)?.push_node(m0);
        let m1 = sg.layer_mut(1)?.push_node(m1);
        let m2 = sg.layer_mut(2)?.push_node(m2);

        sg.nest(n0).under(n2)?;
        sg.nest(n1).under(n2)?;

        sg.nest(m0).under(m2)?;
        sg.nest(m1).under(m2)?;

        // subgraph for n2 should only contain n2, n1, n0
        let subgraph_n2 = sg.subgraph(n2)?;
        assert_eq!(subgraph_n2.layers.len(), 3);
        assert_eq!(subgraph_n2.layer(0)?.nodes.len(), 1);
        assert_eq!(subgraph_n2.layer(1)?.nodes.len(), 1);
        assert_eq!(subgraph_n2.layer(2)?.nodes.len(), 1);
        assert_eq!(subgraph_n2.node(n2)?.features[0].value(), "n2");
        assert_eq!(subgraph_n2.node(n1)?.features[0].value(), "n1");
        assert_eq!(subgraph_n2.node(n0)?.features[0].value(), "n0");

        // subgraph for m1 should only contain m1
        let subgraph_m1 = sg.subgraph(m1)?;
        assert_eq!(subgraph_m1.layers.len(), 1);
        assert_eq!(subgraph_m1.layer(1)?.nodes.len(), 1);
        assert_eq!(subgraph_m1.node(m1)?.features[0].value(), "m1");
        assert_eq!(subgraph_m1.layer(1)?.id, sg.layer_of(m1));
        // there should be no layer with id 0 in the subgraph
        assert!(subgraph_m1.layer(0).is_err());
        Ok(())
    }

    #[test]
    fn del_node_index_propagation() -> Result<()> {
        // creating a scene graph with 2 layers, with the following structure:
        // layers structure:
        // Layer 1
        // └── Layer 0
        // Nodes structure:
        // m0
        // ├── n0 ...
        // └── n10
        //
        // each node in layer 0 has 10 coordinates, namely, (i,i,i) for the i'th node.
        //
        // Scenario:
        // we first check if nearest neighbor query on n0 coordinates returns the n0's
        // coordinates when run on m0; then we delete n0 and repeate the same query, which should
        // not return n0's coordinates anymore.

        // 1. creating the scene graph
        let mut sg = SceneGraph::default();
        sg.new_layer();
        sg.new_layer();
        sg.layer_mut(1)?.add_child(0)?;
        let m0 = sg.new_indexed_node(vec![Feature::new("name", "m0")]);
        let m0_id = sg.layer_mut(1)?.push_node(m0);
        for i in 0..10 {
            let mut n = sg.new_indexed_node(vec![Feature::new("name", &format!("n{}", i))]);
            n.coordinates = Some(Coordinate::new(i as f32, i as f32, i as f32));
            let n_id = sg.layer_mut(0)?.push_node(n);
            sg.nest(n_id).under(m0_id)?;
        }

        // 2. nearest neighbor query on n0's coordinates should return n0
        let query_coord = Coordinate::new(0.0, 0.0, 0.0);
        let nearest = sg.nearest_in_subtree(&query_coord, m0_id, PosQueryCount::Count(1))?;
        assert_eq!(nearest.len(), 1);
        assert_eq!(
            nearest[0].0.coordinates.unwrap(),
            Coordinate::new(0.0, 0.0, 0.0)
        );

        // 3. delete n0 and repeat the same query, which should not return n0's coordinates anymore
        let n0_id = sg.nodes_matching(&[&Feature::new("name", "n0")])[0][0].id;
        sg.del_node(n0_id)?;
        let nearest_after_deletion =
            sg.nearest_in_subtree(&query_coord, m0_id, PosQueryCount::Count(1))?;
        assert_eq!(nearest_after_deletion.len(), 1);
        assert_ne!(
            nearest_after_deletion[0].0.coordinates.unwrap(),
            Coordinate::new(0.0, 0.0, 0.0)
        );

        Ok(())
    }
}
