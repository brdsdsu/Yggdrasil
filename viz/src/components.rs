use bevy::prelude::*;
use scene_graph::NodeId;

/// Marks a mesh entity as a visual sphere representing a scene graph node.
///
/// A node holds a list of coordinates, so it is represented by one sphere per
/// coordinate — every one of them carrying the same `node_id`. That shared id is
/// what lets hover, selection, query highlighting and layer visibility treat a
/// node's spheres as a single unit without any per-coordinate logic.
#[derive(Component)]
pub struct NodeSphere {
    /// The ID of the scene graph node this sphere represents.
    pub node_id: NodeId,
}

/// Which coordinate of its node a [`NodeSphere`] entity stands for, as an index
/// into `Node::coordinates`.
///
/// Nodes with no coordinates get a single sphere at their anchor, indexed `0`.
#[derive(Component)]
pub struct CoordIndex(pub usize);

/// Tags an entity with the layer it belongs to, used for visibility toggling and color coding.
#[derive(Component)]
pub struct LayerTag {
    /// The ID of the layer this entity belongs to.
    pub layer_id: u32,
}

/// The computed 3D position assigned to a sphere in visualization space.
///
/// This differs from the node's raw coordinate: it incorporates the layer z-offset
/// so that higher layers appear visually above lower ones.
#[derive(Component)]
pub struct VisPos(pub Vec3);

/// Marks the line-list mesh holding one layer's coordinate-group spokes — the thin
/// lines binding each coordinate of a multi-coordinate node to that node's anchor.
///
/// These are not graph edges. They connect a node to itself and exist only to make
/// multi-coordinate nodes legible, so they are styled subordinate to real edges and
/// gated behind `VisibilityToggles::show_coord_groups`.
#[derive(Component)]
pub struct CoordGroupMesh {
    /// The ID of the layer whose nodes' spokes this mesh holds.
    pub layer_id: u32,
}


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
