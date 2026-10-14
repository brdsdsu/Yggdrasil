# CLAUDE.md — viz crate

Bevy-based interactive 3D visualizer for Yggdrasil scene graphs.

```bash
# Run with a JSON scene file
cargo run -p viz -- path/to/scene.json

# A scene authored in the renderer's own Y-up frame (the default is Z-up)
cargo run -p viz -- path/to/scene.json --up y

# Run the built-in 4-layer demo scene
cargo run -p viz

# Same demo, but each object's point cloud is one multi-coordinate node
cargo run -p viz -- --demo-multi
```

---

## Invariants

These behaviors **must be preserved** at all times — during refactoring, feature additions, and any other changes.

1. **Layer visibility disables hierarchical edges.** When a layer is toggled off via `VisibilityToggles.layer_visible`, all nesting lines (dashed inter-layer edges) whose either endpoint belongs to that layer must stop being drawn. Intra-layer edges for that layer are also suppressed. No hierarchical edge that touches a hidden layer should appear.

2. **Layer visibility disables bounding boxes.** When a layer is toggled off, AABB wireframes for nodes belonging to that layer must not be drawn, regardless of the `show_aabbs` toggle.

3. **A node's spheres act as one unit.** `Node::coordinates` is a `Vec`, so a node is drawn as one sphere per coordinate — every one of them carrying the same `NodeSphere.node_id`. Hover, selection, query highlighting and layer visibility must always affect all of a node's spheres together; no system may treat individual coordinates as separately selectable entities.

4. **Coordinate-group spokes never outlive their spheres.** A node's spokes are drawn only when its spheres are, using the same `layer_visible && in_active_set` rule as `update_node_visibility`.

---

## Source Layout

```
viz/src/
├── main.rs              # CLI entry: parses optional JSON path and --up, falls back to Demo
├── lib.rs               # App builder: inserts resources, registers all systems
├── components.rs        # Bevy ECS component types
├── resources.rs         # Bevy Resource types (shared mutable state)
├── layout.rs            # 3D position assignment algorithm
├── capture.rs           # Offscreen high-resolution PNG capture of the current view
├── theme.rs             # Dark/light palette for the 3D view, and the repaint system
├── loader.rs            # Scene loading (JSON file or built-in demo)
├── spawn.rs             # Startup systems: camera, lights, node spheres
├── render/
│   ├── nodes.rs         # Node visibility and selection ring
│   ├── edges.rs         # Static LineList meshes for intra-layer and nesting edges
│   ├── coord_groups.rs  # LineList spokes binding a node's coordinates to its anchor
│   └── bounds.rs        # AABB wireframe overlays
├── interaction/
│   ├── hover.rs         # Pointer-Over/Out → HoverState
│   └── selection.rs     # Pointer-Click → SelectionState + query scope sync
├── ui/
│   ├── panels.rs        # Left sidebar: layer toggles, AABB/nesting toggles, sliders
│   ├── props.rs         # Floating properties panel (appears on hover)
│   └── query.rs         # Right sidebar: query form, results, error display
└── query/
    └── dispatch.rs      # Executes submitted query, writes highlighted sets
```

---

## Components (`components.rs`)

| Component | Purpose |
|---|---|
| `NodeSphere { node_id }` | Marks a mesh entity as a scene graph node sphere. **Shared** by every sphere of a multi-coordinate node |
| `CoordIndex(usize)` | Which coordinate of its node this sphere stands for. Coordinate-less nodes get one sphere indexed `0` |
| `LayerTag { layer_id }` | Associates a mesh entity with its layer |
| `VisPos(Vec3)` | Computed visual position of this sphere (`node_y + layer_id * z_sep`) |
| `EdgeBaseMesh { layer_id }` | Tags the base intra-layer edge `LineList` mesh for one layer |
| `EdgeHighlightMesh { layer_id }` | Tags the highlight overlay `LineList` mesh for one layer's edges |
| `NestingBaseMesh` | Tags the single global nesting (parent→child) base mesh |
| `NestingHighlightMesh` | Tags the global nesting highlight overlay mesh |
| `CoordGroupMesh { layer_id }` | Tags one layer's coordinate-group spoke `LineList` mesh |

---

## Resources (`resources.rs`)

| Resource | Contents |
|---|---|
| `SceneGraphRes` | `Arc<SceneGraph>` — shared across all systems |
| `UpAxisRes` | `UpAxis` — the declared vertical axis of the scene data. Fixed at startup; read by the layer-separation slider (which recomputes the layout) and by the AABB renderer |
| `LayoutPositions` | `HashMap<NodeId, NodeLayout>` — per-coordinate positions plus one anchor per node. Use the `anchor(nid)` / `coords(nid)` accessors rather than indexing `.0` |
| `LayerMaterials` | `HashMap<u32, Handle<StandardMaterial>>` — one `StandardMaterial` per layer (shared by all nodes in that layer) |
| `EdgeMeshMaterials` | `base_by_layer`, `highlight_by_layer` (per-layer edge materials), `nesting_base`, `nesting_highlight`, `coord_group_by_layer` |
| `SphereMeshHandle` | Single shared `Handle<Mesh>` for all node spheres; replaced in-place when the radius slider changes |
| `HoverState` | `hovered_node`, `hovered_edge`, `subtree_ids` (BFS descendants) |
| `SelectionState` | `selected_node`, `selected_layer`, `subtree_ids` (BFS descendants) |
| `VisibilityToggles` | `layer_visible: HashMap<u32, bool>`, `show_aabbs: bool`, `show_nesting: bool`, `show_coord_groups: bool`, `z_separation: f32`, `node_radius: f32` |
| `QueryPanelState` | All query form fields, `highlighted_nodes`, `highlighted_edges`, `error`, `submitted` |
| `Theme` | `Dark` (default) or `Light`. Owns every colour in the 3D view; `apply_theme` repaints the startup-built materials when it changes |
| `CaptureState` | `scale` (1/2/4/8×), `dir`, `message: Option<Result<String, String>>`, `window_size`, and a private capture `phase` |

### `QueryType` enum (8 variants)

| Variant | Scene graph call |
|---|---|
| `NodesByFeature` | `nodes_matching(&[Feature::new(key, val)])` |
| `NodesByKey` | `nodes_having(&[key])` |
| `EdgesByLabel` | `edges_matching(label)` |
| `EdgesFrom` | `edges_from(node_id)` |
| `EdgesTo` | `edges_to(node_id)` |
| `NearestNode` | `nearest_nodes(pos, count)` or `nearest_in_subtree(pos, root, count)` |
| `NodesWithin` | `nodes_within(pos, radius, count)` |
| `VisibleSubgraph` | `visible_subgraph(&observer, root)` |

Only `NearestNode` and `VisibleSubgraph` support `QueryScope::Node` (subtree scope).

### `QueryScope` enum

- `Graph` — entire scene graph
- `Layer` — currently selected layer (disabled in UI when no layer selected)
- `Node` — subtree rooted at selected node (disabled when no node selected or query type doesn't support it)

---

## Layout (`layout.rs`)

`compute_layout(sg, z_separation, up) -> HashMap<NodeId, NodeLayout>` assigns visual positions to every node. Layers are processed in **ascending ID order** (children before parents).

```rust
pub struct NodeLayout {
    pub coords: Vec<Vec3>,  // one position per Node::coordinates entry, same order
    pub anchor: Vec3,       // where edges, nesting lines and the selection ring attach
}
```

**Three placement rules (priority order):**
1. Node has coordinates → one position per coordinate, mapped through `up` into the visual frame, then `vis_y += layer_id * z_separation`; anchor = **medoid** of those positions
2. Node has no coordinates but has children → `coords` empty, anchor at mean XZ of children's anchors
3. Node has no coordinates and no children → `coords` empty, anchor placed randomly within global scene footprint, with up to 10 retry attempts to maintain ≥5% of diagonal (min 0.5 units) separation from existing anchors

The anchor is the **medoid** — the node's own coordinate nearest their mean — not the mean itself. The mean of a node whose coordinates form two distant clusters lands in the empty space between them, which would leave edge endpoints and the selection ring detached from every visible sphere. For a single-coordinate node the medoid *is* that coordinate, so behavior is unchanged.

`compute_z_separation(sg, up) -> f32` computes a default z-sep as `y_range * 1.5 + 1.0` (minimum 1.0) over the **visual-frame** bounds, so `y_range` is the scene's real height and adjacent layers don't visually overlap.

`suggest_node_radius(layout, fallback) -> f32` picks the initial sphere radius. The `fallback` (`z_sep * 0.05`) sizes spheres against the whole scene's height and so can never resolve the coordinates *within* one node — at that size a multi-coordinate node is one blob with its spokes buried. When any node holds several coordinates this pulls the radius down to a fifth of the median coordinate-to-anchor distance. It returns `fallback` untouched when every node has at most one coordinate, so single-coordinate scenes keep their historical sizing.

---

## Loader (`loader.rs`)

Three input sources:

- `SceneInput::File(PathBuf)` — deserializes a JSON scene graph via `serde_json`
- `SceneInput::Demo` — built-in 4-layer scene:

```
root_layer (layer 3)
├── dynamic_objects_layer (layer 2) — Human, Dog, Cat
│   │   edges: Human→Dog "holding", Dog→Cat "looking"
│   └── point_cloud_layer (layer 0) — 30 points in [10, 20]³
└── static_objects_layer (layer 1) — car, tree, building, bicycle, bench, sign, lamp
    └── point_cloud_layer (layer 0) — 70 points in [0, 10]³
```

Total: 111 nodes, 2 semantic edges.

- `SceneInput::DemoMulti` (`--demo-multi`) — the same scene built with `PointCloudMode::NodePerObject`: identical layer DAG, object nodes, nesting and semantic edges, but each object's 10 points are held as the coordinates of **one** node instead of 10 separate nodes. Total: 21 nodes carrying the same 100 coordinates. This is the only built-in scene that exercises the multi-coordinate path — `new_coordinates` can only ever produce a one-coordinate node, so multi-coordinate nodes are assembled from `new_node` with `coordinates` and `bounds` set directly.

`NodePerObject` also clusters each object's points around a random center (`CLUSTER_RADIUS`) rather than scattering them across the whole cube, so a node's spokes stay readable instead of fanning across the scene. `point_cloud_id` numbering stays sequential over 0..100 in both modes.

---

## Spawning (`spawn.rs`)

Three startup systems:

1. **`spawn_camera`** — `Camera3d` + `PanOrbitCamera`. Focus at centroid of all node anchors; initial position at `focus + (0, 10, 30)`.
2. **`spawn_lights`** — Ambient (brightness 300) + directional (illuminance 8000, no shadows, from (50, 100, 50)).
3. **`spawn_node_spheres`** — One sphere **per coordinate** using a shared mesh asset (`SphereMeshHandle`); a coordinate-less node still gets exactly one sphere, at its anchor. Radius from `suggest_node_radius`. Color = HSL(`layer_id / num_layers * 360`, 0.75, 0.55) via `LayerMaterials`. Each sphere carries `NodeSphere`, `CoordIndex`, `LayerTag`, `VisPos`, `Visibility`.

---

## Render Systems (`render/`)

### `nodes.rs` — 2 systems

**`update_node_visibility`** — Combines layer visibility and active-set logic into a single binary `Visibility` toggle. A sphere is visible iff its layer is visible AND (no state is active OR its node is in the active set). Since it keys off `NodeSphere.node_id`, all of a node's spheres toggle together with no per-coordinate logic. Active set priority: query results > hover subtree > selection subtree. Runs only when `HoverState`, `SelectionState`, `QueryPanelState`, or `VisibilityToggles` change.

**`draw_selection_ring`** — Bright yellow XY-plane circle gizmo at selected node. Radius = `z_separation * 0.08` (min 0.2). Uses `Gizmos`.

### `edges.rs` — 2 startup + 4 update systems

Edges are static `PrimitiveTopology::LineList` meshes, not per-frame gizmos. Each layer has a base mesh (always-on when no state is active) and a highlight mesh (rebuilt when state changes).

**Startup:**

**`build_edge_meshes`** — Spawns one `EdgeBaseMesh` + one `EdgeHighlightMesh` entity per layer. Base is pre-filled with all intra-layer edge positions (offset `+0.02` on Y to avoid z-fighting). Highlight starts empty. Base material: HSL(hue, 0.75, 0.35, 0.85); highlight: HSL(hue, 0.75, 0.65, 0.85).

**`build_nesting_mesh`** — Spawns one `NestingBaseMesh` + one `NestingHighlightMesh` entity for all cross-layer nesting links. Base material: `RGBA(1,1,1,0.18)`; highlight: `RGBA(1,1,1,0.45)`. Respects per-layer visibility when building initial positions.

**Update:**

**`update_edge_nesting_visibility`** — Toggles base vs. highlight mesh visibility when hover/selection/query/layer-visibility state changes. Base shown when no state active; highlight shown when state active. Nesting meshes additionally gated by `show_nesting`. Skips if no relevant resource changed.

**`rebuild_edge_highlight`** — Rebuilds highlight mesh vertex data when state changes. Only edges whose both endpoints are in the active set are included.

**`rebuild_nesting_highlight`** — Same as above but for nesting links.

**`rebuild_meshes_on_layout_change`** — Rebuilds base edge and nesting mesh positions after `LayoutPositions` changes (e.g. z-separation slider). Runs after `draw_layer_panel`.

All endpoints above come from `LayoutPositions::anchor()`, never from individual coordinates, so an edge is one line regardless of how many coordinates its endpoints hold.

### `coord_groups.rs` — 1 startup + 1 update system

Thin spokes joining each coordinate of a node to that node's anchor. These are **not graph edges** — they connect a node to itself, so they are styled subordinate to lines that carry meaning: HSL(layer hue, 0.35, 0.5, 0.35), versus saturation 0.75 for intra-layer edges and white α 0.18–0.45 for nesting.

**`build_coord_group_meshes`** (startup) — Spawns one `CoordGroupMesh { layer_id }` per layer, pre-filled with every spoke in that layer. Only nodes with **≥2** coordinates contribute; a single-coordinate node's anchor is its only coordinate, so its spoke would be zero-length.

**`rebuild_coord_groups`** (update) — Rebuilds each layer's vertex data using the same `layer_visible && in_active_set` predicate as `update_node_visibility`, which is what keeps invariant 4 true. Mesh visibility additionally gated by `show_coord_groups`. Runs after `draw_layer_panel` (the z-separation slider mutates the layout mid-frame) and skips when no relevant resource changed.

### `bounds.rs` — 1 system

**`draw_aabb_wireframes`** — Draws per-node AABB wireframes when `show_aabbs` is true and the node's layer is visible (invariant 2). One slab per descendant layer (BFS over layer DAG): the leaf layer uses the actual z-extent from the bounding box; other layers draw a flat slab at `layer_id * z_separation`. Color = HSL(sub-layer hue, 0.9, 0.7, 0.8). When hovering, only nodes in the hover subtree are drawn.

---

## Interaction Systems (`interaction/`)

### `hover.rs`

**`detect_node_hover`** — Listens to `Pointer<Over>` / `Pointer<Out>` on node spheres. On entry, sets `HoverState.hovered_node` and populates `subtree_ids` via BFS through `node.children()`. On exit, clears both.

### `selection.rs`

**`handle_node_click`** — Listens to `Pointer<Click>`. Clicking the already-selected node deselects it (clears state, resets scope to `Graph`). Clicking a new node sets `SelectionState` fields and BFS-collects descendants. Auto-switches query scope to `Node` if the current query type supports it; otherwise writes an error message to `QueryPanelState`.

---

## UI Panels (`ui/`)

System draw order matters — left panel before properties before query, so `available_rect()` is correct for each.

### `panels.rs` — Left sidebar

- Scrollable layer list (300px max): one checkbox per layer, label `"Layer {id} ({node_count} nodes)"`
- "Show AABBs" checkbox
- "Show Hierarchical Edges" checkbox (`VisibilityToggles.show_nesting`)
- "Show Coordinate Groups" checkbox (`VisibilityToggles.show_coord_groups`)
- "Dark mode" checkbox (`Theme`). Also switches egui's own visuals, so the panels follow
- Capture section: scale selector (1×–8×), "📷 Save PNG" button (disabled mid-capture), the resulting pixel size, and the last result or error. Drawn by `draw_capture_section`, a helper in the same file
- "Z Separation" logarithmic slider (0.1 – 500.0); changing it immediately recomputes layout and updates all sphere transforms in place (no scene rebuild), each sphere reading `coords[CoordIndex]` and falling back to the anchor
- "Node Radius" logarithmic slider (0.01 – 10.0); changing it replaces the shared sphere mesh asset in place so all spheres resize immediately

### `props.rs` — Floating properties window

Appears when any node is hovered. Positioned right of the layer panel with a 16px gap, constrained to remaining screen area. Sections (each omitted if empty):

1. Identity grid: Layer ID, Node ID, Parent ID
2. Coordinates: `Coordinates (N)` header over a scrollable striped index → `(x, y, z)` grid (3-decimal, max 120px). The anchor's row is marked `⚓`. Shows `Coordinates: none` when the list is empty
3. Bounds: min(x,y,z) and max(x,y,z)
4. Features: scrollable key-value grid (striped)
5. Edges: scrollable list of outgoing edges, format `src_layer:src_node → dst_layer:dst_node [label]` (max 120px)

Window is resizable and collapsible (min width 200px).

### `query.rs` — Right sidebar

- Type ComboBox (8 query types), Scope ComboBox (context-sensitive — Layer disabled without selection, Node disabled without node selection or unsupported type)
- Dynamic input fields per query type:
  - **NodesByFeature** — Key, Value
  - **NodesByKey** — Key
  - **EdgesByLabel** — Label
  - **EdgesFrom / EdgesTo** — Node ID (`layer:node`), "Use selection" button
  - **NearestNode** — Position (x/y/z, 52px each), Count (60px), "All" checkbox
  - **NodesWithin** — Position, Radius, Count, "All" checkbox
  - Count on both counts nearest **coordinates**, not nodes (`nearest_nodes`/`nodes_within` yield one hit per coordinate), so results may contain fewer distinct nodes — noted in a hover tooltip
  - **VisibleSubgraph** — Observer position (x/y/z), Yaw/Pitch/Roll (degrees, 52px), Half-angle (degrees), Near/Far (72px), Root node, "Use Camera" button
- "▶ Run Query" sets `submitted = true`; "✕ Clear" clears highlighted sets (grayed when empty)
- Error message shown in red when `QueryPanelState.error` is set
- Results section: count label + scrollable list of node/edge IDs (max 200px)

---

## Theme (`theme.rs`)

`Theme::Dark` (the default) and `Theme::Light` are the two palettes for the 3D view. The
toggle exists for captures: a figure headed for a white page wants a white background, and
the capture takes the view as it stands.

**It cannot be a clear-colour swap.** Every colour in the view was written against a dark
ground, and the nesting lines have no colour of their own at all — they are white at alpha
0.18/0.45, which on white is nothing. So the palette moves with the background, and it
lives in one place rather than inline at each call site. `layer_hue` also lives here,
replacing the four identical copies of `(layer_id / num_layers) * 360.0` that had been
spread across `spawn.rs`, `edges.rs`, `coord_groups.rs` and `bounds.rs`.

Hue is always the layer's own in both themes; only lightness and alpha move:

| element | dark | light |
|---|---|---|
| background | `#2b2c2f` (bevy's default, so dark is exactly what it always was) | white |
| node spheres | hsl(hue, 0.75, 0.55) | unchanged — a lit, saturated hue reads on either ground |
| intra-layer edges | L 0.35 base / 0.65 highlight | L 0.45 / 0.30 |
| nesting lines | white, α 0.18 / 0.45 | **grey 0.65**, α 0.42 / 0.7 |
| coordinate spokes | hsla(hue, 0.35, 0.5, 0.35) | hsla(hue, 0.35, 0.35, 0.5) |
| AABB wireframes | hsla(hue, 0.9, 0.7, 0.8) | hsla(hue, 0.9, 0.4, 0.9) |
| selection ring | pure yellow | amber |

Two rules the tests enforce, both of which were violated by the obvious implementation:

1. **The highlight must out-contrast the base**, which on a light ground means going
   *darker* rather than lighter — the pair inverts rather than shifting.
2. **The light alphas are not the dark ones mirrored.** Blending is linear and perception
   is not: white at α 0.18 over `#2b2c2f` composites to a ~4× larger sRGB step than black
   at α 0.18 over white, because sRGB spends most of its range on the dark end. Mirroring
   the number gives a line that is technically present and visually absent — measured at
   0.31 contrast against dark versus 0.08 against white, and confirmed in a render before
   the alphas were refitted.
3. **The light nesting line is a light grey, not an inverted white.** Alpha accumulates
   where lines cross, and nesting lines cross constantly: every child of one parent fans
   into the same wedge. A black source drives that wedge to black; a grey source cannot go
   past the grey however many stack up, so the source tone *is* the floor of the densest
   region. Measured in a render: a median line of 219 with pile-ups bottoming out at 135.

`composited_contrast` in the tests blends the colour over its own background and measures
*after* sRGB encoding, because a check on the raw colours passes the invisible case: black
is the exact opposite of white no matter how little of it there is.

Two different bars, because the elements are not doing the same job:

- **Everything that carries data** — edges, spokes, AABBs — must hold **parity** with its
  dark counterpart (≥ 0.7×). A fixed threshold cannot express this, since the two grounds
  are not symmetric, and that asymmetry is the trap rule 2 describes.
- **Nesting lines are exempt from parity and held to a visibility floor instead.** On a
  white ground they are meant to be the faintest thing in the scene — hierarchy is context,
  the edges and spheres are the content — so parity would force them heavier than they
  should be, and they have since been lightened twice on request. The floor is anchored to
  the one point on the scale a render has actually settled: the mirrored-alpha line that
  came back invisible, computed rather than hardcoded, with the bar a margin above it. The
  base currently sits at 1.49× that anchor, so there is not much room left below.

`apply_theme` repaints on change: the sphere, edge, spoke and nesting colours live in
`StandardMaterial` assets built once at startup, so a toggle has to reach into them. The
gizmo colours (AABB slabs, selection ring) are re-issued every frame and need nothing. It
is ordered **before** `drive_capture`, so a theme switched in the same frame as a capture is
the one that lands in the PNG.

---

## Capture (`capture.rs`)

The "📷 Save PNG" button renders the current view offscreen at a multiple of the window's
pixel size and writes it to `CaptureState::dir` (the process's working directory) as
`viz-<unix seconds>[-n].png`.

**It is not a screenshot.** `Screenshot::primary_window()` reads back the swapchain, which
bounds the output at the window's pixel count and bakes in the egui panels. Instead
`drive_capture` spawns a second camera carrying the on-screen camera's `Transform` and a
**clone** of its `Projection` (a fresh one would use the default FOV and stop matching what
the viewport frames), pointed at a `RenderTarget::Image` of `window_size * scale`, and
captures that. What follows from the offscreen target: no UI, since `bevy_egui` only draws
into the window's target; and the gizmo overlays *are* included, since they render for
every view on the default gizmo layer.

`Phase` is a four-state machine advanced one step per frame, and the frame counts are not
padding:

| phase | what it waits for |
|---|---|
| `Requested` | the button was pressed; the target is built on the next tick |
| `Warmup` | `WARMUP_FRAMES` (2) of the offscreen camera rendering. `prepare_screenshots` resolves the target through `RenderAssets<GpuImage>`, and an image asset created this frame has no GPU texture yet — capturing immediately is skipped with only a log line |
| `Capturing` | one frame after the `Screenshot` entity is spawned. Its render pass runs after this system, so despawning the camera immediately would capture an unrendered target |
| `Idle` | camera despawned, image handle dropped |

Scale is clamped so neither side exceeds `RenderDevice::limits().max_texture_dimension_2d`
(8192 by default) — 8× on a 4K window would ask for 30,720 pixels across and fail to
allocate at all.

The PNG is written by `save_png` rather than by bevy's `save_to_disk` observer, so a
failure reaches the panel instead of only the log; the alpha channel is dropped first,
because it carries brightness rather than opacity once HDR is on. **PNG** is the format
because the subject is line art on a flat field — spheres, one-pixel edges, wireframes —
which is exactly what JPEG's chroma subsampling frays.

The image asset is `RenderAssetUsages::RENDER_WORLD` only: nothing reads the pixels on the
CPU (the screenshot copies them out of the GPU texture) and at 8× the main-world copy alone
would be hundreds of megabytes.

---

## Query Dispatch (`query/dispatch.rs`)

**`execute_query`** — Fires when `QueryPanelState.submitted` is true.

1. Calls `run_query()` which dispatches to the appropriate scene graph API based on `QueryType` and `QueryScope`.
2. On success: BFS-expands every result node to include its full descendant subtree; stores in `highlighted_nodes`. Edge results stored in `highlighted_edges`. Clears error.
3. On error: sets `QueryPanelState.error` with a human-readable message.

Parse helpers: `parse_node_id("layer:node")`, `parse_coord([x, y, z])`, `parse_degrees(s, label)` (converts to radians).

**Layer scope for `NearestNode` is `todo!()`** — not yet implemented.

---

## System Execution Order

```
Startup (chained):
  spawn_camera
  spawn_lights
  spawn_node_spheres
  build_edge_meshes
  build_nesting_mesh
  build_coord_group_meshes

Update tick:
  update_edge_nesting_visibility
  rebuild_edge_highlight
  rebuild_nesting_highlight
  rebuild_coord_groups                           ← after draw_layer_panel
  draw_aabb_wireframes
  detect_node_hover
  handle_node_click
  update_node_visibility
  draw_selection_ring
  execute_query
  apply_theme                                    ← after draw_layer_panel, before drive_capture
  drive_capture                                  ← after draw_layer_panel
  draw_layer_panel                               ← must run before props panel
  draw_properties_panel                          ← must run before query panel
  draw_query_panel
  rebuild_meshes_on_layout_change                ← after draw_layer_panel
```

---

## Coordinate Convention

The renderer is Y-up, `glam::Vec3`. The visual y-axis is overloaded for layer separation — do not interpret `VisPos.y` as a raw scene coordinate.

**The scene data need not be Y-up, and by default is assumed not to be.** `UpAxis` (`layout.rs`) names the vertical axis of the incoming coordinates and defaults to `Z`, the ROS REP-103 convention every Spark-DSG export follows (Kimera, Hydra, the uHumans2 corpora). `--up y` selects the pass-through mapping.

`UpAxis::to_visual` is the only place the two frames meet, and everything downstream of it lives in the visual frame: `compute_layout`, `global_bounds`, `compute_z_separation`, and the AABB wireframes (via `to_visual_aabb`, since node bounds are stored in the scene's frame). Z-up is a −90° rotation about X, `(x, y, z) -> (x, z, -y)` — determinant +1, so handedness survives; the tempting `(x, z, y)` mirrors every floor plan.

Reading the wrong axis as up does not merely tilt the view. The scene's *height* feeds `compute_z_separation`, so a floor-plan dimension in its place spreads the layers by that much instead: the office corpus is 50.8 m × 43.1 m × **3.02 m**, so a Y-up reading picks a layer separation of 65.6 instead of 5.5 and — through `suggest_node_radius`'s `z_sep * 0.05` fallback — draws every point as a 3.3 m sphere in a 3 m room.

Two frames stay untouched: `props.rs` shows `Node::coordinates` as stored, and the query panel takes positions in scene coordinates, so both read in the source data's own frame.

One known gap, pre-dating the up axis: the query panel's **"Use Camera"** button copies the camera's *visual*-frame translation and forward vector straight into the observer fields, which `execute_query` then hands to the scene graph as *scene* coordinates. The layer offset alone already made that wrong for anything above layer 0; a non-Y up axis adds the rotation on top. Fixing it needs an inverse mapping and a decision about which layer's offset to subtract, so it is recorded rather than papered over.

`Node::coordinates` is a `Vec<Coordinate>`, so "the position of a node" is not a single value. Two scene-graph semantics follow from that and are worth remembering when reading query results:

- FOV visibility is **any** coordinate inside the cone (`scene-graph/src/layer.rs`)
- `nearest_nodes` / `nodes_within` yield one hit **per coordinate** (`scene-graph/src/pos.rs`); `query/dispatch.rs` dedups into a `HashSet`
