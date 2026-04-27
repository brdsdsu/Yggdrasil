use kiddo::float::kdtree::KdTree;
use serde::{Deserialize, Serialize};

use crate::BoundingBox;

use super::NodeId;

use super::error::{Result, SceneGraphError};

/// A node in the scene graph.
/// Each node is designated to a unique layer in the scene graph and within that layer, it can have
/// multiple edges to other nodes in the same layer. Nodes can also have parent-child relationships
/// with nodes in the layers directly above or below them.
/// Each node can hold a set of features, which are key-value pairs that provide additional
/// information about the node. Nodes also support storeing 3D coordinates which can be used for
/// Field-of-View calculations or spatial queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// Unique identifier for the node which also holds the layer information.
    pub id: NodeId,
    /// Parent node Id, if node is nested under another node.
    pub pid: Option<NodeId>,
    /// Child node Ids from the lower layer, if node has nested nodes under it.
    children: Vec<NodeId>,
    /// Edges to other nodes in the same layer.
    pub edges: Vec<Edge>,
    /// Features associated with the node.
    pub features: Vec<Feature>,
    /// Optional 3D coordinates of the node.
    pub coordinates: Option<Coordinate>,
    /// Position index which keeps track of the coordinates of the sub tree of this node.
    /// This is used for efficient spatial queries.
    #[serde(skip)]
    pub(crate) pos_index: Option<KdTree<f32, NodeId, 3, 128, u32>>,
    /// Optional bounding box for the node, which can be used for spatial queries and optimizations.
    pub bounds: Option<BoundingBox>,
}

impl Node {
    /// Create a new Node with the given id, features, and optional coordinates.
    pub fn new(
        id: NodeId,
        features: Vec<Feature>,
        coordinates: Option<Coordinate>,
        pos_index_enabled: bool,
        bounds: Option<BoundingBox>,
    ) -> Self {
        let pos_index = if pos_index_enabled {
            Some(KdTree::new())
        } else {
            None
        };
        Self {
            id,
            pid: None,
            children: Vec::new(),
            edges: Vec::new(),
            features,
            coordinates,
            pos_index,
            bounds,
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
            .ok_or_else(|| SceneGraphError::FeatureNotFound(key.to_string()))
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

    /// Get the node IDs of this node's children.
    pub fn children(&self) -> &[NodeId] {
        &self.children
    }
}

impl Node {
    /// Remote a child node by its ID.
    pub(super) fn remove_child(&mut self, nid: NodeId) -> Result<()> {
        let index = self
            .children
            .iter()
            .position(|&id| id == nid)
            .ok_or(SceneGraphError::NodeNotFound)?;
        self.children.swap_remove(index);
        Ok(())
    }

    /// Add a child node by its ID.
    pub(super) fn add_child(&mut self, nid: NodeId) {
        match self.children.binary_search(&nid) {
            Ok(_) => (),
            Err(ptr) => self.children.insert(ptr, nid),
        }
        // if !self.children.contains(&nid) {
        //     self.children.push(nid);
        // }
    }
}

/// 3D Coordinate type for representing spacial positions.
/// The coordinate system is right-handed with Y-up convention.
pub type Coordinate = glam::Vec3;

/// A feature associated with a node, represented as a key-value pair.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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

    #[inline]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[inline]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// An edge connecting two nodes in the same layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// Source node ID.
    pub src: NodeId,
    /// Destination node ID.
    pub dst: NodeId,
    /// Description of the edge.
    pub desc: String,
}

impl Edge {
    pub fn new(src: NodeId, dst: NodeId, desc: &str) -> Self {
        Self {
            src,
            dst,
            desc: desc.to_string(),
        }
    }
}

/// A node in the scene graph.
/// Each node is designated to a unique layer in the scene graph and within that layer, it can have
/// multiple edges to other nodes in the same layer. Nodes can also have parent-child relationships
/// with nodes in the layers directly above or below them.
/// Each node can hold a set of features, which are key-value pairs that provide additional
/// information about the node. Nodes also support storeing 3D coordinates which can be used for
/// Field-of-View calculations or spatial queries.
///
/// Nodes Id will only be available after it is pushed into a layer in the scene graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonNode {
    /// Unique identifier for the node.
    pub(crate) id: u32,
    /// Parent node Id, if node is nested under another node.
    pub pid: Option<NodeId>,
    /// Features associated with the node.
    pub features: Vec<Feature>,
    /// Optional 3D coordinates of the node.
    pub coordinates: Option<Coordinate>,
    /// Position index which keeps track of the coordinates of the sub tree of this node.
    /// This is used for efficient spatial queries.
    #[serde(skip)]
    pub(crate) pos_index: Option<KdTree<f32, NodeId, 3, 128, u32>>,
    /// Optional bounding box for the node, which can be used for spatial queries and optimizations.
    pub bounds: Option<BoundingBox>,
}

impl AnonNode {
    /// Create a new Node with the given id, features, and optional coordinates.
    pub fn new(
        id: u32,
        features: Vec<Feature>,
        coordinates: Option<Coordinate>,
        pos_index_enabled: bool,
        bounds: Option<BoundingBox>,
    ) -> Self {
        let pos_index = if pos_index_enabled {
            Some(KdTree::new())
        } else {
            None
        };
        Self {
            id,
            pid: None,
            features,
            coordinates,
            pos_index,
            bounds,
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
            .ok_or_else(|| SceneGraphError::FeatureNotFound(key.to_string()))
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

pub trait IntoNode {
    fn into_node(self, node_id: NodeId) -> Node;
    fn nid(&self) -> u32;
}

impl IntoNode for AnonNode {
    fn into_node(self, node_id: NodeId) -> Node {
        Node {
            id: node_id,
            pid: self.pid,
            children: Vec::new(),
            edges: Vec::new(),
            features: self.features,
            coordinates: self.coordinates,
            pos_index: self.pos_index,
            bounds: self.bounds,
        }
    }

    fn nid(&self) -> u32 {
        self.id
    }
}
impl IntoNode for Node {
    fn into_node(mut self, _node_id: NodeId) -> Node {
        self.id = _node_id;
        self
    }
    fn nid(&self) -> u32 {
        self.id.node_id()
    }
}
