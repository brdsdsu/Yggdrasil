use std::sync::{Arc, Mutex};

use scene_graph::{NodeId, SceneGraph};

use crate::node::PyNodeRef;

use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;

/// Resolve a Python-style index (negative counts from the end) against `len`,
/// raising IndexError when out of range. Raising (not panicking) matters
/// beyond correctness: Python's sequence iteration protocol probes successive
/// indices and relies on IndexError to terminate the loop.
fn resolve_index(index: isize, len: usize) -> PyResult<usize> {
    let idx = if index < 0 {
        index + len as isize
    } else {
        index
    };
    if idx < 0 || idx as usize >= len {
        Err(PyIndexError::new_err("index out of range"))
    } else {
        Ok(idx as usize)
    }
}

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
        let idx = resolve_index(index, self.inner.len())?;
        Ok(PyNodeRef::new(
            Arc::clone(&self.scene_graph),
            self.inner[idx],
        ))
    }

    pub fn __setitem__(&mut self, index: isize, value: PyNodeRef) -> PyResult<()> {
        let idx = resolve_index(index, self.inner.len())?;
        self.inner[idx] = value.nid;
        Ok(())
    }

    pub fn __delitem__(&mut self, index: isize) -> PyResult<()> {
        let idx = resolve_index(index, self.inner.len())?;
        self.inner.remove(idx);
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
        let idx = resolve_index(index, self.inner.len())?;
        Ok((
            PyNodeRef::new(Arc::clone(&self.scene_graph), self.inner[idx].0),
            self.inner[idx].1,
        ))
    }

    pub fn __setitem__(&mut self, index: isize, value: (PyNodeRef, f32)) -> PyResult<()> {
        let idx = resolve_index(index, self.inner.len())?;
        self.inner[idx] = (value.0.nid, value.1);
        Ok(())
    }

    pub fn __delitem__(&mut self, index: isize) -> PyResult<()> {
        let idx = resolve_index(index, self.inner.len())?;
        self.inner.remove(idx);
        Ok(())
    }
}
