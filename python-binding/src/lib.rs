use pyo3::prelude::*;
mod error;
mod layer;
mod node;
mod sg;
mod lazylist;

/// A Python module implemented in Rust.
#[pymodule]
mod yggdrasil {
    #[pymodule_export]
    use crate::layer::{PyLayerRef, PyObserver, PyPosQueryCount};
    #[pymodule_export]
    use crate::node::{PyAnonNode, PyEdge, PyFeature, PyNodeId, PyNodeRef};
    #[pymodule_export]
    use crate::sg::PySceneGraphRef;
    #[pymodule_export]
    use crate::lazylist::PyLazyNodeList;
}
