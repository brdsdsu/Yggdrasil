use crate::error::{AtlasError, Result};

/// A node in the scene graph.
/// Each node is designated to a unique layer in the scene graph and within that layer, it can have
/// multiple edges to other nodes in the same layer. Nodes can also have parent-child relationships
/// with nodes in the layers directly above or below them.
/// Each node can hold a set of features, which are key-value pairs that provide additional
/// information about the node. Nodes also support storeing 3D coordinates which can be used for
/// Field-of-View calculations or spatial queries.
#[derive(Debug, Clone)]
pub struct Node {
    /// Unique identifier for the node.
    pub id: usize,
    /// Parent node Id, if node is nested under another node.
    pub(super) pid: Option<usize>,
    /// Child node Ids from the lower layer, if node has nested nodes under it.
    pub(super) children: Vec<usize>,
    /// Edges to other nodes in the same layer.
    pub edges: Vec<Edge>,
    /// Features associated with the node.
    pub features: Vec<Feature>,
    /// Optional 3D coordinates of the node.
    pub coordinates: Option<Coordinate>,
}

impl Node {
    /// Create a new Node with the given id, features, and optional coordinates.
    pub fn new(id: usize, features: Vec<Feature>, coordinates: Option<Coordinate>) -> Self {
        Self {
            id,
            pid: None,
            children: Vec::new(),
            edges: Vec::new(),
            features,
            coordinates,
        }
    }
    /// Check if the node has a feature with the specified key.
    pub fn has_feature(&self, key: &str) -> bool {
        self.features.iter().any(|f| f.key == key)
    }

    /// Check if the node has the exact key-value pair as a feature.
    pub fn match_feature(&self, f: &Feature) -> bool {
        self.features.contains(f)
    }

    /// Get the value of a feature by its key.
    pub fn feature(&self, key: &str) -> Result<&str> {
        self.features
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.as_str())
            .ok_or_else(|| AtlasError::FeatureNotFound(key.to_string()))
    }

    /// Set or update a feature for the node.
    pub fn set_feature(&mut self, feature: Feature) {
        if !self.has_feature(&feature.key) {
            self.features.push(feature);
        } else {
            for f in &mut self.features {
                if f.key == feature.key {
                    f.value = feature.value;
                    break;
                }
            }
        }
    }
}

impl Node {
    /// Remote a child node by its ID.
    pub(super) fn remove_child(&mut self, nid: usize) -> Result<()> {
        let index = self
            .children
            .iter()
            .position(|&id| id == nid)
            .ok_or(AtlasError::NodeNotFound)?;
        self.children.swap_remove(index);
        Ok(())
    }

    /// Add a child node by its ID.
    pub(super) fn add_child(&mut self, nid: usize) {
        if !self.children.contains(&nid) {
            self.children.push(nid);
        }
    }
}

/// 3D Coordinate type for representing spacial positions.
/// The coordinate system is right-handed with Y-up convention.
pub type Coordinate = glam::Vec3;

/// A feature associated with a node, represented as a key-value pair.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Feature {
    /// Key of the feature.
    key: String,
    /// Value of the feature.
    value: String,
}

impl Feature {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
        }
    }
}

/// An edge connecting two nodes in the same layer.
#[derive(Debug, Clone)]
pub struct Edge {
    /// Source node ID.
    pub src: usize,
    /// Destination node ID.
    pub dst: usize,
    /// Description of the edge.
    pub desc: String,
}

impl Edge {
    pub fn new(src: usize, dst: usize, desc: &str) -> Self {
        Self {
            src,
            dst,
            desc: desc.to_string(),
        }
    }
}
