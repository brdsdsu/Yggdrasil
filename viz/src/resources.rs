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

/// One shared `StandardMaterial` per layer, keyed by layer id.
///
/// All nodes in the same layer share this material so there is O(n_layers) GPU
/// bind-group allocations instead of O(n_nodes).
#[derive(Resource, Default)]
pub struct LayerMaterials(pub HashMap<u32, Handle<StandardMaterial>>);

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

/// Holds `StandardMaterial` handles for the static edge and nesting line meshes.
/// Stored here so visibility-update systems can look up handles without an extra
/// ECS query into `Assets<StandardMaterial>`.
#[derive(Resource, Default)]
pub struct EdgeMeshMaterials {
    /// Base (full-graph) material per layer, keyed by layer id.
    pub base_by_layer: HashMap<u32, Handle<StandardMaterial>>,
    /// Highlight overlay material per layer, keyed by layer id.
    pub highlight_by_layer: HashMap<u32, Handle<StandardMaterial>>,
    /// Base material for the global nesting line mesh.
    pub nesting_base: Option<Handle<StandardMaterial>>,
    /// Highlight material for the global nesting highlight mesh.
    pub nesting_highlight: Option<Handle<StandardMaterial>>,
}

/// Holds the single shared sphere mesh handle so the radius slider can update
/// all node spheres at once by replacing the mesh asset in-place.
#[derive(Resource, Default)]
pub struct SphereMeshHandle(pub Handle<Mesh>);

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
    /// Radius of every node sphere.
    /// Changing this replaces the shared sphere mesh asset in-place.
    pub node_radius: f32,
    /// When false, the nesting (parent→child) lines are hidden entirely.
    pub show_nesting: bool,
}

impl Default for VisibilityToggles {
    fn default() -> Self {
        Self {
            layer_visible: HashMap::new(),
            show_aabbs: false,
            z_separation: 1.0,
            node_radius: 0.1,
            show_nesting: true,
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

impl QueryType {
    /// Returns true if this query type can be scoped to a selected node's subtree.
    ///
    /// Only `NearestNode` and `VisibleSubgraph` support node-level execution.
    /// All other query types operate at graph or layer granularity only.
    pub fn supports_node_scope(&self) -> bool {
        matches!(self, QueryType::NearestNode | QueryType::VisibleSubgraph)
    }
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
