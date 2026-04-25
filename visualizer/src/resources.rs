use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use bevy::prelude::*;
use scene_graph::{NodeId, SceneGraph};

/// The loaded scene graph shared across all systems.
///
/// Wrapped in `Arc` so it can be cloned cheaply into the query dispatch system
/// without holding a mutable borrow on the world.
#[derive(Resource)]
pub struct SceneGraphRes(pub Arc<SceneGraph>);

/// Maps every node ID to its computed 3D position in visualization space.
///
/// Populated by the layout algorithm at startup and recomputed whenever the
/// z-separation slider changes.
#[derive(Resource, Default)]
pub struct LayoutPositions(pub HashMap<NodeId, Vec3>);

/// Caches the `StandardMaterial` handle for each node sphere, keyed by node ID.
///
/// Each node gets its own material so the hover/query systems can independently
/// control per-node opacity without affecting other nodes in the same layer.
#[derive(Resource, Default)]
pub struct MaterialHandles(pub HashMap<NodeId, Handle<StandardMaterial>>);

/// Tracks which node or edge the cursor is currently over, and the set of
/// node IDs that should remain fully opaque (the hovered node and all its descendants).
#[derive(Resource, Default)]
pub struct HoverState {
    /// The node ID currently under the cursor, if any.
    pub hovered_node: Option<NodeId>,
    /// The edge (src, dst) currently under the cursor, if any.
    pub hovered_edge: Option<(NodeId, NodeId)>,
    /// The hovered node plus all its descendants, collected via downward BFS.
    /// All nodes outside this set are faded to low opacity while hovering.
    pub subtree_ids: HashSet<NodeId>,
}

/// Tracks the user's active selection, used to scope queries to a specific
/// layer or node rather than the entire scene graph.
#[derive(Resource, Default)]
pub struct SelectionState {
    /// The node ID that was last clicked, if any.
    pub selected_node: Option<NodeId>,
    /// The layer ID derived from the last clicked node, if any.
    pub selected_layer: Option<u32>,
    /// The selected node plus all its descendants, collected via downward BFS.
    /// All nodes outside this set are faded while a selection is active.
    pub subtree_ids: HashSet<NodeId>,
}

/// Controls per-layer visibility and optional overlays.
#[derive(Resource)]
pub struct VisibilityToggles {
    /// Whether each layer's nodes are visible, keyed by layer ID.
    pub layer_visible: HashMap<u32, bool>,
    /// When true, AABB wireframes are drawn for nodes with bounding boxes.
    pub show_aabbs: bool,
    /// Vertical spacing between layers in visualization space.
    /// Recomputing the layout is triggered whenever this value changes.
    pub z_separation: f32,
}

impl Default for VisibilityToggles {
    fn default() -> Self {
        Self {
            layer_visible: HashMap::new(),
            show_aabbs: false,
            z_separation: 1.0,
        }
    }
}

/// All state backing the query panel UI, including form inputs, results, and errors.
#[derive(Resource, Default)]
pub struct QueryPanelState {
    /// The type of query currently selected in the UI.
    pub query_type: QueryType,
    /// The scope at which the query runs (whole graph, a layer, or a subtree).
    pub scope: QueryScope,
    /// Feature key input for NodesByFeature / NodesByKey queries.
    pub feature_key: String,
    /// Feature value input for NodesByFeature queries.
    pub feature_value: String,
    /// Edge label input for EdgesByLabel queries.
    pub edge_label: String,
    /// XYZ coordinate inputs for spatial queries (NearestNode, NodesWithin).
    pub coord: [String; 3],
    /// Radius input for NodesWithin queries.
    pub radius: String,
    /// Result count input for NearestNode queries.
    pub count: String,
    /// When true, return all results instead of a fixed count.
    pub use_all: bool,
    /// Observer position inputs for VisibleSubgraph queries.
    pub fov_pos: [String; 3],
    /// Observer yaw (rotation around Y-axis) for VisibleSubgraph queries.
    pub fov_yaw: String,
    /// Observer pitch (rotation around X-axis) for VisibleSubgraph queries.
    pub fov_pitch: String,
    /// Observer roll (rotation around Z-axis) for VisibleSubgraph queries.
    pub fov_roll: String,
    /// Half-angle of the FOV cone for VisibleSubgraph queries.
    pub fov_half_angle: String,
    /// Near clipping distance for VisibleSubgraph queries.
    pub fov_near: String,
    /// Far clipping distance for VisibleSubgraph queries.
    pub fov_far: String,
    /// Root node ID for VisibleSubgraph / nearest-in-subtree queries.
    pub root_node: String,
    /// Node IDs highlighted as query results in the 3D view.
    pub highlighted_nodes: HashSet<NodeId>,
    /// Edge pairs highlighted as query results in the 3D view.
    pub highlighted_edges: HashSet<(NodeId, NodeId)>,
    /// Error message displayed in the panel when the last query failed.
    pub error: Option<String>,
    /// Set to true by the UI "Run" button; consumed and reset by the dispatch system.
    pub submitted: bool,
}

/// The type of query to run against the scene graph.
#[derive(Default, PartialEq, Clone)]
pub enum QueryType {
    /// Find nodes whose features contain the given key-value pair (`nodes_matching`).
    #[default]
    NodesByFeature,
    /// Find nodes that have a feature with the given key, regardless of value (`nodes_having`).
    NodesByKey,
    /// Find edges whose description matches the given label (`edges_matching`).
    EdgesByLabel,
    /// Find all edges originating from a given node (`edges_from`).
    EdgesFrom,
    /// Find all edges terminating at a given node (`edges_to`).
    EdgesTo,
    /// Find the N nearest coordinate nodes to a given position (`nearest_nodes` / `nearest_in_subtree`).
    NearestNode,
    /// Find all coordinate nodes within a given radius of a position (`nodes_within`).
    NodesWithin,
    /// Find all nodes visible from a given observer position and FOV cone (`visible_subgraph`).
    VisibleSubgraph,
}

/// The scope at which a query is executed.
#[derive(Default, PartialEq, Clone)]
pub enum QueryScope {
    /// Run the query across the entire scene graph.
    #[default]
    Graph,
    /// Run the query within the currently selected layer only.
    Layer,
    /// Run the query within the subtree rooted at the currently selected node.
    Node,
}
