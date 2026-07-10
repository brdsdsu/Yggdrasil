use std::sync::{Arc, Mutex};

use scene_graph::{NodeId, SceneGraph};

use crate::node::PyNodeRef;

use pyo3::prelude::*;

#[pyclass(name = "__NodeList", sequence)]
pub struct PyLazyNodeList {
    inner: Vec<NodeId>,
    scene_graph: Arc<Mutex<SceneGraph>>,
}

#[pymethods]
impl PyLazyNodeList {
    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.inner.len())
    }

    pub fn __getitem__(&self, index: isize) -> PyResult<PyNodeRef> {
        Ok(PyNodeRef::new(
            Arc::clone(&self.scene_graph),
            self.inner[index as usize],
        ))
    }

    pub fn __setitem__(&mut self, index: isize, value: PyNodeRef) -> PyResult<()> {
        self.inner[index as usize] = value.nid;
        Ok(())
    }

    pub fn __delitem__(&mut self, index: isize) -> PyResult<()> {
        self.inner.remove(index as usize);
        Ok(())
    }
}

#[pyclass(name = "__NodeDistPairList", sequence)]
pub struct PyLazyNodeDistPairList {
    inner: Vec<(NodeId, f32)>,
    scene_graph: Arc<Mutex<SceneGraph>>,
}

impl PyLazyNodeDistPairList {
    pub(crate) fn new(scene_graph: Arc<Mutex<SceneGraph>>, inner: Vec<(NodeId, f32)>) -> Self {
        Self { inner, scene_graph }
    }
}

#[pymethods]
impl PyLazyNodeDistPairList {
    pub fn __len__(&self) -> PyResult<usize> {
        Ok(self.inner.len())
    }

    pub fn __getitem__(&self, index: isize) -> PyResult<(PyNodeRef, f32)> {
        Ok((
            PyNodeRef::new(Arc::clone(&self.scene_graph), self.inner[index as usize].0),
            self.inner[index as usize].1,
        ))
    }

    pub fn __setitem__(&mut self, index: isize, value: (PyNodeRef, f32)) -> PyResult<()> {
        self.inner[index as usize] = (value.0.nid, value.1);
        Ok(())
    }

    pub fn __delitem__(&mut self, index: isize) -> PyResult<()> {
        self.inner.remove(index as usize);
        Ok(())
    }
}
