use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::components::NodeSphere;
use crate::resources::{QueryPanelState, QueryScope, SceneGraphRes, SelectionState};

/// Handles click events on node spheres to toggle selection.
///
/// Clicking a node sets it as the active selection, derives its layer, and
/// automatically switches the query scope to `Node`. Clicking the same node
/// again clears both the selection and resets the scope to `Graph`.
pub fn handle_node_click(
    mut click_events: EventReader<Pointer<Click>>,
    sphere_query: Query<&NodeSphere>,
    sg_res: Res<SceneGraphRes>,
    mut selection: ResMut<SelectionState>,
    mut qps: ResMut<QueryPanelState>,
) {
    for event in click_events.read() {
        let Ok(sphere) = sphere_query.get(event.target) else {
            continue;
        };

        let node_id = sphere.node_id;

        if selection.selected_node == Some(node_id) {
            selection.selected_node = None;
            selection.selected_layer = None;
            qps.scope = QueryScope::Graph;
        } else {
            selection.selected_node = Some(node_id);
            selection.selected_layer = Some(sg_res.0.layer_of(node_id));
            qps.scope = QueryScope::Node;
        }
    }
}
