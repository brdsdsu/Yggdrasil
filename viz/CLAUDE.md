# CLAUDE.md — viz crate

Bevy-based interactive 3D visualizer for Yggdrasil scene graphs.

```bash
# Run with a JSON scene file
cargo run -p viz -- path/to/scene.json

# Run the built-in 4-layer demo scene
cargo run -p viz
```

---

## Invariants

These behaviors **must be preserved** at all times — during refactoring, feature additions, and any other changes.

1. **Layer visibility disables hierarchical edges.** When a layer is toggled off via `VisibilityToggles.layer_visible`, all nesting lines (dashed inter-layer edges) whose either endpoint belongs to that layer must stop being drawn. Intra-layer edges for that layer are also suppressed. No hierarchical edge that touches a hidden layer should appear.

2. **Layer visibility disables bounding boxes.** When a layer is toggled off, AABB wireframes for nodes belonging to that layer must not be drawn, regardless of the `show_aabbs` toggle.

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
| `NodeSphere { node_id }` | Marks a mesh entity as a scene graph node sphere |
| `LayerTag { layer_id }` | Associates a mesh entity with its layer |
| `VisPos(Vec3)` | Computed visual position in 3D space (`node_z + layer_id * z_sep`) |
| `EdgeBaseMesh { layer_id }` | Tags the base intra-layer edge `LineList` mesh for one layer |
| `EdgeHighlightMesh { layer_id }` | Tags the highlight overlay `LineList` mesh for one layer's edges |
| `NestingBaseMesh` | Tags the single global nesting (parent→child) base mesh |
| `NestingHighlightMesh` | Tags the global nesting highlight overlay mesh |

---

## Resources (`resources.rs`)

| Resource | Contents |
|---|---|
| `SceneGraphRes` | `Arc<SceneGraph>` — shared across all systems |
| `LayoutPositions` | `HashMap<NodeId, Vec3>` — visual positions for every node |
| `LayerMaterials` | `HashMap<u32, Handle<StandardMaterial>>` — one `StandardMaterial` per layer (shared by all nodes in that layer) |
| `EdgeMeshMaterials` | `base_by_layer`, `highlight_by_layer` (per-layer edge materials), `nesting_base`, `nesting_highlight` |
| `SphereMeshHandle` | Single shared `Handle<Mesh>` for all node spheres; replaced in-place when the radius slider changes |
| `HoverState` | `hovered_node`, `hovered_edge`, `subtree_ids` (BFS descendants) |
| `SelectionState` | `selected_node`, `selected_layer`, `subtree_ids` (BFS descendants) |
| `VisibilityToggles` | `layer_visible: HashMap<u32, bool>`, `show_aabbs: bool`, `show_nesting: bool`, `z_separation: f32`, `node_radius: f32` |
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

`compute_layout(sg, z_separation) -> HashMap<NodeId, Vec3>` assigns a visual position to every node. Layers are processed in **ascending ID order** (children before parents).

**Three placement rules (priority order):**
1. Node has coordinates → use XY from coordinates, `vis_z = node_z + layer_id * z_separation`
2. Node has no coordinates but has children → place at mean XY of children
3. Node has no coordinates and no children → random XY within global scene footprint, with up to 10 retry attempts to maintain ≥5% of diagonal (min 0.5 units) separation from existing nodes

`compute_z_separation(sg) -> f32` computes a default z-sep as `z_range * 1.5 + 1.0` (minimum 1.0), chosen so adjacent layers don't visually overlap.

---

## Loader (`loader.rs`)

Two input sources:

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

---

## Spawning (`spawn.rs`)

Three startup systems:

1. **`spawn_camera`** — `Camera3d` + `PanOrbitCamera`. Focus at centroid of all nodes; initial position at `focus + (0, 10, 30)`.
2. **`spawn_lights`** — Ambient (brightness 300) + directional (illuminance 8000, no shadows, from (50, 100, 50)).
3. **`spawn_node_spheres`** — One sphere per node using a shared mesh asset (`SphereMeshHandle`). Radius = `z_separation * 0.05` (min 0.1). Color = HSL(`layer_id / num_layers * 360`, 0.75, 0.55) via `LayerMaterials`. Each sphere carries `NodeSphere`, `LayerTag`, `VisPos`, `Visibility`.

---

## Render Systems (`render/`)

### `nodes.rs` — 2 systems

**`update_node_visibility`** — Combines layer visibility and active-set logic into a single binary `Visibility` toggle. A node is visible iff its layer is visible AND (no state is active OR the node is in the active set). Active set priority: query results > hover subtree > selection subtree. Runs only when `HoverState`, `SelectionState`, `QueryPanelState`, or `VisibilityToggles` change.

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
- "Z Separation" logarithmic slider (0.1 – 500.0); changing it immediately recomputes layout and updates all sphere transforms in place (no scene rebuild)
- "Node Radius" logarithmic slider (0.01 – 10.0); changing it replaces the shared sphere mesh asset in place so all spheres resize immediately

### `props.rs` — Floating properties window

Appears when any node is hovered. Positioned right of the layer panel with a 16px gap, constrained to remaining screen area. Sections (each omitted if empty):

1. Identity grid: Layer ID, Node ID, Parent ID
2. Coordinates: x, y, z (3-decimal)
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

Update tick:
  update_edge_nesting_visibility
  rebuild_edge_highlight
  rebuild_nesting_highlight
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

Y-up, `glam::Vec3`. The visual z-axis is overloaded for layer separation — do not interpret `VisPos.z` as a raw scene coordinate.
