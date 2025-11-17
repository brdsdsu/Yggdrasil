use thiserror::Error;

pub type Result<T, E = AtlasError> = core::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum AtlasError {
    #[error("edge not found")]
    EdgeNotFound,

    #[error("node not found")]
    NodeNotFound,

    #[error("point not found")]
    PointNotFound,

    #[error("layer index out of bounds: {0} is not within (0, {1})")]
    LayerOutOfBounds(usize, usize),

    #[error("invalid layers for nesting: cannot nest layer {0} within layer {1}")]
    InvalidLayersForNesting(usize,usize),

    #[error("feature '{0}' not found")]
    FeatureNotFound(String),

    #[error("coordinates are required for this layer")]
    CoordinatesRequired,
}
