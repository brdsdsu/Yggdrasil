use pyo3::{PyErr, exceptions::PyValueError};

use scene_graph::SceneGraphError as SGError;

pub struct ErrorWrapper(SGError);

pub type Result<T> = std::result::Result<T, ErrorWrapper>;

impl From<ErrorWrapper> for PyErr {
    fn from(err: ErrorWrapper) -> PyErr {
        PyValueError::new_err(err.0.to_string())
    }
}

impl From<SGError> for ErrorWrapper {
    fn from(err: SGError) -> Self {
        ErrorWrapper(err)
    }
}
