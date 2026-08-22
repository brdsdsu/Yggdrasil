use std::collections::HashMap;

use pyo3::prelude::*;
use scene_graph::NodeId;

use crate::node::PyNodeId;

/// Result of a single-source shortest-path run over one layer.
///
/// Owns its data (ids and distances) rather than holding a handle into the
/// graph, so it stays valid and lock-free no matter what happens to the graph
/// afterwards. Settled nodes are stored in settle order.
#[pyclass(name = "ShortestPaths")]
pub struct PyShortestPaths {
    source: NodeId,
    dist: Vec<(NodeId, f64)>,
    paths: Vec<Vec<NodeId>>,
    index: HashMap<NodeId, usize>,
}

impl PyShortestPaths {
    pub fn new(sp: scene_graph::ShortestPaths) -> Self {
        let index = sp
            .dist
            .iter()
            .enumerate()
            .map(|(i, (id, _))| (*id, i))
            .collect();
        Self {
            source: sp.source,
            dist: sp.dist,
            paths: sp.paths,
            index,
        }
    }
}

#[pymethods]
impl PyShortestPaths {
    /// The source node this run started from.
    #[getter]
    pub fn source(&self) -> PyNodeId {
        self.source.into()
    }

    /// All settled nodes as `(node_id, distance, path)` tuples, in settle
    /// order (equal distances keep first-found-first order).
    pub fn items(&self) -> Vec<(PyNodeId, f64, Vec<PyNodeId>)> {
        self.dist
            .iter()
            .zip(&self.paths)
            .map(|((id, d), path)| {
                (
                    (*id).into(),
                    *d,
                    path.iter().map(|n| (*n).into()).collect(),
                )
            })
            .collect()
    }

    /// Distance to `target`, or None if it was not settled (unreachable).
    pub fn distance_to(&self, target: PyNodeId) -> Option<f64> {
        self.index
            .get(&NodeId::from(target))
            .map(|i| self.dist[*i].1)
    }

    /// Full path `source ..= target`, or None if `target` was not settled.
    pub fn path_to(&self, target: PyNodeId) -> Option<Vec<PyNodeId>> {
        self.index
            .get(&NodeId::from(target))
            .map(|i| self.paths[*i].iter().map(|n| (*n).into()).collect())
    }

    /// True if `target` was settled (reachable and reached).
    pub fn __contains__(&self, target: PyNodeId) -> bool {
        self.index.contains_key(&NodeId::from(target))
    }

    pub fn __len__(&self) -> usize {
        self.dist.len()
    }
}
