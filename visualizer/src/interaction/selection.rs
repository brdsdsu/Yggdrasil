use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::components::NodeSphere;
use crate::resources::{SceneGraphRes, SelectionState};

/// Handles click events on node spheres to toggle selection.
///
/// Clicking a node sets it as the active selection and derives its layer.
/// Clicking the same node again clears the selection entirely.
/// Selection is used by the query panel to scope queries to a specific
/// node subtree or layer.
pub fn handle_node_click(
    mut click_events: EventReader<Pointer<Click>>,
    sphere_query: Query<&NodeSphere>,
    sg_res: Res<SceneGraphRes>,
    mut selection: ResMut<SelectionState>,
) {
    for event in click_events.read() {
        let Ok(sphere) = sphere_query.get(event.target) else {
            continue;
        };

        let node_id = sphere.node_id;

        if selection.selected_node == Some(node_id) {
            // Second click on the same node clears the selection.
            selection.selected_node = None;
            selection.selected_layer = None;
        } else {
            selection.selected_node = Some(node_id);
            selection.selected_layer = Some(sg_res.0.layer_of(node_id));
        }
    }
}
