use bevy::prelude::*;
use scene_graph::NodeId;

/// Marks a mesh entity as the visual sphere representing a single scene graph node.
#[derive(Component)]
pub struct NodeSphere {
    /// The ID of the scene graph node this sphere represents.
    pub node_id: NodeId,
}

/// Tags an entity with the layer it belongs to, used for visibility toggling and color coding.
#[derive(Component)]
pub struct LayerTag {
    /// The ID of the layer this entity belongs to.
    pub layer_id: u32,
}

/// The computed 3D position assigned to a node in visualization space.
///
/// This differs from the node's raw coordinate: it incorporates the layer z-offset
/// so that higher layers appear visually above lower ones.
#[derive(Component)]
pub struct VisPos(pub Vec3);


/// Marks the base line-list mesh entity for one layer's intra-layer edges.
/// Visible when no hover/selection/query state is active.
#[derive(Component)]
pub struct EdgeBaseMesh {
    pub layer_id: u32,
}

/// Marks the highlight overlay line-list mesh for one layer's intra-layer edges.
/// Visible (and rebuilt) only when hover/selection/query state is active.
#[derive(Component)]
pub struct EdgeHighlightMesh {
    pub layer_id: u32,
}

/// Marks the single global line-list mesh for all parent→child nesting links.
/// Visible when no state is active.
#[derive(Component)]
pub struct NestingBaseMesh;

/// Marks the highlight overlay for nesting lines.
/// Rebuilt with only active-set links when hover/selection/query state is active.
#[derive(Component)]
pub struct NestingHighlightMesh;
