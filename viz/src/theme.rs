//! The scene's colour palette, in a dark and a light variant.
//!
//! Every colour in the 3D view was originally written for a dark background, and two of
//! them are *only* meaningful against one: the nesting lines are white at alpha 0.18–0.45,
//! which on a white background is nothing at all. So "light mode" cannot be a clear-colour
//! swap — the palette has to come with it, which is why the colours live here rather than
//! inline at each call site. Nor is it the dark palette inverted: see
//! [`Theme::nesting_base`] for the two separate reasons that fails.
//!
//! What changes between the two, and what does not:
//!
//! | element | dark | light | why |
//! |---|---|---|---|
//! | background | bevy's default `#2b2c2f` | white | figures go on white pages |
//! | node spheres | hsl(hue, 0.75, 0.55) | unchanged | a lit, saturated hue reads on either ground |
//! | intra-layer edges | L 0.35 base / 0.65 highlight | L 0.45 / 0.30 | the highlight must be the *more* prominent of the two, so on a light ground the pair inverts: darker means louder |
//! | nesting lines | white, alpha 0.18 / 0.45 | grey 0.65, alpha 0.42 / 0.7 | the one element with no colour of its own, and deliberately the faintest thing on a light ground. Neither the tone nor the alphas are the dark theme's mirrored; see [`Theme::nesting_base`] |
//! | coordinate spokes | hsla(hue, 0.35, 0.5, 0.35) | hsla(hue, 0.35, 0.35, 0.5) | subordinate to real edges in both, which means *lighter* than them on dark and *fainter* than them on light |
//! | AABB wireframes | hsla(hue, 0.9, 0.7, 0.8) | hsla(hue, 0.9, 0.4, 0.9) | a gizmo overlay, so it tracks the same rule as the edges |
//! | selection ring | pure yellow | amber | pure yellow on white is barely a mark |
//!
//! Hue is always the layer's own ([`layer_hue`]), so which layer a thing belongs to is
//! readable in both themes — only lightness and alpha move.

use bevy::prelude::*;

/// Which palette the 3D view is drawn with.
///
/// Also drives the egui panels' own light/dark visuals, so the toggle is the whole app's
/// rather than the viewport's alone — but only the 3D view reaches a capture, since the
/// panels are not rendered into the offscreen target.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// The hue for a layer, spread evenly around the wheel.
///
/// One definition rather than the four identical copies this used to have, so a sphere,
/// its edges, its spokes and its bounding box cannot drift apart.
pub fn layer_hue(layer_id: u32, num_layers: usize) -> f32 {
    (layer_id as f32 / num_layers.max(1) as f32) * 360.0
}

impl Theme {
    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }

    /// What the camera clears to, and so the background of any capture.
    ///
    /// Dark reproduces bevy's own default clear colour exactly, so switching the toggle on
    /// and off leaves the view where it started.
    pub fn background(self) -> Color {
        match self {
            Self::Dark => Color::srgb_u8(43, 44, 47),
            Self::Light => Color::WHITE,
        }
    }

    /// A node sphere. Lit, unlike everything else here.
    pub fn node(self, hue: f32) -> Color {
        Color::hsl(hue, 0.75, 0.55)
    }

    /// An intra-layer edge, drawn when nothing is hovered, selected or queried.
    pub fn edge_base(self, hue: f32) -> Color {
        match self {
            Self::Dark => Color::hsla(hue, 0.75, 0.35, 0.85),
            Self::Light => Color::hsla(hue, 0.75, 0.45, 0.85),
        }
    }

    /// An intra-layer edge inside the active set. Must out-shout [`Theme::edge_base`],
    /// which on a light background means going darker rather than lighter.
    pub fn edge_highlight(self, hue: f32) -> Color {
        match self {
            Self::Dark => Color::hsla(hue, 0.75, 0.65, 0.85),
            Self::Light => Color::hsla(hue, 0.75, 0.30, 0.85),
        }
    }

    /// A nesting (parent → child) line. Colourless by design: it spans two layers and so
    /// belongs to neither's hue.
    ///
    /// The light theme's line is a **grey**, not the dark theme's white inverted to black,
    /// and its alphas are not the dark ones mirrored either.
    ///
    /// *Grey*, and a light one, for two reasons. Alpha accumulates where lines cross, and
    /// these cross constantly — the fan of children under one parent overlaps into a solid
    /// wedge. A black source drives that wedge to black; a grey source cannot go darker than
    /// the grey however many stack up, so dense regions stay a tone rather than a blot. And
    /// hierarchy is context rather than content: on a white ground these are meant to be the
    /// faintest marks in the scene, well under the edges and spheres that carry the data.
    ///
    /// *Not mirrored* because blending is linear and perception is not: white at alpha 0.18
    /// over the dark ground composites to a *four times* larger sRGB step than black at
    /// alpha 0.18 over white, since sRGB spends most of its range on the dark end. Mirroring
    /// the number produces a line that is technically present and visually absent. These
    /// were fitted to match the dark theme's composited contrast, then checked in a render.
    pub fn nesting_base(self) -> Color {
        match self {
            Self::Dark => Color::srgba(1.0, 1.0, 1.0, 0.18),
            Self::Light => Color::srgba(0.80, 0.80, 0.80, 0.42),
        }
    }

    /// A nesting line inside the active set. Same grey, more of it, so a crossing of
    /// highlighted lines saturates at the same tone the base does.
    pub fn nesting_highlight(self) -> Color {
        match self {
            Self::Dark => Color::srgba(1.0, 1.0, 1.0, 0.45),
            Self::Light => Color::srgba(0.65, 0.65, 0.65, 0.7),
        }
    }

    /// A spoke joining one coordinate of a node to that node's anchor. Deliberately weaker
    /// than a real edge — it connects a node to itself and carries no graph meaning.
    pub fn coord_spoke(self, hue: f32) -> Color {
        match self {
            Self::Dark => Color::hsla(hue, 0.35, 0.5, 0.35),
            Self::Light => Color::hsla(hue, 0.35, 0.35, 0.5),
        }
    }

    /// One slab of an AABB wireframe.
    pub fn aabb(self, hue: f32) -> Color {
        match self {
            Self::Dark => Color::hsla(hue, 0.9, 0.7, 0.8),
            Self::Light => Color::hsla(hue, 0.9, 0.4, 0.9),
        }
    }

    /// The ring around the selected node.
    pub fn selection_ring(self) -> Color {
        match self {
            Self::Dark => Color::srgb(1.0, 0.9, 0.0),
            Self::Light => Color::srgb(0.75, 0.5, 0.0),
        }
    }
}

/// Repaints everything the theme owns when it changes.
///
/// The edge, spoke and sphere colours live in `StandardMaterial` assets built once at
/// startup, so a toggle has to reach into them; the gizmo colours (AABB slabs, selection
/// ring) are re-issued every frame and need nothing here.
///
/// Runs on the first frame too, which is what puts [`Theme::background`] into `ClearColor`
/// — harmless, since dark is bevy's own default.
pub fn apply_theme(
    theme: Res<Theme>,
    mut clear: ResMut<ClearColor>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    layer_materials: Res<crate::resources::LayerMaterials>,
    edge_materials: Res<crate::resources::EdgeMeshMaterials>,
    sg: Res<crate::resources::SceneGraphRes>,
) {
    if !theme.is_changed() {
        return;
    }
    let theme = *theme;
    clear.0 = theme.background();

    let num_layers = sg.0.layers_count();
    let mut repaint = |handle: &Handle<StandardMaterial>, color: Color| {
        if let Some(material) = materials.get_mut(handle) {
            material.base_color = color;
        }
    };

    for (&layer_id, handle) in &layer_materials.0 {
        repaint(handle, theme.node(layer_hue(layer_id, num_layers)));
    }
    for (&layer_id, handle) in &edge_materials.base_by_layer {
        repaint(handle, theme.edge_base(layer_hue(layer_id, num_layers)));
    }
    for (&layer_id, handle) in &edge_materials.highlight_by_layer {
        repaint(
            handle,
            theme.edge_highlight(layer_hue(layer_id, num_layers)),
        );
    }
    for (&layer_id, handle) in &edge_materials.coord_group_by_layer {
        repaint(handle, theme.coord_spoke(layer_hue(layer_id, num_layers)));
    }
    if let Some(handle) = &edge_materials.nesting_base {
        repaint(handle, theme.nesting_base());
    }
    if let Some(handle) = &edge_materials.nesting_highlight {
        repaint(handle, theme.nesting_highlight());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far a colour lands from the background once its alpha is blended in, measured
    /// after sRGB encoding.
    ///
    /// Comparing the raw colours instead is the trap this whole module is about: in linear
    /// terms the dark theme's nesting line and a naive mirrored light one are equally far
    /// from their backgrounds, and on screen one is a line and the other is nothing. The
    /// encoding is where the asymmetry lives, so the measurement has to happen after it.
    fn composited_contrast(theme: Theme, color: Color) -> f32 {
        let ground = theme.background().to_linear();
        let color = color.to_linear();
        let a = color.alpha;
        let blend = |c: f32, g: f32| a * c + (1.0 - a) * g;
        let over = bevy::color::Srgba::from(bevy::color::LinearRgba::new(
            blend(color.red, ground.red),
            blend(color.green, ground.green),
            blend(color.blue, ground.blue),
            1.0,
        ));
        let ground = bevy::color::Srgba::from(ground);
        ((over.red - ground.red).abs()
            + (over.green - ground.green).abs()
            + (over.blue - ground.blue).abs())
            / 3.0
    }

    /// The lightness ordering that makes a highlight read as *more* prominent than the
    /// base flips with the background, and getting it backwards is invisible in code
    /// review but obvious on screen.
    #[test]
    fn the_highlight_out_contrasts_the_base_in_both_themes() {
        for theme in [Theme::Dark, Theme::Light] {
            for (base, highlight) in [
                (theme.edge_base(200.0), theme.edge_highlight(200.0)),
                (theme.nesting_base(), theme.nesting_highlight()),
            ] {
                assert!(
                    composited_contrast(theme, highlight) > composited_contrast(theme, base),
                    "{theme:?}: highlight is no louder than the base against the background",
                );
            }
        }
    }

    /// The contrast of the line that rendered as nothing: the dark theme's white at alpha
    /// 0.18, mirrored to black over white. Derived rather than hardcoded, so the number
    /// stays tied to the case it came from.
    ///
    /// This is the anchor for the nesting lines' floor because it is the one point on the
    /// scale that a render has actually settled: at this value the hierarchy was invisible.
    /// The floor sits a margin above it — deliberately low, since these lines were then
    /// lightened twice on purpose, but not so low that the original failure could return.
    fn measured_invisible() -> f32 {
        composited_contrast(Theme::Light, Color::srgba(0.0, 0.0, 0.0, 0.18))
    }

    /// Everything that carries data must read as well against a white ground as against the
    /// dark one. Parity rather than a fixed threshold, because the two grounds are not
    /// symmetric — that asymmetry is what the mirrored-alpha nesting line fell into.
    #[test]
    fn light_overlays_read_as_strongly_as_their_dark_counterparts() {
        let elements: [(&str, fn(Theme) -> Color); 4] = [
            ("coordinate spoke", |t| t.coord_spoke(200.0)),
            ("aabb", |t| t.aabb(200.0)),
            ("edge base", |t| t.edge_base(200.0)),
            ("edge highlight", |t| t.edge_highlight(200.0)),
        ];

        for (what, color) in elements {
            let dark = composited_contrast(Theme::Dark, color(Theme::Dark));
            let light = composited_contrast(Theme::Light, color(Theme::Light));
            assert!(
                light >= dark * 0.7,
                "{what}: {light:.3} against white against {dark:.3} against the dark ground \
                 — the light theme is the weaker of the two",
            );
        }
    }

    /// Nesting lines are exempt from that parity: on a light ground they are deliberately
    /// the faintest thing in the scene, since hierarchy is context and the edges and spheres
    /// are the content, and they have been lightened twice on request since. What they are
    /// not exempt from is being *visible*, which is the one thing a render has pinned down.
    #[test]
    fn nesting_lines_stay_above_the_visibility_floor() {
        let floor = measured_invisible() * 1.4;
        for theme in [Theme::Dark, Theme::Light] {
            for (what, color) in [
                ("base", theme.nesting_base()),
                ("highlight", theme.nesting_highlight()),
            ] {
                let contrast = composited_contrast(theme, color);
                assert!(
                    contrast >= floor,
                    "{theme:?} nesting {what}: {contrast:.3} is under the {floor:.3} floor, \
                     which is a margin above the value measured invisible in a render",
                );
            }
        }
    }

    /// Nesting lines overlap heavily — every child of one parent fans into the same wedge —
    /// so the source colour, not the composited one, is what the dense regions converge to.
    /// A black source turns those into a blot on a white page.
    #[test]
    fn light_nesting_lines_saturate_to_a_grey_rather_than_to_black() {
        for line in [
            Theme::Light.nesting_base(),
            Theme::Light.nesting_highlight(),
        ] {
            let line = bevy::color::Srgba::from(line);
            assert_eq!(
                (line.red, line.green),
                (line.green, line.blue),
                "the line is tinted; it should be a neutral grey",
            );
            assert!(
                line.red >= 0.25,
                "grey {:.2} is dark enough to read as black where lines pile up",
                line.red,
            );
        }
    }

    #[test]
    fn layer_hues_span_the_wheel_and_survive_an_empty_graph() {
        assert_eq!(layer_hue(0, 4), 0.0);
        assert_eq!(layer_hue(2, 4), 180.0);
        // `num_layers` of zero would divide by zero; a graph with no layers still asks.
        assert_eq!(layer_hue(0, 0), 0.0);
    }
}
