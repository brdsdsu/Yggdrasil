use std::collections::{HashSet, VecDeque};

use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;

use crate::components::NodeSphere;
use crate::resources::{HoverState, SceneGraphRes};

/// Detects when the cursor enters or leaves a node sphere and updates [`HoverState`].
///
/// On `Pointer<Over>`, the hovered node and its full descendant subtree (BFS through
/// `node.children()`) are collected into `HoverState.subtree_ids`.
/// On `Pointer<Out>`, the hover state is cleared.
/// Hover activation is suppressed while any mouse button is pressed so that clicking
/// or camera-dragging does not trigger subtree highlighting.
pub fn detect_node_hover(
    mut over_events: EventReader<Pointer<Over>>,
    mut out_events: EventReader<Pointer<Out>>,
    sphere_query: Query<&NodeSphere>,
    sg_res: Res<SceneGraphRes>,
    mut hover: ResMut<HoverState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
) {
    // While any button is held (click or drag), suppress hover highlighting.
    if mouse_buttons.any_pressed([MouseButton::Left, MouseButton::Right, MouseButton::Middle]) {
        over_events.clear();
        out_events.clear();
        if hover.hovered_node.is_some() {
            hover.hovered_node = None;
            hover.subtree_ids.clear();
        }
        return;
    }

    for event in out_events.read() {
        if sphere_query.get(event.target).is_ok() {
            hover.hovered_node = None;
            hover.subtree_ids.clear();
        }
    }

    for event in over_events.read() {
        let Ok(sphere) = sphere_query.get(event.target) else {
            continue;
        };

        let node_id = sphere.node_id;
        let sg = &sg_res.0;

        // BFS downward through child nodes to collect the full subtree.
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

        hover.hovered_node = Some(node_id);
        hover.subtree_ids = subtree;
    }
}
