/// Benchmarks for the two field-of-view queries, [`SceneGraph::visible_subgraph`] and
/// [`SceneGraph::visible_in_subtree`].
///
/// Each benchmark builds a scene graph whose nodes form a single tree hanging off one
/// root node on the top layer, gives every node on every layer a coordinate, and then
/// times the query issued at that root.
///
/// Three dimensions are swept, and both queries are measured against the very same
/// scenes in each of them so their costs are directly comparable:
///
/// - `node_count` — a fixed depth, widening tree. Shows how cost grows with scene size.
/// - `depth` — a roughly fixed node count spread over 2 to 10 layers. Shows how cost
///   responds to hierarchy depth alone.
/// - `visible_ratio` — a fixed tree, varying how much of it the observer can see. Shows
///   how cost responds to how much survives pruning.
///
/// [`SceneGraph::subgraph`] is measured over the `node_count` shapes as well.
/// `visible_subgraph` starts by building that subgraph, so the gap between
/// `subgraph/node_count` and `visible_subgraph/node_count` is what the field-of-view
/// filtering itself costs.
///
/// Note that the two queries answer different questions — `visible_subgraph` returns a
/// whole sub-scenegraph including the ancestors of every visible node, while
/// `visible_in_subtree` returns a flat list of the visible node ids — so their timings
/// are not interchangeable. What they share is the input scene and the observer.
///
/// Every scene is generated from a fixed seed by a PRNG defined in this file, so the
/// same shape yields byte-for-byte the same graph on every run and across the two
/// implementations being compared.
///
/// To compare an old and a new implementation:
///
/// ```text
/// # on the current implementation
/// cargo bench -p scene-graph --bench visible_subgraph -- --save-baseline before
/// # after rewriting the query
/// cargo bench -p scene-graph --bench visible_subgraph -- --baseline before
/// ```
///
/// The second run prints the percentage change per benchmark along with whether the
/// change is statistically significant.
use std::f32::consts::{PI, TAU};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use scene_graph::{Coordinate, Feature, NodeId, Observer, Result, SceneGraph};

/// Half-angle of the observer's cone, in radians.
const HALF_ANGLE: f32 = 35.0 * PI / 180.0;

/// Near plane of the observer's frustum. Nodes closer than this are not observed.
const NEAR: f32 = 0.6;

/// Far plane of the observer's frustum. Nodes farther than this are not observed.
const FAR: f32 = 6.0;

/// Seed for the scene generator. Fixed so every run produces the same scenes.
const SEED: u64 = 0x5CE7_EC4A;

/// The observer every benchmark queries with: at the origin, looking along `+Z`.
fn observer() -> Observer {
    Observer::new_looking_along(Coordinate::ZERO, Coordinate::Z, HALF_ANGLE, NEAR, FAR)
        .expect("+Z is a valid viewing axis")
}

/// A deterministic `splitmix64` PRNG.
///
/// Rolled by hand rather than pulled from `rand` so that the generated scenes stay
/// identical regardless of the `rand` version in the lockfile — two benchmark runs being
/// compared must see the exact same graph for the comparison to mean anything.
struct Rng(u64);

impl Rng {
    /// Start a new stream from `seed`.
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Draw the next 64 raw bits.
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Draw a `f32` uniformly from `[0, 1)`.
    fn unit(&mut self) -> f32 {
        // 24 bits is the full mantissa of an f32, so this covers the range evenly.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Draw a `f32` uniformly from `[lo, hi)`.
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// Where a generated coordinate sits relative to the observer's cone frustum.
#[derive(Clone, Copy)]
enum Placement {
    /// Inside the cone, so the node is observed.
    Inside,
    /// Within reach but outside the cone's angle, so the angular test rejects it.
    OffAxis,
    /// Aimed down the cone's axis but past the far plane, so the range test rejects it.
    TooFar,
}

/// Generate a coordinate with the requested relationship to the observer's cone.
///
/// The observer sits at the origin looking along `+Z`, so spherical coordinates about
/// `+Z` map straight into world space and `theta` is exactly the angle off the viewing
/// axis. Values are kept 5% clear of `HALF_ANGLE`, `NEAR` and `FAR` so float error in
/// [`Observer::observers`] cannot flip a node to the other side of the boundary.
fn coordinate(rng: &mut Rng, placement: Placement) -> Coordinate {
    let (theta, r) = match placement {
        Placement::Inside => (
            rng.range(0.0, 0.95 * HALF_ANGLE),
            rng.range(1.05 * NEAR, 0.95 * FAR),
        ),
        Placement::OffAxis => (
            rng.range(1.05 * HALF_ANGLE, PI),
            rng.range(1.05 * NEAR, 0.95 * FAR),
        ),
        Placement::TooFar => (
            rng.range(0.0, 0.95 * HALF_ANGLE),
            rng.range(1.05 * FAR, 5.0 * FAR),
        ),
    };
    let phi = rng.range(0.0, TAU);
    Coordinate::new(
        r * theta.sin() * phi.cos(),
        r * theta.sin() * phi.sin(),
        r * theta.cos(),
    )
}

/// The shape of a generated benchmark scene.
#[derive(Clone, Copy)]
struct Shape {
    /// Number of layers, which is also the number of levels in the node tree.
    /// Layer `0` holds the leaves; layer `depth - 1` holds the single root node.
    depth: u32,
    /// Number of children every non-leaf node is given on the layer below it.
    branching: u32,
    /// Fraction of the nodes on every layer whose coordinates land inside the
    /// observer's cone.
    visible_ratio: f32,
}

impl Shape {
    /// Total number of nodes the shape generates: `1 + b + b^2 + ... + b^(depth-1)`.
    fn node_count(&self) -> u64 {
        (0..self.depth)
            .map(|level| (self.branching as u64).pow(level))
            .sum()
    }
}

/// Create one coordinate-bearing node on layer `lid` and return its id.
///
/// Every node gets a coordinate, and `visible_ratio` of them land inside the cone. The
/// rest are split evenly between the two ways of being invisible, so both the angular
/// and the range rejection paths are exercised.
fn push_node(sg: &mut SceneGraph, lid: u32, rng: &mut Rng, visible_ratio: f32) -> Result<NodeId> {
    let placement = if rng.unit() < visible_ratio {
        Placement::Inside
    } else if rng.unit() < 0.5 {
        Placement::OffAxis
    } else {
        Placement::TooFar
    };
    let c = coordinate(rng, placement);
    let node = sg.new_coordinates(c.x, c.y, c.z, vec![Feature::new("layer", lid)]);
    Ok(sg.layer_mut(lid)?.push_node(node))
}

/// Build a scene graph of `shape` and return it alongside the id of its root node.
///
/// Layer `l` holds exactly the nodes at tree level `depth - 1 - l`, and every node other
/// than the root is nested under a node on the layer above it, so the whole graph sits
/// directly or indirectly below the returned root and the query can be issued there.
///
/// Siblings are additionally wired into a ring, one intra-layer edge per node, so the
/// benchmark pays the edge-pruning cost that a real scene's connectivity would impose.
fn scene(shape: Shape) -> Result<(SceneGraph, NodeId)> {
    let mut rng = Rng::new(SEED);
    let mut sg = SceneGraph::default();

    // `add_child` only accepts a lower id, so layer 0 is the leaf layer and each layer
    // parents the one directly beneath it.
    for _ in 0..shape.depth {
        sg.new_layer();
    }
    for lid in 1..shape.depth {
        sg.layer_mut(lid)?.add_child(lid - 1)?;
    }

    let root_layer = shape.depth - 1;
    let root = push_node(&mut sg, root_layer, &mut rng, shape.visible_ratio)?;

    // Grow the tree top-down and nest each node the moment it is created, while its own
    // subtree is still empty. That keeps `nest`'s index and bounds bookkeeping O(depth)
    // per node instead of re-walking a finished subtree.
    let mut level = vec![root];
    for lid in (0..root_layer).rev() {
        let mut next = Vec::with_capacity(level.len() * shape.branching as usize);
        for parent in &level {
            let first = next.len();
            for _ in 0..shape.branching {
                let child = push_node(&mut sg, lid, &mut rng, shape.visible_ratio)?;
                sg.nest(child).under(*parent)?;
                next.push(child);
            }
            let siblings = &next[first..];
            for (i, &src) in siblings.iter().enumerate() {
                let dst = siblings[(i + 1) % siblings.len()];
                sg.layer_mut(lid)?.add_edge(src, dst, "next to")?;
            }
        }
        level = next;
    }

    Ok((sg, root))
}

/// Count the nodes a scene graph holds across all of its layers.
fn total_nodes(sg: &SceneGraph) -> u64 {
    sg.layers().map(|l| l.all_nodes().len() as u64).sum()
}

/// Build a scene and confirm it really exercises the query before it is timed.
///
/// Without this a shape that silently generated an empty tree, or one where nothing at
/// all survives pruning, would still produce a plausible-looking benchmark number.
fn checked_scene(shape: Shape, observer: &Observer) -> (SceneGraph, NodeId) {
    let (sg, root) = scene(shape).expect("scene generation should not fail");

    let total = total_nodes(&sg);
    assert_eq!(
        total,
        shape.node_count(),
        "generated tree has {total} nodes, expected {}",
        shape.node_count()
    );

    let visible = sg
        .visible_subgraph(observer, root)
        .expect("visible_subgraph should not fail");
    assert_visible_count("visible_subgraph", total_nodes(&visible), total, shape);

    let visible = sg.visible_in_subtree(observer, root);
    assert_visible_count("visible_in_subtree", visible.len() as u64, total, shape);

    (sg, root)
}

/// Assert that the number of nodes `query` reported as visible is consistent with the
/// shape's `visible_ratio`.
///
/// The two queries return different things — a sub-scenegraph including the ancestors of
/// every visible node, versus a flat list of the visible nodes themselves — but both are
/// bounded the same way: empty when nothing is in view, everything when the whole scene
/// is, and strictly in between otherwise.
fn assert_visible_count(query: &str, count: u64, total: u64, shape: Shape) {
    if shape.visible_ratio == 0.0 {
        assert_eq!(
            count, 0,
            "{query}: nothing is inside the cone, yet {count} nodes survived"
        );
    } else {
        assert!(
            count > 0,
            "{query}: {:.0}% of the scene is inside the cone, yet nothing survived",
            shape.visible_ratio * 100.0
        );
    }
    if shape.visible_ratio == 1.0 {
        assert_eq!(
            count, total,
            "{query}: the whole scene is inside the cone, yet only {count} of {total} survived"
        );
    } else {
        assert!(
            count < total,
            "{query}: part of the scene is outside the cone, yet all {total} nodes survived"
        );
    }
}

/// Default fraction of visible nodes, used by every sweep that is not varying it.
const DEFAULT_RATIO: f32 = 0.5;

/// The node-count sweep: a fixed 4-layer hierarchy that grows wider, at 85, 585, 1885
/// and 4369 nodes.
fn width_shapes() -> impl Iterator<Item = Shape> {
    [4, 8, 12, 16].into_iter().map(|branching| Shape {
        depth: 4,
        branching,
        visible_ratio: DEFAULT_RATIO,
    })
}

/// The depth sweep: 2 to 10 layers, every shape landing within 1001 to 1365 nodes so
/// that depth is the only dimension that meaningfully varies.
fn depth_shapes() -> impl Iterator<Item = Shape> {
    [(2, 1000), (4, 10), (6, 4), (10, 2)]
        .into_iter()
        .map(|(depth, branching)| Shape {
            depth,
            branching,
            visible_ratio: DEFAULT_RATIO,
        })
}

/// The visibility sweep: a fixed 1111-node tree, varying how much of it is in view.
fn ratio_shapes() -> impl Iterator<Item = Shape> {
    [0.0, 0.1, 0.5, 1.0].into_iter().map(|visible_ratio| Shape {
        depth: 4,
        branching: 10,
        visible_ratio,
    })
}

/// Time `routine` once per shape in `shapes`, under the criterion group `group_name`.
///
/// Scene construction and validation happen outside the timed loop, and `id` names each
/// case within the group. Every query goes through here so that the ones being compared
/// are guaranteed to see the same scenes, the same observer and the same throughput
/// basis — the comparison is only meaningful if nothing but the query itself differs.
fn sweep<P, R, O>(
    c: &mut Criterion,
    group_name: &str,
    shapes: impl IntoIterator<Item = Shape>,
    id: impl Fn(Shape) -> P,
    routine: R,
) where
    P: std::fmt::Display,
    R: Fn(&SceneGraph, NodeId, &Observer) -> O,
{
    let observer = observer();
    let mut group = c.benchmark_group(group_name);

    for shape in shapes {
        let (sg, root) = checked_scene(shape, &observer);

        group.throughput(Throughput::Elements(shape.node_count()));
        group.bench_with_input(
            BenchmarkId::from_parameter(id(shape)),
            &(sg, root),
            |b, (sg, root)| b.iter(|| routine(sg, *root, &observer)),
        );
    }

    group.finish();
}

/// How each query scales with scene size at a fixed hierarchy depth.
///
/// `subgraph` is measured here too: `visible_subgraph` builds that subgraph first and
/// then filters it, so the gap between those two groups is what the field-of-view
/// filtering itself costs.
fn bench_node_count(c: &mut Criterion) {
    let by_count = |s: Shape| s.node_count();
    sweep(
        c,
        "visible_subgraph/node_count",
        width_shapes(),
        by_count,
        |sg, root, observer| sg.visible_subgraph(observer, root).unwrap(),
    );
    sweep(
        c,
        "visible_in_subtree/node_count",
        width_shapes(),
        by_count,
        |sg, root, observer| sg.visible_in_subtree(observer, root),
    );
    sweep(
        c,
        "subgraph/node_count",
        width_shapes(),
        by_count,
        |sg, root, _| sg.subgraph(root).unwrap(),
    );
}

/// How each query responds to hierarchy depth at a roughly fixed node count.
fn bench_depth(c: &mut Criterion) {
    let by_depth = |s: Shape| s.depth;
    sweep(
        c,
        "visible_subgraph/depth",
        depth_shapes(),
        by_depth,
        |sg, root, observer| sg.visible_subgraph(observer, root).unwrap(),
    );
    sweep(
        c,
        "visible_in_subtree/depth",
        depth_shapes(),
        by_depth,
        |sg, root, observer| sg.visible_in_subtree(observer, root),
    );
}

/// How each query responds to the share of the scene that survives pruning.
fn bench_visible_ratio(c: &mut Criterion) {
    let by_ratio = |s: Shape| format!("{:.0}%", s.visible_ratio * 100.0);
    sweep(
        c,
        "visible_subgraph/visible_ratio",
        ratio_shapes(),
        by_ratio,
        |sg, root, observer| sg.visible_subgraph(observer, root).unwrap(),
    );
    sweep(
        c,
        "visible_in_subtree/visible_ratio",
        ratio_shapes(),
        by_ratio,
        |sg, root, observer| sg.visible_in_subtree(observer, root),
    );
}

criterion_group!(benches, bench_node_count, bench_depth, bench_visible_ratio,);
criterion_main!(benches);
