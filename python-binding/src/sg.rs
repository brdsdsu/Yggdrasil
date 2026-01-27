use std::sync::{Arc, Mutex};

use crate::{
    layer::{PyLayerRef, PyObserver, PyPosQueryCount},
    node::{PyEdge, PyFeature, PyAnonNode, PyNodeId, PyNodeRef},
};

use super::error::Result;
use pyo3::prelude::*;
use scene_graph::{Coordinate, NodeId, SceneGraph};

/// A hierarchical representation of objects and their relationships in a 3D environment.
/// The scene graph is organized into layers, where each layer contains nodes representing objects.
/// Nodes on adjacent layers can have parent-child relationships, and edges can represent various
/// types of connections between nodes.
///
/// The scene graph supports operations such as adding/removing nodes and edges,
/// nesting nodes under other nodes, and querying nodes by their IDs.
#[derive(Clone)]
#[pyclass(name = "SceneGraph")]
pub struct PySceneGraphRef {
    inner: Arc<Mutex<SceneGraph>>,
}

#[pymethods]
impl PySceneGraphRef {
    #[new]
    fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    /// Create a new layer without position indexing and add it to the scene graph.
    ///
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: SceneGraph::nearest_nodes
    /// [`nodes_within`]: SceneGraph::nodes_within
    pub fn new_layer(&mut self) -> PyLayerRef {
        let sg = &mut self.inner.lock().unwrap();
        let layer = sg.new_layer();
        PyLayerRef::new(Arc::clone(&self.inner), layer.id)
    }

    /// Create a new layer with position indexing enabled and add it to the scene graph.
    ///
    /// This allows for efficient spatial queries on node positions.
    /// Consider enabling position indexing if the layer will be frequently queried by `nearest_nodes` or `nodes_within`.
    ///
    /// [`nearest_nodes`]: SceneGraph::nearest_nodes
    /// [`nodes_within`]: SceneGraph::nodes_within
    pub fn new_indexed_layer(&mut self) -> PyLayerRef {
        let sg = &mut self.inner.lock().unwrap();
        let layer = sg.new_indexed_layer();
        PyLayerRef::new(Arc::clone(&self.inner), layer.id)
    }

    /// Merge another SceneGraph into this one.
    /// This Process will not delete any nodes or edges, but will apply any change in nodes
    /// features and/or edges between two nodes that exist in both SceneGraphs.
    pub fn merge(&mut self, m: PySceneGraphRef) -> Result<()> {
        let sg = &mut self.inner.lock().unwrap();
        let m = m.inner.lock().unwrap();

        sg.merge(m.clone()).map_err(Into::into)
    }

    /// Get a reference to the top layer.
    pub fn top_layer(&self) -> Result<PyLayerRef> {
        let sg = &self.inner.lock().unwrap();
        Ok(PyLayerRef::new(Arc::clone(&self.inner), sg.top_layer()?.id))
    }

    /// Get a reference to a layer by its id.
    pub fn layer(&self, id: u32) -> PyLayerRef {
        PyLayerRef::new(Arc::clone(&self.inner), id)
    }

    /// Get the layer index of a node by its ID.
    pub fn layer_of(&self, nid: PyNodeId) -> u32 {
        NodeId::from(nid).layer_id()
    }

    /// Get a reference to a node by its ID.
    pub fn node(&self, nid: PyNodeId) -> PyNodeRef {
        PyNodeRef::new(Arc::clone(&self.inner), nid.into())
    }

    /// Create a new Metric Node with specified coordinates and features.
    pub fn new_coordinates(&mut self, x: f32, y: f32, z: f32, features: Vec<PyFeature>) -> PyAnonNode {
        let features = features.iter().map(Into::into).collect();
        let node = self
            .inner
            .lock()
            .unwrap()
            .new_coordinates(x, y, z, features);
        PyAnonNode::new(node)
    }

    /// Create a new Indexed Semantic Node with specified features.
    /// This node will have position indexing enabled and will track of the coordinates of its
    /// sub-tree for efficient spatial queries.
    pub fn new_indexed_node(&mut self, features: Vec<PyFeature>) -> PyAnonNode {
        let features = features.iter().map(Into::into).collect();
        let node = self.inner.lock().unwrap().new_indexed_node(features);
        PyAnonNode::new(node)
    }

    /// Create a new Semantic Node with specified features.
    pub fn new_node(&mut self, features: Vec<PyFeature>) -> PyAnonNode {
        let features = features.iter().map(Into::into).collect();
        let node = self.inner.lock().unwrap().new_node(features);
        PyAnonNode::new(node)
    }

    /// Delete a node by its ID from the Scene Graph.
    /// This will also recursively delete all child nodes of the specified node.
    /// If the node has a parent, it will be removed from the parent's list of children.
    pub fn del_node(&mut self, nid: PyNodeId) -> Result<()> {
        self.inner.lock().unwrap().del_node(nid.into()).map_err(Into::into)
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
    pub fn nest(&mut self, nestee: PyNodeId, nester: PyNodeId) -> Result<()> {
        let _ = self.inner.lock().unwrap().nest(nestee.into()).under(nester.into());
        Ok(())
    }

    /// Get List of all nodes having a specific set of features.
    pub fn nodes_having(&self, keys: Vec<String>) -> Vec<Vec<PyNodeRef>> {
        let keys = keys.iter().map(String::as_str).collect::<Vec<&str>>();
        self.inner
            .lock()
            .unwrap()
            .nodes_having(&keys)
            .into_iter()
            .map(|layer_nodes| {
                layer_nodes
                    .into_iter()
                    .map(|n| PyNodeRef::new(Arc::clone(&self.inner), n.id))
                    .collect()
            })
            .collect()
    }

    /// Get List of all nodes matching a specific set of features.
    pub fn nodes_matching(&self, features: Vec<PyFeature>) -> Vec<Vec<PyNodeRef>> {
        let features = features
            .iter()
            .map(|f| f.into())
            .collect::<Vec<scene_graph::Feature>>();
        let features = features.iter().collect::<Vec<&scene_graph::Feature>>();
        self.inner
            .lock()
            .unwrap()
            .nodes_matching(&features)
            .into_iter()
            .map(|layer_nodes| {
                layer_nodes
                    .into_iter()
                    .map(|n| PyNodeRef::new(Arc::clone(&self.inner), n.id))
                    .collect()
            })
            .collect()
    }

    /// Get a subgraph containing nodes within the field of view of an observer and are descendants of the specified root node.
    /// The check is done using the nodes' coordinates and nodes without coordinates are pruned.
    /// nodes from upper layers that have no descendants within the field of view are also pruned.
    pub fn visible_subgraph(&self, observer: PyObserver, root_node_id: PyNodeId) -> Result<Self> {
        let sg = &self.inner.lock().unwrap();
        let subgraph = sg.visible_subgraph(observer.into(), root_node_id.into())?;
        Ok(Self {
            inner: Arc::new(Mutex::new(subgraph)),
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
    /// assert_eq!(nearest_nodes[0].coordinates.unwrap(), Coordinate::new(1.0, 1.0, 1.0));
    /// assert_eq!(nearest_nodes[1].coordinates.unwrap(), Coordinate::new(0.0, 0.0, 0.0));
    /// ```
    pub fn nearest_nodes(
        &self,
        coord: (f32, f32, f32),
        count: PyPosQueryCount,
    ) -> Vec<(PyNodeRef, f32)> {
        let (x, y, z) = coord;
        self.inner
            .lock()
            .unwrap()
            .nearest_nodes(&Coordinate::new(x, y, z), count.into())
            .into_iter()
            .map(|(n, d)| (PyNodeRef::new(Arc::clone(&self.inner), n.id), d))
            .collect::<Vec<_>>()
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
    /// assert!(nodes_within_radius.iter().any(|n| n.coordinates.unwrap() == Coordinate::new(0.0, 0.0, 0.0)));
    /// assert!(nodes_within_radius.iter().any(|n| n.coordinates.unwrap() == Coordinate::new(1.0, 1.0, 1.0)));
    /// ```
    pub fn nodes_within(
        &self,
        coord: (f32, f32, f32),
        radius: f32,
        count: PyPosQueryCount,
    ) -> Vec<(PyNodeRef, f32)> {
        let (x, y, z) = coord;
        self.inner
            .lock()
            .unwrap()
            .nodes_within(&Coordinate::new(x, y, z), radius, count.into())
            .into_iter()
            .map(|(n, d)| (PyNodeRef::new(Arc::clone(&self.inner), n.id), d))
            .collect::<Vec<_>>()
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
        coord: (f32, f32, f32),
        root: PyNodeId,
        count: PyPosQueryCount,
    ) -> Result<Vec<(PyNodeRef, f32)>> {
        let coords = coord.into();
        self.inner
            .lock()
            .unwrap()
            .nearest_in_subtree(&coords, root.into(), count.into())
            .map(|nodes| {
                nodes
                    .into_iter()
                    .map(|(n, d)| (PyNodeRef::new(Arc::clone(&self.inner), n.id), d))
                    .collect::<Vec<_>>()
            })
            .map_err(Into::into)
    }

    /// Same as `nearest_in_subtree` but for multiple points.
    pub fn nearest_in_subtree_for_each(
        &self,
        points: Vec<(f32, f32, f32)>,
        root: PyNodeId,
        count: PyPosQueryCount,
    ) -> Result<Vec<Vec<(PyNodeRef, f32)>>> {
        points
            .iter()
            .map(|c| self.nearest_in_subtree(*c, root, count))
            .collect()
    }

    /// Same as `nearest_nodes` but for multiple points.
    pub fn nearest_nodes_for_each(
        &self,
        points: Vec<(f32, f32, f32)>,
        count: PyPosQueryCount,
    ) -> Vec<Vec<(PyNodeRef, f32)>> {
        points
            .iter()
            .map(|c| self.nearest_nodes(*c, count))
            .collect()
    }

    /// Same as `nodes_within` but for multiple points.
    pub fn nodes_within_for_each(
        &self,
        points: Vec<(f32, f32, f32)>,
        radius: f32,
        count: PyPosQueryCount,
    ) -> Vec<Vec<(PyNodeRef, f32)>> {
        points
            .iter()
            .map(|c| self.nodes_within(*c, radius, count))
            .collect()
    }

    /// Get List of all edges matching a specific description.
    pub fn edges_matching(&self, desc: &str) -> Vec<Vec<PyEdge>> {
        self.inner
            .lock()
            .unwrap()
            .edges_matching(desc)
            .into_iter()
            .map(|layer_edges| {
                layer_edges
                    .into_iter()
                    .map(Into::into)
                    .collect::<Vec<PyEdge>>()
            })
            .collect()
    }

    /// Get List of all edges from a specific source node.
    pub fn edges_from(&self, src: PyNodeId) -> Vec<PyEdge> {
        self.inner
            .lock()
            .unwrap()
            .edges_from(src.into())
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: PyNodeId) -> Vec<PyEdge> {
        self.inner
            .lock()
            .unwrap()
            .edges_to(dst.into())
            .into_iter()
            .map(Into::into)
            .collect()
    }
}
