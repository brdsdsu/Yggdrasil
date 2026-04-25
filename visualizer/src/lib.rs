pub mod components;
pub mod layout;
pub mod loader;
pub mod render;
pub mod resources;
pub mod spawn;

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use scene_graph::SceneGraph;

use resources::{
    HoverState, LayoutPositions, MaterialHandles, QueryPanelState, SceneGraphRes, SelectionState,
    VisibilityToggles,
};

/// Launches the visualizer window for the given scene graph.
///
/// This is the library entry point, allowing callers to pass a `SceneGraph`
/// directly without serializing it to JSON. The function blocks until the
/// window is closed.
pub fn run(sg: SceneGraph) {
    let z_sep = layout::compute_z_separation(&sg);
    let positions = layout::compute_layout(&sg, z_sep);

    // Initialize all layers as visible.
    let layer_visible: HashMap<u32, bool> = sg.layers().map(|l| (l.id, true)).collect();

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Yggdrasil Scene Graph Visualizer".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(bevy_egui::EguiPlugin)
        .add_plugins(PanOrbitCameraPlugin)
        .add_plugins(MeshPickingPlugin)
        .insert_resource(SceneGraphRes(Arc::new(sg)))
        .insert_resource(LayoutPositions(positions))
        .insert_resource(MaterialHandles::default())
        .insert_resource(VisibilityToggles {
            layer_visible,
            show_aabbs: false,
            z_separation: z_sep,
        })
        .insert_resource(HoverState::default())
        .insert_resource(SelectionState::default())
        .insert_resource(QueryPanelState::default())
        .add_systems(
            Startup,
            (
                spawn::spawn_camera,
                spawn::spawn_lights,
                spawn::spawn_node_spheres,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                render::edges::draw_intra_layer_edges,
                render::edges::draw_nesting_lines,
                render::bounds::draw_aabb_wireframes,
            ),
        )
        .run();
}
