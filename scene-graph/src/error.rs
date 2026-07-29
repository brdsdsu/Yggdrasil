use thiserror::Error;

pub type Result<T, E = SceneGraphError> = core::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum SceneGraphError {
    #[error("edge not found")]
    EdgeNotFound,

    #[error("node not found")]
    NodeNotFound,

    #[error("point not found")]
    PointNotFound,

    #[error("layer not found")]
    LayerNotFound,

    #[error("invalid layers for nesting: cannot nest layer {0} within layer {1}")]
    InvalidLayersForNesting(u32, u32),

    #[error("feature '{0}' not found")]
    FeatureNotFound(String),

    #[error("coordinates are required for this layer")]
    CoordinatesRequired,


    /// The observer was given a direction that is not a viewing axis.
    ///
    /// A field of view is a cone about its direction, so one that cannot be
    /// normalized, i.e. having zero-length or carrying a NaN or infinity, leaves the cone
    /// with nothing to point along.
    #[error("observer direction has no viewing axis")]
    ObserverCreationError,

    #[error("failed to parse scene graph: {0}")]
    ParseError(String),
}
