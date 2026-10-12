use crate::error::Result;
use crate::lazylist::PyLazyNodeDistPairList;
use crate::node::{PyAnonNode, PyEdge, PyNodeId, PyNodeRef};
use crate::shortest_path::PyShortestPaths;
use glam::Vec3;

use pyo3::prelude::*;
use scene_graph::{Coordinate, SceneGraph};
use std::sync::{Arc, Mutex};

#[pyclass(name = "Layer")]
pub struct PyLayerRef {
    sg: Arc<Mutex<SceneGraph>>,

    #[pyo3(get)]
    lid: u32,
}

impl PyLayerRef {
    pub fn new(sg: Arc<Mutex<SceneGraph>>, lid: u32) -> Self {
        Self { sg, lid }
    }
}

/// Node Access and Modification
#[pymethods]
impl PyLayerRef {
    /// Get a reference to a node by its ID.
    pub fn node(&self, id: PyNodeId) -> Result<PyNodeRef> {
        Ok(PyNodeRef::new(Arc::clone(&self.sg), id.into()))
    }

    /// Add a new node to the layer.
    pub fn push_node(&mut self, node: PyAnonNode) -> Result<PyNodeId> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)
            .unwrap()
            .push_node(node.into_inner())?
            .into())
    }

    /// Add an edge from source node to destination node with a description.
    /// Ensures both source and destination nodes exist in the layer.
    pub fn add_edge(&mut self, src: PyNodeId, dst: PyNodeId, desc: &str) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)?
            .add_edge(src.into(), dst.into(), desc)?;
        Ok(())
    }

    /// Add a child layer by its ID.
    pub fn add_child(&mut self, child_id: u32) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)?
            .add_child(child_id)?;
        Ok(())
    }

    /// Delete an edge from source node to destination node.
    /// Returns an error if the edge does not exist.
    pub fn del_edge(&mut self, src: PyNodeId, dst: PyNodeId) -> Result<()> {
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)?
            .del_edge(src.into(), dst.into())?;
        Ok(())
    }

    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nearest_nodes(
        &self,
        coord: (f32, f32, f32),
        count: PyPosQueryCount,
    ) -> Result<PyLazyNodeDistPairList> {
        let (x, y, z) = coord;
        let nodes = self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .nearest_nodes(&Coordinate::new(x, y, z), count.into());
        Ok(PyLazyNodeDistPairList::new(Arc::clone(&self.sg), nodes))
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nodes_within(
        &self,
        coord: (f32, f32, f32),
        radius: f32,
        count: PyPosQueryCount,
    ) -> Result<PyLazyNodeDistPairList> {
        let (x, y, z) = coord;
        let nodes = self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .nodes_within(&Coordinate::new(x, y, z), radius, count.into());
        Ok(PyLazyNodeDistPairList::new(Arc::clone(&self.sg), nodes))
    }

    /// Single-source shortest paths over this layer's intra-layer edges,
    /// following edges in their insertion order. The weight of an edge is the
    /// Euclidean distance between the first coordinate of its endpoints.
    /// Edges are directed: insert both directions for undirected semantics.
    pub fn dijkstra(&self, source: PyNodeId) -> Result<PyShortestPaths> {
        let sg = self.sg.lock().unwrap();
        let sp = sg.layer(self.lid)?.dijkstra(source.into())?;
        Ok(PyShortestPaths::new(sp))
    }

    /// Same as `dijkstra`, but stops as soon as every node in `targets` has
    /// been settled. Unreachable targets are absent from the result.
    pub fn dijkstra_to(
        &self,
        source: PyNodeId,
        targets: Vec<PyNodeId>,
    ) -> Result<PyShortestPaths> {
        let targets: Vec<scene_graph::NodeId> = targets.into_iter().map(Into::into).collect();
        let sg = self.sg.lock().unwrap();
        let sp = sg.layer(self.lid)?.dijkstra_to(source.into(), &targets)?;
        Ok(PyShortestPaths::new(sp))
    }

    /// Get List of all nodes in the layer.
    pub fn nodes(&self) -> Result<Vec<PyNodeRef>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .all_nodes()
            .map(|n| PyNodeRef::new(Arc::clone(&self.sg), n.id))
            .collect())
    }

    /// Get List of all nodes having a specific node features.
    pub fn nodes_having(&self, keys: Vec<String>) -> Result<Vec<PyNodeRef>> {
        let keys = keys.iter().map(|k| k.as_str()).collect::<Vec<&str>>();
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .nodes_having(&keys)
            .into_iter()
            .map(|n| PyNodeRef::new(Arc::clone(&self.sg), n.id))
            .collect())
    }

    /// Get List of all nodes matching a specific node features.
    pub fn nodes_matching(&self, features: Vec<super::node::PyFeature>) -> Result<Vec<PyNodeRef>> {
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
            .map(|n| PyNodeRef::new(Arc::clone(&self.sg), n.id))
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
    pub fn edges_from(&self, src: PyNodeId) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .edges_from(src.into())
            .into_iter()
            .map(Into::into)
            .collect())
    }

    /// Get List of all edges to a specific destination node.
    pub fn edges_to(&self, dst: PyNodeId) -> Result<Vec<PyEdge>> {
        Ok(self
            .sg
            .lock()
            .unwrap()
            .layer(self.lid)?
            .edges_to(dst.into())
            .into_iter()
            .map(Into::into)
            .collect())
    }
}

/// Specifies the number of nodes to return in position-based queries.
#[derive(Debug, Clone, Copy)]
#[pyclass(name = "PosQueryCount")]
pub enum PyPosQueryCount {
    /// Return All matching nodes.
    All(),
    /// Return a specific Count of matching nodes.
    Count(usize),
}

impl From<PyPosQueryCount> for scene_graph::PosQueryCount {
    fn from(pqc: PyPosQueryCount) -> Self {
        match pqc {
            PyPosQueryCount::All() => scene_graph::PosQueryCount::All,
            PyPosQueryCount::Count(c) => scene_graph::PosQueryCount::Count(c),
        }
    }
}

/// Observer represent a Field-of-View cone frustum in 3D space.
/// The cone is defined by a position, a viewing direction,
/// and a half-angle (in radians). The frustum is further limited
/// by near and far distances.
///
///  ↑↑            O <---------- observer position
///  ||           /|\
///  ||          / | \
///  ||near     /  |  \
///  ||        /   |-θ-\ <------ half_angle
///  ||       /         \
///  |↓      *-----------* <---- Chord with `near` radius
///  |      / observable  \
///  |far  /    volume     \
///  ↓    *-----------------* <- Chord with `far` radius
///
#[derive(Clone, Copy, Debug)]
#[pyclass(name = "Observer")]
pub struct PyObserver {
    /// The validated cone, built once at construction so a bad direction is
    /// reported where Python can still point at the offending arguments.
    inner: scene_graph::Observer,
}

#[pymethods]
impl PyObserver {
    /// Build an observer at `position` looking along `direction`.
    ///
    /// `direction` is an (x, y, z) vector in the same frame as the scene's own
    /// coordinates; only its axis matters, so it need not be a unit vector. A
    /// zero-length or non-finite direction raises `ValueError`, since neither gives
    /// the cone an axis to point along.
    ///
    /// `half_angle` is in radians and measures from the direction outwards, so the
    /// full opening of the cone is twice it.
    #[new]
    pub fn new(
        position: (f32, f32, f32),
        direction: (f32, f32, f32),
        half_angle: f32,
        near: f32,
        far: f32,
    ) -> Result<Self> {
        let (px, py, pz) = position;
        let (dx, dy, dz) = direction;
        let inner = scene_graph::Observer::new_looking_along(
            Vec3::new(px, py, pz),
            Vec3::new(dx, dy, dz),
            half_angle,
            near,
            far,
        )?;
        Ok(Self { inner })
    }
}

impl From<PyObserver> for scene_graph::Observer {
    fn from(obs: PyObserver) -> Self {
        obs.inner
    }
}
