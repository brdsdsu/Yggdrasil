use std::collections::{HashSet, VecDeque};

use bevy::picking::events::{Out, Over, Pointer};
use bevy::prelude::*;

use crate::components::{NodeSphere, Opacity};
use crate::resources::{HoverState, MaterialHandles, SceneGraphRes};

/// Detects when the cursor enters or leaves a node sphere and updates [`HoverState`].
///
/// On `Pointer<Over>`, the hovered node and its full descendant subtree (BFS through
/// `node.children()`) are collected into `HoverState.subtree_ids`.
/// On `Pointer<Out>`, the hover state is cleared.
pub fn detect_node_hover(
    mut over_events: EventReader<Pointer<Over>>,
    mut out_events: EventReader<Pointer<Out>>,
    sphere_query: Query<&NodeSphere>,
    sg_res: Res<SceneGraphRes>,
    mut hover: ResMut<HoverState>,
) {
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

/// Sets each node sphere's `Opacity.target` based on the current hover state.
///
/// When a node is hovered, nodes inside the subtree get target 1.0; all others
/// get target 0.15. When nothing is hovered, every node targets 1.0.
pub fn apply_hover_fade(hover: Res<HoverState>, mut query: Query<(&NodeSphere, &mut Opacity)>) {
    let hovering = hover.hovered_node.is_some();
    for (sphere, mut opacity) in &mut query {
        opacity.target = if !hovering || hover.subtree_ids.contains(&sphere.node_id) {
            1.0
        } else {
            0.15
        };
    }
}

/// Lerps each node sphere's opacity toward its target and writes the result into
/// the node's `StandardMaterial` base color alpha.
///
/// Lerp speed is 8.0 per second, giving a ~125 ms half-life transition.
pub fn lerp_opacity(
    time: Res<Time>,
    mat_handles: Res<MaterialHandles>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(&NodeSphere, &mut Opacity)>,
) {
    let dt = time.delta_secs();
    for (sphere, mut opacity) in &mut query {
        let t = (dt * 8.0).min(1.0);
        opacity.current += (opacity.target - opacity.current) * t;

        if let Some(handle) = mat_handles.0.get(&sphere.node_id) {
            if let Some(mat) = materials.get_mut(handle) {
                mat.base_color.set_alpha(opacity.current);
            }
        }
    }
}
