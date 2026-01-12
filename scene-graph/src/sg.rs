use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

use crate::{
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
    node_counter: usize,

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

    /// Create a subgraph rooted at the specified node ID.
    /// The subgraph includes the specified node and all its descendants.
    /// If the node is not found, an error is returned.
    fn subgraph(&self, root_node_id: usize) -> Result<SceneGraph> {
        let mut subgraph = SceneGraph::default();
        let root_layer_id = self.layer_of(root_node_id)?;
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
            cur_layer = match self.layer(root_layer_id - layers.len()) {
                Ok(l) => l,
                Err(_) => break, // No more layers to process
            };
            nodes_to_visit = next_nodes_to_visit;
        }
        // Ensure the subgraph has the same number of layers as the original up to the root layer
        if root_layer_id > layers.len() {
            layers.extend(std::iter::repeat_with(Layer::new).take(root_layer_id - layers.len()));
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
            let nodes: Vec<usize> = nodes.into_iter().collect();
            let l = layer.subset(&nodes);
            for nid in nodes {
                for cid in &l.node(nid)?.children {
                    let clid = self.layer_of(*cid)?;
                    let ptr = layers.binary_search_by_key(&clid, |(lid, _)| *lid).unwrap();
                    layers[ptr].1.insert(*cid);
                }
            }
            subgraph.layers.push(l);
        }
        Ok(subgraph)
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
    pub fn layer_of(&self, nid: usize) -> Result<u32> {
        let nestee_layer_id = self
            .layers
            .iter()
            .position(|l| l.node(nid).is_ok())
            .ok_or(SceneGraphError::NodeNotFound)?;
        Ok(self.layers[nestee_layer_id].id)
    }
}

/// Node Accessors
impl SceneGraph {
    /// Get an immutable reference to a node by its ID.
    pub fn node(&self, nid: usize) -> Result<&Node> {
        self.layers
            .iter()
            .find_map(|layer| layer.node(nid).ok())
            .ok_or(SceneGraphError::NodeNotFound)
    }

    /// Get a mutable reference to a node by its ID.
    pub fn node_mut(&mut self, nid: usize) -> Result<NodeMut<'_>> {
        self.layers
            .iter_mut()
            .find_map(|layer| layer.node_mut(nid).ok())
            .ok_or(SceneGraphError::NodeNotFound)
    }
}

/// Node Manipulation
impl SceneGraph {
    /// Create a new Metric Node with specified coordinates and features.
    pub fn new_coordinates(&mut self, x: f32, y: f32, z: f32, features: Vec<Feature>) -> Node {
        let node = Node::new(
            self.node_counter,
            features,
            Some(Coordinate::new(x, y, z)),
            false,
        );
        self.node_counter += 1;
        node
    }

    /// Create a new Semantic Node with specified features.
    pub fn new_node(&mut self, features: Vec<Feature>) -> Node {
        let node = Node::new(self.node_counter, features, None, false);
        self.node_counter += 1;
        node
    }

    /// Create a new Indexed Semantic Node with specified features.
    /// This node will have position indexing enabled and will track of the coordinates of its
    /// sub-tree for efficient spatial queries.
    pub fn new_indexed_node(&mut self, features: Vec<Feature>) -> Node {
        let node = Node::new(self.node_counter, features, None, true);
        self.node_counter += 1;
        node
    }

    /// Delete a node by its ID from the Scene Graph.
    /// This will also recursively delete all child nodes of the specified node.
    /// If the node has a parent, it will be removed from the parent's list of children.
    pub fn del_node(&mut self, nid: usize) -> Result<()> {
        // Remove node from its parent's children list
        let lid = self.layer_of(nid)?;
        let layer = self.layer_mut(lid)?;
        if let Some(pid) = layer.node(nid)?.pid {
            self.layer_mut(lid + 1)?.node_mut(pid)?.remove_child(nid)?;
        }

        // Recursively delete the node and its children
        fn del_node_recursive(sg: &mut SceneGraph, lid: u32, nid: usize) -> Result<()> {
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
    /// # let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let node1 = sg.new_node(vec![]);
    /// let node2 = sg.new_node(vec![]);
    /// let id1 = node1.id;
    /// let id2 = node2.id;
    ///
    /// // Create a layer and add nodes to it
    /// sg.new_layer();
    /// sg.new_layer();
    /// sg.layer_mut(0).unwrap().push_node(node1);
    /// sg.layer_mut(1).unwrap().push_node(node2);
    ///
    /// // Nest node1 under node2
    /// sg.nest(id1).under(id2).unwrap();
    ///
    /// assert_eq!(sg.node(id2).unwrap().children, vec![id1]);
    /// assert_eq!(sg.node(id1).unwrap().pid, Some(id2));
    /// ```
    pub fn nest(&mut self, nid: usize) -> NestUnder<'_> {
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
    pub fn visible_subgraph(&self, observer: Observer, root_node_id: usize) -> Result<Self> {
        let subgraph_layers = self.subgraph(root_node_id)?.layers;

        if subgraph_layers.is_empty() {
            return Ok(Default::default());
        }
        let first_layer = subgraph_layers[0].observable_nodes(observer);

        let mut retain_nodes = first_layer
            .nodes
            .iter()
            .filter_map(|n| n.pid)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let mut layers = vec![first_layer];

        for mut layer in subgraph_layers.into_iter().skip(1) {
            layer.retain_nodes(&retain_nodes.into_iter().collect::<Vec<_>>());
            retain_nodes = layer
                .nodes
                .iter()
                .filter_map(|n| n.pid)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            layers.push(layer);
        }
        Ok(Self {
            node_counter: self.node_counter,
            layers,
        })
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
    ///
    /// let mut sg = SceneGraph::default();
    ///
    /// // Create nodes
    /// let n1 = sg.new_coordinates(0.0, 0.0, 0.0, vec![]);
    /// let n2 = sg.new_coordinates(1.0, 1.0, 1.0, vec![]);
    /// let n3 = sg.new_coordinates(2.0, 2.0, 2.0, vec![]);
    ///
    /// let n1_id = n1.id;
    /// let n2_id = n2.id;
    /// let n3_id = n3.id;
    ///
    /// // Create a layer and add nodes to it
    /// sg.new_indexed_layer();
    /// sg.layer_mut(0).unwrap().push_node(n1);
    /// sg.layer_mut(0).unwrap().push_node(n2);
    /// sg.layer_mut(0).unwrap().push_node(n3);
    ///
    /// // create new semantic layer and nest all nodes under a new parent node
    /// sg.new_layer();
    /// let parent_node = sg.new_indexed_node(vec![]);
    /// let pid = parent_node.id;
    /// sg.top_layer_mut().unwrap().push_node(parent_node);
    /// sg.nest(n1_id).under(pid).unwrap();
    /// sg.nest(n2_id).under(pid).unwrap();
    /// sg.nest(n3_id).under(pid).unwrap();
    ///
    /// let coord = Coordinate::new(0.6, 0.6, 0.6);
    /// let nearest_in_subtree = sg.nearest_in_subtree(&coord, pid, PosQueryCount::Count(2)).unwrap();
    /// assert_eq!(nearest_in_subtree.len(), 2);
    /// assert_eq!(nearest_in_subtree[0].0.coordinates.unwrap(), Coordinate::new(1.0, 1.0, 1.0));
    /// assert_eq!(nearest_in_subtree[1].0.coordinates.unwrap(), Coordinate::new(0.0, 0.0, 0.0));
    /// ```
    pub fn nearest_in_subtree(
        &self,
        coord: &Coordinate,
        root: usize,
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
    pub fn edges_from(&self, src: usize) -> Vec<&Edge> {
        match self.node(src) {
            Ok(n) => n.edges.iter().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: usize) -> Vec<&Edge> {
        self.layers.iter().flat_map(|l| l.edges_to(dst)).collect()
    }
}

impl SceneGraph {
    /// Get list of all coordinates in the sub-tree rooted at the specified node ID.
    fn pos_subtree(&self, root: usize) -> Vec<(Coordinate, usize)> {
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
    /// # Safety
    /// This function is unsafe because it allows modification of the Node without updating
    /// the Layer's position index. The caller must ensure that any changes to the Node's
    /// coordinates are properly reflected in the Layer's position index to maintain consistency.
    /// Get a mutable reference to a node by its ID.
    unsafe fn node_mut_unsynced(&mut self, id: usize) -> Result<&mut Node> {
        self.layers
            .iter_mut()
            .find_map(|layer| unsafe { layer.node_mut_unsynced(id) }.ok())
            .ok_or(SceneGraphError::NodeNotFound)
    }
}

/// An intermediate struct to facilitate the nesting of one node under another in a SceneGraph.
/// Refer to the `nest` method in `SceneGraph` for usage example.
///
/// [`nest`](SceneGraph::nest)
pub struct NestUnder<'a> {
    sg: &'a mut SceneGraph,
    nestee: usize,
}

impl<'a> NestUnder<'a> {
    /// Complete the nesting operation by specifying the `nester` node under which the `nestee` node
    /// Refer to the `nest` method in `SceneGraph` for usage example.
    ///
    /// [`nest`](SceneGraph::nest)
    pub fn under(&mut self, nester: usize) -> Result<&mut SceneGraph> {
        //! TODO: diccuss bringing back layer check for nesting
        // let nester_layer_id = self.sg.layer_of(nester)?;
        // let nestee_layer_id = self.sg.layer_of(self.nestee)?;
        //
        // if nester_layer_id - 1 != nestee_layer_id {
        //     return Err(SceneGraphError::InvalidLayersForNesting(
        //         nestee_layer_id,
        //         nester_layer_id,
        //     ));
        // }

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

        let node = unsafe { self.sg.node_mut_unsynced(nester)? };
        node.add_child(self.nestee);

        // propagate position index with the new child
        if node.pos_index.is_some() {
            let pos = self.sg.pos_subtree(self.nestee);
            let mut node = self.sg.node_mut(nester)?;
            let pos_index = node.pos_index.as_mut().unwrap();
            pos.iter()
                .for_each(|(coords, id)| pos_index.add(&(*coords).into(), *id));
        }

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

        sg.top_layer_mut()?.add_child(1)?;
        sg.top_layer_mut()?.add_child(2)?;
        sg.layer_mut(1)?.add_child(2)?;

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
        assert_eq!(subgraph_m1.layer(0)?.nodes.len(), 1);
        assert_eq!(subgraph_m1.node(m1)?.features[0].value(), "m1");
        assert_eq!(subgraph_m1.layer(0)?.id, sg.layer_of(m1)?);
        assert_eq!(subgraph_m1.layer(0)?.children.len(), 0);
        Ok(())
    }
}
