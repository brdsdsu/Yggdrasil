use std::sync::{Arc, Mutex};

use crate::node::{PyEdge, PyNode};

use super::error::Result;
use pyo3::prelude::*;
use scene_graph::{Layer, Node, Observer, PosQueryCount, SceneGraph};

#[pyclass]
pub struct PyLayer {
    sg: Arc<Mutex<SceneGraph>>,
    lid: usize,
}

/// Node Access and Modification
#[pymethods]
impl PyLayer {
    /// Get a reference to a node by its ID.
    pub fn node(&self, id: usize) -> Result<PyNode> {
        Ok(PyNode::new(Arc::clone(&self.sg), id))
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
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)?
            .add_edge(src, dst, desc)?;
        Ok(())
    }

    /// Delete an edge from source node to destination node.
    /// Returns an error if the edge does not exist.
    pub fn del_edge(&mut self, src: usize, dst: usize) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)?
            .del_edge(src, dst)?;
        Ok(())
    }

    /// Get List of all nodes matching a specific node features.
    pub fn nodes_having(&self, keys: Vec<String>) -> Result<Vec<PyNode>> {
        let keys = keys.iter().map(|k| k.as_str()).collect::<Vec<&str>>();
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .nodes_having(&keys)
            .into_iter()
            .map(|n| PyNode::new(Arc::clone(&self.sg), n.id))
            .collect())
    }

    /// Get List of all nodes matching a specific node features.
    pub fn nodes_matching(&self, features: Vec<super::node::Feature>) -> Result<Vec<PyNode>> {
        let features = features
            .iter()
            .map(|f| f.into())
            .collect::<Vec<scene_graph::Feature>>();
        let features = features.iter().collect::<Vec<&scene_graph::Feature>>();
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .nodes_matching(&features)
            .into_iter()
            .map(|n| PyNode::new(Arc::clone(&self.sg), n.id))
            .collect())
    }

    /// Get List of all edges matching a specific description.
    pub fn edges_matching(&self, desc: &str) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .edges_matching(desc)
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Get List of all edges from a specific source node.
    pub fn edges_from(&self, src: usize) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .edges_from(src)
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: usize) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .edges_to(dst)
            .into_iter()
            .map(Into::into)
            .collect())
    }
}
