pub mod components;
pub mod interaction;
pub mod layout;
pub mod loader;
pub mod query;
pub mod render;
pub mod resources;
pub mod spawn;
pub mod ui;

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_panorbit_camera::{PanOrbitCameraPlugin, PanOrbitCameraSystemSet};
use scene_graph::SceneGraph;

use resources::{
    EdgeMeshMaterials, HoverState, LayerMaterials, LayoutPositions, QueryPanelState, SceneGraphRes,
    SelectionState, SphereMeshHandle, VisibilityToggles,
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
        .insert_resource(LayerMaterials::default())
        .insert_resource(EdgeMeshMaterials::default())
        .insert_resource(SphereMeshHandle::default())
        .insert_resource(VisibilityToggles {
            layer_visible,
            show_aabbs: false,
            z_separation: z_sep,
            node_radius: (z_sep * 0.05).max(0.1),
            show_nesting: true,
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
                render::edges::build_edge_meshes,
                render::edges::build_nesting_mesh,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                render::edges::update_edge_nesting_visibility,
                render::edges::rebuild_edge_highlight,
                render::edges::rebuild_nesting_highlight,
                render::edges::rebuild_meshes_on_layout_change
                    .after(ui::panels::draw_layer_panel),
                render::bounds::draw_aabb_wireframes,
                // Hover detection.
                interaction::hover::detect_node_hover,
                // Click selection (also auto-updates query scope).
                interaction::selection::handle_node_click,
                // Visibility: combines layer toggles + hover/selection/query state.
                render::nodes::update_node_visibility,
                // Selection ring gizmo.
                render::nodes::draw_selection_ring,
                // Query dispatch.
                query::dispatch::execute_query,
                // UI panels — must run in this order: left panel first so
                // available_rect() is correct when the properties window positions itself.
                ui::panels::draw_layer_panel.before(ui::props::draw_properties_panel),
                ui::props::draw_properties_panel.before(ui::query::draw_query_panel),
                ui::query::draw_query_panel,
            ),
        )
        .add_systems(
            PostUpdate,
            interaction::camera::block_camera_on_egui.before(PanOrbitCameraSystemSet),
        )
        .run();
}
