use bevy::prelude::*;
use bevy_egui::EguiContexts;
use bevy_panorbit_camera::PanOrbitCamera;

/// Disables [`PanOrbitCamera`] input whenever the pointer is over or actively
/// interacting with (e.g. dragging a slider on) any egui panel.
///
/// Runs in `PostUpdate`, just before `PanOrbitCameraSystemSet`, so the camera
/// sees the updated `enabled` flag before it processes mouse input.
pub fn block_camera_on_egui(
    mut contexts: EguiContexts,
    mut cameras: Query<&mut PanOrbitCamera>,
) {
    let ctx = contexts.ctx_mut();
    let ui_active = ctx.is_pointer_over_area() || ctx.is_using_pointer();
    for mut cam in &mut cameras {
        cam.enabled = !ui_active;
    }
}
