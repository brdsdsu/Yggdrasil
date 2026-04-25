use bevy::prelude::*;

use crate::components::LayerTag;
use crate::resources::VisibilityToggles;

/// Shows or hides each node sphere based on `VisibilityToggles.layer_visible`.
///
/// Runs every frame so that toggling a layer checkbox in the UI takes effect
/// immediately without requiring a scene rebuild.
pub fn apply_layer_visibility(
    vis: Res<VisibilityToggles>,
    mut query: Query<(&LayerTag, &mut Visibility)>,
) {
    for (tag, mut visibility) in &mut query {
        let visible = vis.layer_visible.get(&tag.layer_id).copied().unwrap_or(true);
        *visibility = if visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}
