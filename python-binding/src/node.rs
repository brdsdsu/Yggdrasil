use std::sync::{Arc, Mutex};

use super::error::Result;
use pyo3::prelude::*;
use scene_graph::SceneGraph;

/// A node in the scene graph.
/// Each node is designated to a unique layer in the scene graph and within that layer, it can have
/// multiple edges to other nodes in the same layer. Nodes can also have parent-child relationships
/// with nodes in the layers directly above or below them.
/// Each node can hold a set of features, which are key-value pairs that provide additional
/// information about the node. Nodes also support storeing 3D coordinates which can be used for
/// Field-of-View calculations or spatial queries.
#[pyclass]
pub struct PyNode {
    sg: Arc<Mutex<SceneGraph>>,
    nid: usize,
}

impl PyNode {
    pub fn new(sg: Arc<Mutex<SceneGraph>>, nid: usize) -> Self {
        Self { sg, nid }
    }
}

#[pymethods]
impl PyNode {
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
    pub fn features(&self) -> Result<Vec<Feature>> {
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
        let coords = match coords {
            Some((x, y, z)) => Some(scene_graph::Coordinate::new(x, y, z)),
            None => None,
        };
        self.sg.lock().unwrap().node_mut(self.nid)?.coordinates = coords;
        Ok(())
    }

    /// Check if the node has a feature with the specified key.
    pub fn has_feature(&self, key: &str) -> Result<bool> {
        Ok(self.sg.lock().unwrap().node(self.nid)?.has_feature(key))
    }

    /// Check if the node has the exact key-value pair as a feature.
    pub fn match_feature(&self, f: &Feature) -> Result<bool> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .node(self.nid)?
            .match_feature(&f.into()))
    }

    /// Get the value of a feature by its key.
    pub fn feature(&self, key: &str) -> Result<String> {
        let f = self.sg.lock().unwrap().node(self.nid)?.feature(key)?;
        Ok(String::from(f))
    }

    /// Set or update a feature for the node.
    pub fn set_feature(&mut self, feature: Feature) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .node_mut(self.nid)?
            .set_feature(feature.into());
        Ok(())
    }
}

#[pyclass]
/// An edge connecting two nodes in the same layer.
#[derive(Debug, Clone)]
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
#[pyclass]
pub struct Feature {
    /// Key of the feature.
    key: String,
    /// Value of the feature.
    value: String,
}

impl From<Feature> for scene_graph::Feature {
    fn from(f: Feature) -> Self {
        scene_graph::Feature::new(&f.key, &f.value)
    }
}

impl From<&Feature> for scene_graph::Feature {
    fn from(f: &Feature) -> Self {
        scene_graph::Feature::new(&f.key, &f.value)
    }
}

impl From<scene_graph::Feature> for Feature {
    fn from(f: scene_graph::Feature) -> Self {
        Feature {
            key: f.key().to_string(),
            value: f.value().to_string(),
        }
    }
}

impl From<&scene_graph::Feature> for Feature {
    fn from(f: &scene_graph::Feature) -> Self {
        Feature {
            key: f.key().to_string(),
            value: f.value().to_string(),
        }
    }
}
