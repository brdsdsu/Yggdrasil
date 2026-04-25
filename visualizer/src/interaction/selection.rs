use std::collections::{HashSet, VecDeque};

use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;

use crate::components::NodeSphere;
use crate::resources::{QueryPanelState, QueryScope, SceneGraphRes, SelectionState};

/// Handles click events on node spheres to toggle selection.
///
/// Clicking a node sets it as the active selection, computes its full descendant
/// subtree via BFS (stored in `SelectionState.subtree_ids` for the fade system),
/// derives its layer, and automatically switches the query scope to `Node`.
/// Clicking the same node again clears everything and resets scope to `Graph`.
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
            selection.subtree_ids.clear();
            qps.scope = QueryScope::Graph;
        } else {
            let sg = &sg_res.0;

            // BFS downward to collect selected node + all descendants.
            let mut subtree: HashSet<_> = [node_id].into();
            let mut queue = VecDeque::from([node_id]);
            while let Some(nid) = queue.pop_front() {
                if let Ok(node) = sg.node(nid) {
                    for &child_id in node.children() {
                        if subtree.insert(child_id) {
                            queue.push_back(child_id);
                        }
                    }
                }
            }

            selection.selected_node = Some(node_id);
            selection.selected_layer = Some(sg_res.0.layer_of(node_id));
            selection.subtree_ids = subtree;
            qps.scope = QueryScope::Node;
        }
    }
}
