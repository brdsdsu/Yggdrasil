use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::capture::CaptureState;
use crate::components::{CoordIndex, NodeSphere};
use crate::layout::compute_layout;
use crate::resources::{
    LayoutPositions, SceneGraphRes, SphereMeshHandle, UpAxisRes, VisibilityToggles,
};
use crate::theme::Theme;

/// Draws the left sidebar panel containing per-layer visibility toggles,
/// the AABB wireframe toggle, and the z-separation slider.
///
/// When the z-separation slider is moved, the layout is recomputed immediately
/// and all node sphere transforms are updated in place so the scene responds
/// in real time without requiring a restart.
pub fn draw_layer_panel(
    mut contexts: EguiContexts,
    mut vis: ResMut<VisibilityToggles>,
    sg_res: Res<SceneGraphRes>,
    mut layout: ResMut<LayoutPositions>,
    up: Res<UpAxisRes>,
    mut sphere_query: Query<(&NodeSphere, &CoordIndex, &mut Transform)>,
    sphere_handle: Res<SphereMeshHandle>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut capture: ResMut<CaptureState>,
    mut theme: ResMut<Theme>,
) {
    let sg = &sg_res.0;

    egui::SidePanel::left("layers")
        .resizable(true)
        .default_width(210.0)
        .show(contexts.ctx_mut(), |ui| {
            ui.heading("Layers");
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for layer in sg.layers() {
                        let node_count = layer.all_nodes().count();
                        let visible =
                            vis.layer_visible.entry(layer.id).or_insert(true);
                        ui.checkbox(
                            visible,
                            format!("Layer {} ({} nodes)", layer.id, node_count),
                        );
                    }
                });

            ui.separator();
            ui.checkbox(&mut vis.show_aabbs, "Show AABBs");
            ui.checkbox(&mut vis.show_nesting, "Show Hierarchical Edges");
            ui.checkbox(&mut vis.show_coord_groups, "Show Coordinate Groups")
                .on_hover_text(
                    "Thin spokes binding each coordinate of a multi-coordinate node to \
                     that node's anchor. Not graph edges — they connect a node to itself.",
                );

            ui.separator();
            ui.label("Layer Separation");

            let old_z_sep = vis.z_separation;
            ui.add(
                egui::Slider::new(&mut vis.z_separation, 0.1f32..=500.0)
                    .logarithmic(true)
                    .text("units"),
            );

            if (vis.z_separation - old_z_sep).abs() > 1e-4 {
                let new_positions = compute_layout(sg, vis.z_separation, up.0);
                for (sphere, coord_index, mut transform) in &mut sphere_query {
                    let Some(node_layout) = new_positions.get(&sphere.node_id) else {
                        continue;
                    };
                    // Coordinate-less nodes have a single sphere sitting on the anchor.
                    transform.translation = node_layout
                        .coords
                        .get(coord_index.0)
                        .copied()
                        .unwrap_or(node_layout.anchor);
                }
                layout.0 = new_positions;
            }

            ui.separator();
            ui.label("Node Radius");

            let old_radius = vis.node_radius;
            ui.add(
                egui::Slider::new(&mut vis.node_radius, 0.01f32..=10.0)
                    .logarithmic(true)
                    .text("r"),
            );

            if (vis.node_radius - old_radius).abs() > 1e-6 {
                if let Some(mesh) = meshes.get_mut(&sphere_handle.0) {
                    *mesh = Sphere::new(vis.node_radius).mesh().build();
                }
            }

            ui.separator();
            draw_theme_toggle(ui, &mut theme);

            ui.separator();
            draw_capture_section(ui, &mut capture);
        });

    // egui keeps its own palette, so the panels follow the toggle too — set only on a
    // change, since `set_visuals` rebuilds the style every call.
    if theme.is_changed() {
        let visuals = if theme.is_dark() {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        contexts.ctx_mut().set_visuals(visuals);
    }
}

/// The dark/light toggle.
///
/// It is next to the capture button because that is what it is for: a figure headed for a
/// white page wants a white background, and the capture takes the view as it stands.
///
/// Takes the `ResMut` rather than a `&mut Theme`: writing through the latter every frame
/// would deref-mut the resource every frame, and bevy's change detection cannot tell that
/// from a real toggle. `apply_theme` would then repaint — and so re-upload — every material
/// in the scene on every frame.
fn draw_theme_toggle(ui: &mut egui::Ui, theme: &mut ResMut<Theme>) {
    let mut dark = theme.is_dark();
    if ui
        .checkbox(&mut dark, "Dark mode")
        .on_hover_text(
            "Off gives the scene a white background — and repaints the nesting lines, \
             wireframes and selection ring, which are drawn in light tones that vanish \
             against white. Captures take the view as it stands, so switch this off \
             before saving a PNG for a white page.",
        )
        .changed()
    {
        **theme = if dark { Theme::Dark } else { Theme::Light };
    }
}

/// The capture button and its scale selector.
///
/// Split out of [`draw_layer_panel`] only for length; it draws into that same panel and
/// has no meaning outside it.
fn draw_capture_section(ui: &mut egui::Ui, capture: &mut CaptureState) {
    ui.label("Capture");

    ui.horizontal(|ui| {
        for scale in CaptureState::SCALES {
            ui.selectable_value(&mut capture.scale, scale, format!("{scale}×"));
        }
    });

    let size = capture.target_size();
    let button = egui::Button::new("📷  Save PNG");
    let response = ui.add_enabled(!capture.busy(), button).on_hover_text(
        "Renders the current view offscreen at the selected multiple of the window \
         and writes a PNG in the process's working directory. The panels are not \
         part of it — the capture is the scene alone, framed exactly as the viewport \
         frames it.",
    );
    if response.clicked() {
        capture.request();
    }

    ui.small(format!("{} × {} px", size.x, size.y));

    match &capture.message {
        Some(Ok(text)) => {
            ui.small(text.as_str());
        }
        Some(Err(text)) => {
            ui.colored_label(egui::Color32::from_rgb(220, 80, 80), text.as_str());
        }
        None => {}
    }
}
