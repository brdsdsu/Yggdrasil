use std::sync::{Arc, Mutex};

use super::error::Result;
use pyo3::prelude::*;
use scene_graph::{Node, SceneGraph};

/// A node in the scene graph.
/// Each node is designated to a unique layer in the scene graph and within that layer, it can have
/// multiple edges to other nodes in the same layer. Nodes can also have parent-child relationships
/// with nodes in the layers directly above or below them.
/// Each node can hold a set of features, which are key-value pairs that provide additional
/// information about the node. Nodes also support storeing 3D coordinates which can be used for
/// Field-of-View calculations or spatial queries.
#[pyclass(name = "Node")]
pub struct PyNodeRef {
    sg: Arc<Mutex<SceneGraph>>,
    nid: usize,
}

#[pyclass(name = "__Node")]
#[derive(Debug, Clone)]
pub struct PyNode {
    inner: Node,
}
impl PyNode {
    pub fn new(inner: Node) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> Node {
        self.inner
    }
}

#[pymethods]
impl PyNode {
    /// Unique identifier for the node.
    #[getter]
    pub fn id(&self) -> usize {
        self.inner.id
    }

    /// Parent node Id, if node is nested under another node.
    #[getter]
    pub fn pid(&self) -> Option<usize> {
        self.inner.pid
    }

    /// Child node Ids from the lower layer, if node has nested nodes under it.
    #[getter]
    pub fn children(&self) -> Vec<usize> {
        self.inner.children.clone()
    }

    /// Edges to other nodes in the same layer.
    #[getter]
    pub fn edges(&self) -> Vec<PyEdge> {
        self.inner.edges.iter().map(Into::into).collect()
    }

    /// Features associated with the node.
    #[getter]
    pub fn features(&self) -> Vec<PyFeature> {
        self.inner.features.iter().map(Into::into).collect()
    }

    /// Optional 3D coordinates of the node.
    #[getter]
    pub fn coordinates(&self) -> Option<(f32, f32, f32)> {
        self.inner.coordinates.map(|c| (c.x, c.y, c.z))
    }

    #[setter]
    pub fn set_coordinates(&mut self, coords: Option<(f32, f32, f32)>) {
        self.inner.coordinates = coords.map(|(x, y, z)| scene_graph::Coordinate::new(x, y, z));
    }

    /// Check if the node has a feature with the specified key.
    pub fn has_feature(&self, key: &str) -> bool {
        self.inner.has_feature(key)
    }

    /// Check if the node has the exact key-value pair as a feature.
    pub fn match_feature(&self, f: &PyFeature) -> bool {
        self.inner.match_feature(&f.into())
    }

    /// Get the value of a feature by its key.
    pub fn feature(&self, key: &str) -> Result<String> {
        Ok(self.inner.feature(key)?.to_string())
    }

    /// Set or update a feature for the node.
    pub fn set_feature(&mut self, feature: PyFeature) {
        self.inner.set_feature(feature.into())
    }
}

impl PyNodeRef {
    pub fn new(sg: Arc<Mutex<SceneGraph>>, nid: usize) -> Self {
        Self { sg, nid }
    }
}

#[pymethods]
impl PyNodeRef {
    /// Unique identifier for the node.
    #[getter]
    pub fn id(&self) -> usize {
        self.nid
    }

    /// Parent node Id, if node is nested under another node.
    #[getter]
    pub fn pid(&self) -> Result<Option<usize>> {
        Ok(self.sg.lock().unwrap().node(self.nid)?.pid)
    }

    /// Child node Ids from the lower layer, if node has nested nodes under it.
    #[getter]
    pub fn children(&self) -> Result<Vec<usize>> {
        Ok(self.sg.lock().unwrap().node(self.nid)?.children.clone())
    }

    /// Edges to other nodes in the same layer.
    #[getter]
    pub fn edges(&self) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .node(self.nid)?
            .edges
            .iter()
            .map(Into::into)
            .collect())
    }

    /// Features associated with the node.
    #[getter]
    pub fn features(&self) -> Result<Vec<PyFeature>> {
        let sg = self.sg.lock().unwrap();
        let node = sg.node(self.nid)?;
        Ok(node.features.iter().cloned().map(|f| f.into()).collect())
    }

    /// Optional 3D coordinates of the node.
    #[getter]
    pub fn coordinates(&self) -> Result<Option<(f32, f32, f32)>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .node(self.nid)?
            .coordinates
            .map(|c| (c.x, c.y, c.z)))
    }

    #[setter]
    pub fn set_coordinates(&mut self, coords: Option<(f32, f32, f32)>) -> Result<()> {
        let coords = coords.map(|(x, y, z)| scene_graph::Coordinate::new(x, y, z));
        self.sg.lock().unwrap().node_mut(self.nid)?.coordinates = coords;
        Ok(())
    }

    /// Check if the node has a feature with the specified key.
    pub fn has_feature(&self, key: &str) -> Result<bool> {
        Ok(self.sg.lock().unwrap().node(self.nid)?.has_feature(key))
    }

    /// Check if the node has the exact key-value pair as a feature.
    pub fn match_feature(&self, f: &PyFeature) -> Result<bool> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .node(self.nid)?
            .match_feature(&f.into()))
    }

    /// Get the value of a feature by its key.
    pub fn feature(&self, key: &str) -> Result<String> {
        let sg = self.sg.lock().unwrap();
        let f = sg.node(self.nid)?.feature(key)?;
        Ok(String::from(f))
    }

    /// Set or update a feature for the node.
    pub fn set_feature(&mut self, feature: PyFeature) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .node_mut(self.nid)?
            .set_feature(feature.into());
        Ok(())
    }
}

/// An edge connecting two nodes in the same layer.
#[derive(Debug, Clone)]
#[pyclass(name = "Edge")]
pub struct PyEdge {
    /// Source node ID.
    pub src: usize,
    /// Destination node ID.
    pub dst: usize,
    /// Description of the edge.
    pub desc: String,
}

impl From<scene_graph::Edge> for PyEdge {
    fn from(e: scene_graph::Edge) -> Self {
        PyEdge {
            src: e.src,
            dst: e.dst,
            desc: e.desc,
        }
    }
}

impl From<&scene_graph::Edge> for PyEdge {
    fn from(e: &scene_graph::Edge) -> Self {
        PyEdge {
            src: e.src,
            dst: e.dst,
            desc: e.desc.clone(),
        }
    }
}

impl From<PyEdge> for scene_graph::Edge {
    fn from(e: PyEdge) -> Self {
        scene_graph::Edge::new(e.src, e.dst, &e.desc)
    }
}

/// A feature associated with a node, represented as a key-value pair.
#[derive(Debug, Clone, Eq, PartialEq)]
#[pyclass(name = "Feature")]
pub struct PyFeature {
    /// Key of the feature.
    key: String,
    /// Value of the feature.
    value: String,
}

#[pymethods]
impl PyFeature {
    #[new]
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }
}

impl From<PyFeature> for scene_graph::Feature {
    fn from(f: PyFeature) -> Self {
        scene_graph::Feature::new(&f.key, &f.value)
    }
}

impl From<&PyFeature> for scene_graph::Feature {
    fn from(f: &PyFeature) -> Self {
        scene_graph::Feature::new(&f.key, &f.value)
    }
}

impl From<scene_graph::Feature> for PyFeature {
    fn from(f: scene_graph::Feature) -> Self {
        PyFeature {
            key: f.key().to_string(),
            value: f.value().to_string(),
        }
    }
}

impl From<&scene_graph::Feature> for PyFeature {
    fn from(f: &scene_graph::Feature) -> Self {
        PyFeature {
            key: f.key().to_string(),
            value: f.value().to_string(),
        }
    }
}
