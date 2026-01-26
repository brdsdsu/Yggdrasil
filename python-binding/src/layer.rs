use crate::error::Result;
use crate::node::{PyEdge, PyNode, PyNodeRef};
use glam::Vec3;

use pyo3::prelude::*;
use scene_graph::{Coordinate, SceneGraph};
use std::sync::{Arc, Mutex};

#[pyclass(name = "Layer")]
pub struct PyLayerRef {
    sg: Arc<Mutex<SceneGraph>>,
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
    pub fn node(&self, id: usize) -> Result<PyNodeRef> {
        Ok(PyNodeRef::new(Arc::clone(&self.sg), id))
    }

    /// Add a new node to the layer.
    pub fn push_node(&mut self, node: PyNode) {
        self.sg
            .lock()
            .unwrap()
            .layer_mut(self.lid)
            .unwrap()
            .push_node(node.into_inner());
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


    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nearest_nodes(&self, coord: (f32, f32, f32), count: PyPosQueryCount) -> Vec<(PyNodeRef, f32)> {
        let (x, y, z) = coord;
        self.sg
            .lock()
            .unwrap()
            .nearest_nodes(&Coordinate::new(x, y, z), count.into())
            .into_iter()
            .map(|(n,d)| (PyNodeRef::new(Arc::clone(&self.sg), n.id),d))
            .collect::<Vec<_>>()
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    pub fn nodes_within(
        &self,
        coord: (f32, f32, f32),
        radius: f32,
        count: PyPosQueryCount,
    ) -> Vec<(PyNodeRef, f32)> {
        let (x, y, z) = coord;
        self.sg
            .lock()
            .unwrap()
            .nodes_within(&Coordinate::new(x, y, z), radius, count.into())
            .into_iter()
            .map(|(n,d)| (PyNodeRef::new(Arc::clone(&self.sg), n.id),d))
            .collect::<Vec<_>>()
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
/// The cone is defined by a position, orientation (quaternion),
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
    /// Position of the observer/camera in world space.
    position: Vec3,

    /// Orientation of the observer/camera as a function of yaw, pitch, roll (radians).
    yaw: f32,
    /// Orientation of the observer/camera as a function of yaw, pitch, roll (radians).
    pitch: f32,
    /// Orientation of the observer/camera as a function of yaw, pitch, roll (radians).
    roll: f32,

    /// `half_angle` represents the maximum angle (in radians) from the forward
    /// direction that is still considered "inside" the field of view.
    half_angle_cos: f32,

    /// Near distance of the frustum. Points closer than this are not
    /// observed by the observer.
    near: f32,

    /// Far distance of the frustum. Points farther than this are not
    /// observed by the observer.
    far: f32,
}

impl From<PyObserver> for scene_graph::Observer {
    fn from(obs: PyObserver) -> Self {
        scene_graph::Observer::from_ypr(
            obs.position,
            obs.yaw,
            obs.pitch,
            obs.roll,
            obs.half_angle_cos,
            obs.near,
            obs.far,
        )
    }
}
