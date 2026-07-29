//! A/B benchmark harness for the vendored k-d tree against upstream kiddo.
//!
//! Both implementations are linked into this one binary and measured in the same run on the
//! same inputs, so a comparison never depends on remembered numbers from an earlier session.
//! Today `spatial-index` is behaviourally identical to upstream, so every delta should read
//! as noise — that is the baseline, and it doubles as a check that the harness is fair.
//!
//! ```text
//! cargo run --release -p bench                       # synthetic cases
//! cargo run --release -p bench -- --data path.f32    # adds a real point cloud
//! cargo run --release -p bench -- --json             # machine-readable, for diffing later
//! ```
//!
//! Points come from a xorshift PRNG defined here rather than from the `rand` crate, so inputs
//! are reproducible across machines and immune to `rand` version churn.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

/// Bucket size under test. Mirrors `scene-graph`'s position index, which is the configuration
/// the adaptive-split work must not change.
const B: usize = 128;

/// Item width mirrors `scene-graph`'s `NodeId`, so leaf memory matches production.
type Item = u64;

/// Memory a single build is allowed before the harness gives up on it, overridable with
/// `--budget <GiB>`.
///
/// Clustered insertion order makes stem storage grow with `2^depth`, which on real data means
/// the process is killed by the OOM killer rather than returning a result. Watching the
/// allocator and bailing out converts that from a dead harness into a measurement: how many
/// points went in before the tree needed more than this. The default is deliberately low so a
/// plain run is quick and safe; raise it to measure queries on inputs that need more.
static BUILD_MEMORY_BUDGET: AtomicUsize = AtomicUsize::new(1 << 30);

fn budget() -> usize {
    BUILD_MEMORY_BUDGET.load(Relaxed)
}

// ---------------------------------------------------------------------------------------
// Allocation accounting
// ---------------------------------------------------------------------------------------

/// Wraps the system allocator to track live and peak bytes.
///
/// Peak allocation is a better A/B signal than peak RSS: it is per-case rather than a
/// high-water mark for the whole process, and it does not depend on when the OS reclaims
/// pages.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(live, Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if new_size >= layout.size() {
            let grown = new_size - layout.size();
            let live = LIVE.fetch_add(grown, Relaxed) + grown;
            PEAK.fetch_max(live, Relaxed);
        } else {
            LIVE.fetch_sub(layout.size() - new_size, Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Live bytes currently allocated.
fn live() -> usize {
    LIVE.load(Relaxed)
}

/// Drop the recorded peak to the current live total, so the next measurement starts fresh.
fn reset_peak() {
    PEAK.store(LIVE.load(Relaxed), Relaxed);
}

/// Bytes allocated at the high-water mark since [`reset_peak`], net of what was already live.
fn peak_since(baseline: usize) -> usize {
    PEAK.load(Relaxed).saturating_sub(baseline)
}

// ---------------------------------------------------------------------------------------
// Deterministic input generation
// ---------------------------------------------------------------------------------------

/// xorshift64*, seeded per dataset so every run sees identical points.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `[0, 1)`, from the top 24 bits so every value is exactly representable.
    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / (1u32 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.next_f32() * (hi - lo)
    }
}

/// Uniform points in an `extent`-metre cube.
fn uniform(n: usize, extent: f32, seed: u64) -> Vec<[f32; 3]> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            [
                rng.range(0.0, extent),
                rng.range(0.0, extent),
                rng.range(0.0, extent),
            ]
        })
        .collect()
}

/// Points grouped into tight clusters and emitted cluster by cluster, mirroring how a
/// ConceptGraph dump lists one object's point cloud at a time. This is the insertion order
/// that made upstream consume exponential memory on real data.
fn clustered(n: usize, clusters: usize, extent: f32, seed: u64) -> Vec<[f32; 3]> {
    let mut rng = Rng::new(seed);
    let mut out = Vec::with_capacity(n);
    for _ in 0..clusters {
        let c = [
            rng.range(0.0, extent),
            rng.range(0.0, extent),
            rng.range(0.0, extent),
        ];
        for _ in 0..n / clusters {
            out.push([
                c[0] + rng.range(-0.25, 0.25),
                c[1] + rng.range(-0.25, 0.25),
                c[2] + rng.range(-0.25, 0.25),
            ]);
        }
    }
    out
}

/// Uniform points snapped to a lattice: the shape reconstructed indoor geometry takes, and the
/// one upstream refuses outright.
fn lattice(n: usize, extent: f32, grid: f32, seed: u64) -> Vec<[f32; 3]> {
    uniform(n, extent, seed)
        .into_iter()
        .map(|p| p.map(|c| (c / grid).round() * grid))
        .collect()
}

/// Points on the six faces of a room, snapped to a lattice.
///
/// A uniform cloud snapped to the same lattice does *not* reproduce the coplanar defect: with
/// points spread through a volume, a 128-point bucket almost never ends up entirely on one
/// plane. Real reconstructed geometry is not volumetric — it is surfaces, so each plane holds
/// thousands of points and a bucket easily fills with them. That is the shape that breaks.
fn surfaces(n: usize, extent: f32, grid: f32, seed: u64) -> Vec<[f32; 3]> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let mut p = [
                rng.range(0.0, extent),
                rng.range(0.0, extent),
                rng.range(0.0, extent),
            ];
            // Pin one coordinate to a face, leaving the other two free.
            let face = (rng.next_u64() % 6) as usize;
            p[face / 2] = if face % 2 == 0 { 0.0 } else { extent };
            p.map(|c| (c / grid).round() * grid)
        })
        .collect()
}

/// Raw little-endian `[f32; 3]` triples, as exported from a ConceptGraph dump.
fn load(path: &str) -> Vec<[f32; 3]> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    bytes
        .chunks_exact(12)
        .map(|c| {
            let f = |i: usize| f32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
            [f(0), f(4), f(8)]
        })
        .collect()
}

/// Radius enclosing roughly `target` neighbours, derived from point density in the input's
/// bounding box, so radius queries stay comparable across datasets of different scale.
fn radius_for(points: &[[f32; 3]], target: f64) -> f32 {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in points {
        for d in 0..3 {
            lo[d] = lo[d].min(p[d]);
            hi[d] = hi[d].max(p[d]);
        }
    }
    let volume: f64 = (0..3).map(|d| (hi[d] - lo[d]).max(1e-6) as f64).product();
    let density = points.len() as f64 / volume;
    ((3.0 * target) / (4.0 * std::f64::consts::PI * density)).cbrt() as f32
}

// ---------------------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------------------

/// One measured number, plus what it took to get it.
#[derive(Clone, Copy)]
struct Measure {
    /// Median wall time across repetitions.
    time: Duration,
    /// Peak bytes allocated while it ran.
    bytes: usize,
    /// Operations performed, for per-op normalisation.
    ops: usize,
}

impl Measure {
    fn per_op_ns(&self) -> f64 {
        self.time.as_nanos() as f64 / self.ops.max(1) as f64
    }
}

/// How a build ended.
enum BuildResult {
    /// Every point went in.
    Done(Measure),
    /// The tree refused a point — the coplanar-split defect — after accepting this many.
    Refused(usize),
    /// The build blew through [`BUILD_MEMORY_BUDGET`] after accepting this many points,
    /// having reached `bytes` live. Without the budget this is a SIGKILL.
    Exceeded { after: usize, bytes: usize },
}

impl BuildResult {
    fn measure(&self) -> Option<&Measure> {
        match self {
            Self::Done(m) => Some(m),
            _ => None,
        }
    }
}

/// Shape of a built tree, for separating "how deep" from "how big".
#[derive(Clone, Copy)]
struct Shape {
    /// Deepest interior stem level reached.
    depth: i32,
    /// Slots in the stem array — sized by the deepest path, not the node count.
    stem_slots: usize,
    /// Leaves a perfectly balanced tree would need for this many points.
    leaves: usize,
}

impl Shape {
    /// Depth a balanced tree over `leaves` leaves would have.
    fn ideal_depth(&self) -> u32 {
        self.leaves.max(1).next_power_of_two().ilog2()
    }
}

/// What one implementation did with one dataset.
struct Outcome {
    build: BuildResult,
    shape: Option<Shape>,
    queries: Vec<(&'static str, Measure)>,
    remove: Option<Measure>,
    bulk: Result<Measure, String>,
    /// Tree size after the removal pass, as a cross-check that both impls agree.
    size: usize,
}

/// Minimum, not mean or median: for latency microbenchmarks the fastest observed run is the
/// one least contaminated by cache misses, page faults and scheduler noise, so it is the
/// stable estimator to compare across implementations.
fn best(times: Vec<Duration>) -> Duration {
    times.into_iter().min().expect("at least one repetition")
}

/// Generates the measurement body for one crate. Both crates expose the same API, so the body
/// is identical apart from the paths it resolves through — which is what has to stay true for
/// an A/B to mean anything.
macro_rules! measure_with {
    ($lib:ident, $ss:ident) => {{
        type Tree =
            $lib::KdTree<f32, Item, $lib::$ss, $lib::VecOfArrays<f32, Item, 3, B>, 3, B>;

        fn run(points: &[[f32; 3]], queries: &[[f32; 3]], radius: f32, reps: usize) -> Outcome {
            // --- incremental build ---------------------------------------------------
            // Built `reps` times, keeping the fastest. The first pass pays for first-touch
            // page faults on freshly mapped memory; without repeating, whichever
            // implementation happens to run first absorbs that cost and looks slower.
            let mut build_times = Vec::new();
            let mut build_bytes = 0usize;
            let mut accepted = 0usize;
            let mut stopped;

            // Each repetition's tree is dropped before the next one starts. Keeping them
            // alive would put two trees in the heap at once, which on the pathological
            // inputs is the difference between a measurement and an OOM kill.
            for _ in 0..reps {
                let base = live();
                reset_peak();
                let started = Instant::now();
                let mut t = Tree::default();
                accepted = 0;
                stopped = None;
                for (i, p) in points.iter().enumerate() {
                    if t.add(p, i as Item).is_err() {
                        stopped = Some(BuildResult::Refused(accepted));
                        break;
                    }
                    accepted += 1;
                    // Polled rather than checked every insert: the check is cheap but the
                    // insert is cheaper, and 1024 points cannot move the total far.
                    if accepted % 1024 == 0 && live().saturating_sub(base) > budget() {
                        stopped = Some(BuildResult::Exceeded {
                            after: accepted,
                            bytes: live().saturating_sub(base),
                        });
                        break;
                    }
                }
                build_times.push(started.elapsed());
                build_bytes = peak_since(base);
                let reached = t.size();
                drop(t);
                if stopped.is_some() {
                    return Outcome {
                        build: stopped.expect("checked"),
                        queries: Vec::new(),
                        remove: None,
                        shape: None,
                        bulk: Err("skipped: incremental build did not complete".into()),
                        size: reached,
                    };
                }
            }

            let build = BuildResult::Done(Measure {
                time: best(build_times),
                bytes: build_bytes,
                ops: accepted.max(1),
            });

            // One more build, this time kept, to query and remove against.
            let mut tree = Tree::default();
            for (i, p) in points.iter().enumerate() {
                tree.add(p, i as Item).expect("build already proved to succeed");
            }

            // Tree shape, read straight off the accessor. Stems are addressed by
            // breadth-first position, so the stem array is sized by the *deepest* path
            // rather than by the number of nodes actually in it: that gap is the whole
            // memory story, and this measures it instead of inferring it.
            let shape = Shape {
                depth: <Tree as $lib::kd_tree::KdTreeAccessor<_, _, _, _, 3, B>>::max_stem_level(
                    &tree,
                ),
                stem_slots: <Tree as $lib::kd_tree::KdTreeAccessor<_, _, _, _, 3, B>>::stems(&tree)
                    .len(),
                leaves: points.len().div_ceil(B),
            };

            // --- queries -------------------------------------------------------------
            let mut results = Vec::new();
            for (label, k) in [("nearest_1", 1usize), ("nearest_10", 10), ("nearest_100", 100)] {
                let n = std::num::NonZero::new(k).unwrap();
                let mut times = Vec::new();
                // One untimed warmup pass, so the timed passes measure steady state rather
                // than the cost of pulling the tree into cache for the first time.
                for rep in 0..=reps {
                    let t = Instant::now();
                    let mut sink = 0usize;
                    for q in queries {
                        sink += tree
                            .query(q)
                            .nearest_n::<$lib::SquaredEuclidean<f32>>(n)
                            .execute()
                            .len();
                    }
                    std::hint::black_box(sink);
                    if rep > 0 {
                        times.push(t.elapsed());
                    }
                }
                results.push((
                    label,
                    Measure {
                        time: best(times),
                        bytes: 0,
                        ops: queries.len(),
                    },
                ));
            }

            let mut times = Vec::new();
            for rep in 0..=reps {
                let t = Instant::now();
                let mut sink = 0usize;
                for q in queries {
                    sink += tree
                        .query(q)
                        .within::<$lib::SquaredEuclidean<f32>>(radius * radius)
                        .execute()
                        .len();
                }
                std::hint::black_box(sink);
                if rep > 0 {
                    times.push(t.elapsed());
                }
            }
            results.push((
                "within",
                Measure {
                    time: best(times),
                    bytes: 0,
                    ops: queries.len(),
                },
            ));

            // --- removal -------------------------------------------------------------
            // Every tenth point, so the walk touches leaves spread across the tree.
            let victims: Vec<_> = points.iter().enumerate().step_by(10).collect();
            let t = Instant::now();
            for (i, p) in &victims {
                tree.remove(p, *i as Item);
            }
            let remove = Measure {
                time: t.elapsed(),
                bytes: 0,
                ops: victims.len(),
            };
            let size = tree.size();
            drop(tree);

            // --- bulk build ----------------------------------------------------------
            // The other construction path. It selects split dimensions independently of the
            // incremental one, so it has to be patched — and measured — separately.
            let entries: Vec<(Item, [f32; 3])> =
                points.iter().enumerate().map(|(i, p)| (i as Item, *p)).collect();
            let mut bulk_times = Vec::new();
            let mut bulk_bytes = 0usize;
            let mut bulk_err = None;
            for _ in 0..reps {
                let base = live();
                reset_peak();
                let t = Instant::now();
                match Tree::new_from_entries(&entries) {
                    Ok(built) => {
                        bulk_times.push(t.elapsed());
                        bulk_bytes = peak_since(base);
                        std::hint::black_box(built.size());
                    }
                    Err(e) => {
                        bulk_err = Some(format!("{e}"));
                        break;
                    }
                }
            }
            let bulk = match bulk_err {
                Some(e) => Err(e),
                None => Ok(Measure {
                    time: best(bulk_times),
                    bytes: bulk_bytes,
                    ops: points.len(),
                }),
            };

            Outcome {
                build,
                shape: Some(shape),
                queries: results,
                remove: Some(remove),
                bulk,
                size,
            }
        }

        run
    }};
}

// ---------------------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------------------

/// Combine two runs of the same implementation, keeping the better of each measurement.
///
/// Whichever implementation runs first in a case pays for heap growth and instruction-cache
/// misses the second one then benefits from, which showed up as a systematic ~12% swing on
/// build and remove even with identical code. Each implementation is therefore run once in
/// each position and the positions merged here, so ordering cancels instead of being
/// mistaken for a regression.
fn merge_best(a: Outcome, b: Outcome) -> Outcome {
    let pick = |x: Measure, y: Measure| Measure {
        time: x.time.min(y.time),
        bytes: x.bytes.min(y.bytes),
        ops: x.ops,
    };

    Outcome {
        build: match (a.build, b.build) {
            (BuildResult::Done(x), BuildResult::Done(y)) => BuildResult::Done(pick(x, y)),
            // A build that did not complete is the finding, not a number to minimise.
            (other @ (BuildResult::Refused(_) | BuildResult::Exceeded { .. }), _) => other,
            (_, other) => other,
        },
        // Shape is a property of the tree, not of the run, so both positions agree.
        shape: a.shape.or(b.shape),
        queries: a
            .queries
            .into_iter()
            .zip(b.queries)
            .map(|((label, x), (_, y))| (label, pick(x, y)))
            .collect(),
        remove: match (a.remove, b.remove) {
            (Some(x), Some(y)) => Some(pick(x, y)),
            (x, y) => x.or(y),
        },
        bulk: match (a.bulk, b.bulk) {
            (Ok(x), Ok(y)) => Ok(pick(x, y)),
            (Err(e), _) | (_, Err(e)) => Err(e),
        },
        size: a.size,
    }
}

fn pct(fork: f64, upstream: f64) -> String {
    if upstream == 0.0 {
        return "      -".into();
    }
    format!("{:+6.1}%", (fork - upstream) / upstream * 100.0)
}

fn bytes(n: usize) -> String {
    const U: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < 3 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", U[i])
}

fn report(name: &str, n: usize, f: &Outcome, u: &Outcome, json: bool) {
    if json {
        print!("{{\"dataset\":\"{name}\",\"points\":{n}");
        let status = |b: &BuildResult| match b {
            BuildResult::Done(_) => "completed".to_string(),
            BuildResult::Refused(after) => format!("refused_after_{after}"),
            BuildResult::Exceeded { after, .. } => format!("exceeded_memory_after_{after}"),
        };
        print!(
            ",\"build_status\":{{\"fork\":\"{}\",\"upstream\":\"{}\"}}",
            status(&f.build),
            status(&u.build)
        );
        if let (Some(fb), Some(ub)) = (f.build.measure(), u.build.measure()) {
            print!(
                ",\"build_ns_per_insert\":{{\"fork\":{:.1},\"upstream\":{:.1}}},\
                 \"build_peak_bytes\":{{\"fork\":{},\"upstream\":{}}}",
                fb.per_op_ns(),
                ub.per_op_ns(),
                fb.bytes,
                ub.bytes
            );
        }
        for ((label, fm), (_, um)) in f.queries.iter().zip(u.queries.iter()) {
            print!(
                ",\"{label}_ns\":{{\"fork\":{:.1},\"upstream\":{:.1}}}",
                fm.per_op_ns(),
                um.per_op_ns()
            );
        }
        if let (Ok(fb), Ok(ub)) = (&f.bulk, &u.bulk) {
            print!(
                ",\"bulk_ns_per_point\":{{\"fork\":{:.1},\"upstream\":{:.1}}}",
                fb.per_op_ns(),
                ub.per_op_ns()
            );
        }
        println!("}}");
        return;
    }

    println!("\n=== {name}  ({n} points) ===");

    let describe = |b: &BuildResult| match b {
        BuildResult::Done(_) => "completed".to_string(),
        BuildResult::Refused(after) => format!("REFUSED after {after}"),
        BuildResult::Exceeded {
            after,
            bytes: used,
        } => format!("EXCEEDED {} after {after}", bytes(*used)),
    };

    match (&f.build, &u.build) {
        (BuildResult::Done(fb), BuildResult::Done(ub)) => {
            println!(
                "  {:<16} {:>10.1} {:>10.1} ns/insert {}",
                "build",
                fb.per_op_ns(),
                ub.per_op_ns(),
                pct(fb.per_op_ns(), ub.per_op_ns())
            );
            println!(
                "  {:<16} {:>10} {:>10}           {}",
                "build peak mem",
                bytes(fb.bytes),
                bytes(ub.bytes),
                pct(fb.bytes as f64, ub.bytes as f64)
            );
        }
        (fb, ub) => {
            println!("  build            fork: {}", describe(fb));
            println!("                   upstream: {}", describe(ub));
            if f.queries.is_empty() && u.queries.is_empty() {
                return;
            }
        }
    }

    if let (Some(fs), Some(us)) = (&f.shape, &u.shape) {
        println!(
            "  {:<16} {:>10} {:>10}  depth (balanced would be {})",
            "tree depth", fs.depth, us.depth, fs.ideal_depth()
        );
        println!(
            "  {:<16} {:>10} {:>10}  stem slots for {} leaves",
            "stem slots", fs.stem_slots, us.stem_slots, fs.leaves
        );
    }

    for ((label, fm), (_, um)) in f.queries.iter().zip(u.queries.iter()) {
        println!(
            "  {:<16} {:>10.0} {:>10.0} ns/query  {}",
            label,
            fm.per_op_ns(),
            um.per_op_ns(),
            pct(fm.per_op_ns(), um.per_op_ns())
        );
    }

    if let (Some(fr), Some(ur)) = (&f.remove, &u.remove) {
        println!(
            "  {:<16} {:>10.0} {:>10.0} ns/remove {}",
            "remove",
            fr.per_op_ns(),
            ur.per_op_ns(),
            pct(fr.per_op_ns(), ur.per_op_ns())
        );
    }

    match (&f.bulk, &u.bulk) {
        (Ok(fb), Ok(ub)) => {
            println!(
                "  {:<16} {:>10.1} {:>10.1} ns/point  {}",
                "bulk build",
                fb.per_op_ns(),
                ub.per_op_ns(),
                pct(fb.per_op_ns(), ub.per_op_ns())
            );
            println!(
                "  {:<16} {:>10} {:>10}           {}",
                "bulk peak mem",
                bytes(fb.bytes),
                bytes(ub.bytes),
                pct(fb.bytes as f64, ub.bytes as f64)
            );
        }
        // The fork refuses bulk construction with the adaptive cursor by design: that builder
        // partitions before it writes, so it cannot record a split dimension in the value without
        // shifting the plane off its own partition. `scene-graph` only ever inserts incrementally,
        // so nothing is lost - but it means the bulk row is not comparable and is labelled rather
        // than reported as a failure.
        (Err(fe), _) if fe.contains("not supported") => {
            println!("  bulk build       n/a for fork: adaptive splits require incremental insert")
        }
        (Err(fe), Err(_)) => println!("  bulk build       both failed: {fe}"),
        (Ok(_), Err(ue)) => println!("  bulk build       fork ok, upstream failed: {ue}"),
        (Err(fe), Ok(_)) => println!("  bulk build       fork failed: {fe}, upstream ok"),
    }

    if f.size != u.size {
        println!(
            "  !! size after removals differs: fork {} upstream {}",
            f.size, u.size
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let only = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let data = args
        .iter()
        .position(|a| a == "--data")
        .and_then(|i| args.get(i + 1))
        .cloned();
    let reps = 3;

    if let Some(gib) = args
        .iter()
        .position(|a| a == "--budget")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
    {
        BUILD_MEMORY_BUDGET.store((gib * (1u64 << 30) as f64) as usize, Relaxed);
    }

    // The fork under test uses the adaptive cursor; upstream has only the cycling one.
    let fork_run = measure_with!(fork, EytzingerAdaptive);
    let upstream_run = measure_with!(upstream, Eytzinger);

    let mut cases: Vec<(String, Vec<[f32; 3]>)> = vec![
        ("uniform-100k".into(), uniform(100_000, 10.0, 0xC0FFEE)),
        ("uniform-500k".into(), uniform(500_000, 10.0, 0xBEEF)),
        ("clustered-100k".into(), clustered(100_000, 80, 10.0, 0xFEED)),
        ("lattice-0.05-100k".into(), lattice(100_000, 10.0, 0.05, 0xD00D)),
        ("surfaces-0.05-100k".into(), surfaces(100_000, 10.0, 0.05, 0xD00D)),
    ];

    if let Some(path) = &data {
        let name = path.rsplit('/').next().unwrap_or("real");
        cases.push((format!("real:{name}"), load(path)));
    }

    if let Some(filter) = &only {
        cases.retain(|(name, _)| name.contains(filter.as_str()));
    }

    if !json {
        println!(
            "k-d tree A/B: vendored spatial-index vs crates.io kiddo 6.0.0-alpha.3\n\
             B = {B}, item = u64, {reps} reps per query set; columns are fork / upstream / delta.\n\
             Fork splits on the widest axis; upstream cycles the axis with depth."
        );
    }

    for (name, points) in &cases {
        // 2,000 query points sampled from the data itself, shared by both implementations.
        let queries: Vec<[f32; 3]> = {
            let mut rng = Rng::new(0x5EED);
            (0..2000)
                .map(|_| points[(rng.next_u64() as usize) % points.len()])
                .collect()
        };
        let radius = radius_for(points, 10.0);

        // Each implementation runs once first and once second; see `merge_best`.
        let f1 = fork_run(points, &queries, radius, reps);
        let u1 = upstream_run(points, &queries, radius, reps);
        let u2 = upstream_run(points, &queries, radius, reps);
        let f2 = fork_run(points, &queries, radius, reps);

        let f = merge_best(f1, f2);
        let u = merge_best(u1, u2);
        report(name, points.len(), &f, &u, json);
    }
}
