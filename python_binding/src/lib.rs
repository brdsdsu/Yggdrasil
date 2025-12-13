use pyo3::prelude::*;
mod error;
mod layer;
mod node;
mod sg;

/// A Python module implemented in Rust.
#[pymodule]
mod python_binding {
    #[pymodule_export]
    use crate::layer::{PyLayerRef, PyObserver, PyPosQueryCount};
    #[pymodule_export]
    use crate::node::{PyEdge, PyFeature, PyNode, PyNodeRef};
    #[pymodule_export]
    use crate::sg::PySceneGraphRef;
}
