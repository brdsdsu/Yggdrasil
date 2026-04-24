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

/// Drives the per-frame alpha lerp for the hover fade effect.
///
/// Each frame, `current` is lerped toward `target` and written into the node
/// sphere's `StandardMaterial` base color alpha.
#[derive(Component)]
pub struct Opacity {
    /// The alpha value being lerped toward (0.0 = fully transparent, 1.0 = fully opaque).
    pub target: f32,
    /// The current alpha value applied to the material this frame.
    pub current: f32,
}

/// Marker placed on entities that match the current query results, used by the
/// highlight system to recolor them and dim everything else.
#[derive(Component)]
pub struct QueryHighlight;
