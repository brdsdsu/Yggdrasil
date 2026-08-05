# CLAUDE.md — viz crate

Bevy-based interactive 3D visualizer for Yggdrasil scene graphs.

```bash
# Run with a JSON scene file
cargo run -p viz -- path/to/scene.json

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
├── main.rs              # CLI entry: parses optional JSON path, falls back to Demo
├── lib.rs               # App builder: inserts resources, registers all systems
├── components.rs        # Bevy ECS component types
├── resources.rs         # Bevy Resource types (shared mutable state)
├── layout.rs            # 3D position assignment algorithm
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
| `LayoutPositions` | `HashMap<NodeId, NodeLayout>` — per-coordinate positions plus one anchor per node. Use the `anchor(nid)` / `coords(nid)` accessors rather than indexing `.0` |
| `LayerMaterials` | `HashMap<u32, Handle<StandardMaterial>>` — one `StandardMaterial` per layer (shared by all nodes in that layer) |
| `EdgeMeshMaterials` | `base_by_layer`, `highlight_by_layer` (per-layer edge materials), `nesting_base`, `nesting_highlight`, `coord_group_by_layer` |
| `SphereMeshHandle` | Single shared `Handle<Mesh>` for all node spheres; replaced in-place when the radius slider changes |
| `HoverState` | `hovered_node`, `hovered_edge`, `subtree_ids` (BFS descendants) |
| `SelectionState` | `selected_node`, `selected_layer`, `subtree_ids` (BFS descendants) |
| `VisibilityToggles` | `layer_visible: HashMap<u32, bool>`, `show_aabbs: bool`, `show_nesting: bool`, `show_coord_groups: bool`, `z_separation: f32`, `node_radius: f32` |
| `QueryPanelState` | All query form fields, `highlighted_nodes`, `highlighted_edges`, `error`, `submitted` |

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

`compute_layout(sg, z_separation) -> HashMap<NodeId, NodeLayout>` assigns visual positions to every node. Layers are processed in **ascending ID order** (children before parents).

```rust
pub struct NodeLayout {
    pub coords: Vec<Vec3>,  // one position per Node::coordinates entry, same order
    pub anchor: Vec3,       // where edges, nesting lines and the selection ring attach
}
```

**Three placement rules (priority order):**
1. Node has coordinates → one position per coordinate, XY from the coordinate, `vis_y = coord_y + layer_id * z_separation`; anchor = **medoid** of those positions
2. Node has no coordinates but has children → `coords` empty, anchor at mean XZ of children's anchors
3. Node has no coordinates and no children → `coords` empty, anchor placed randomly within global scene footprint, with up to 10 retry attempts to maintain ≥5% of diagonal (min 0.5 units) separation from existing anchors

The anchor is the **medoid** — the node's own coordinate nearest their mean — not the mean itself. The mean of a node whose coordinates form two distant clusters lands in the empty space between them, which would leave edge endpoints and the selection ring detached from every visible sphere. For a single-coordinate node the medoid *is* that coordinate, so behavior is unchanged.

`compute_z_separation(sg) -> f32` computes a default z-sep as `y_range * 1.5 + 1.0` (minimum 1.0), chosen so adjacent layers don't visually overlap.

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
  draw_layer_panel                               ← must run before props panel
  draw_properties_panel                          ← must run before query panel
  draw_query_panel
  rebuild_meshes_on_layout_change                ← after draw_layer_panel
```

---

## Coordinate Convention

Y-up, `glam::Vec3`. The visual y-axis is overloaded for layer separation — do not interpret `VisPos.y` as a raw scene coordinate.

`Node::coordinates` is a `Vec<Coordinate>`, so "the position of a node" is not a single value. Two scene-graph semantics follow from that and are worth remembering when reading query results:

- FOV visibility is **any** coordinate inside the cone (`scene-graph/src/layer.rs`)
- `nearest_nodes` / `nodes_within` yield one hit **per coordinate** (`scene-graph/src/pos.rs`); `query/dispatch.rs` dedups into a `HashSet`
