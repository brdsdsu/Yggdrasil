# spatial-index

A vendored fork of [kiddo](https://github.com/sdd/kiddo), carrying a k-d tree that chooses
split dimensions adaptively instead of by cycling with tree depth. `scene-graph` uses it to
back the optional position indexes on layers and nodes.

Upstream's own README is preserved as [`README.upstream.md`](README.upstream.md).

## Provenance

| | |
| --- | --- |
| Upstream | `kiddo` 6.0.0-alpha.3, published 2026-07-21 |
| Source | crates.io registry tarball, unpacked verbatim |
| SHA-256 of `kiddo-6.0.0-alpha.3.crate` | `a2925679f5e7ef437e4ee1afb561b77b3eb5aa1b032c0bbe7034b78b3d3e5214` |
| Repository | <https://github.com/sdd/kiddo> |
| Licence | MIT OR Apache-2.0 — unchanged, see `LICENSE-MIT` and `LICENSE-APACHE` |
| Author | Scott Donnelly \<scott@donnel.ly\> |
| Upstream MSRV | 1.89.0 |

Note that 6.0.0-alpha.3 is a **pre-release**; there is no stable 6.x. kiddo 6 is a near-total
rewrite of 5.x, and the API moved between alphas, so expect rebases to be real work.

## Why fork rather than depend

The fix has to change how a full leaf picks the axis it splits on, and that decision lives in
`StemStrategy`. That trait cannot be implemented outside the crate: its associated types are
bounded by `kd_tree::query_stack::{ScalarStackContext, StackTrait}`, and `query_stack` is
`pub(crate)`, so the types a third-party implementation would have to name are unnameable.
Making those types public would turn this whole fork into an ordinary downstream crate, and is
worth proposing upstream on its own.

## Deviations from upstream

The source tree is upstream's byte for byte — `src/`, `tests/`, `benches/`, `examples/`,
`build.rs`, both licences — with these exceptions. Every source deviation carries a
`FORK DEVIATION` comment at the site, so `git grep 'FORK DEVIATION'` lists them all.

### 1. `Cargo.toml` — rewritten, cannot be verbatim

* Package renamed to `spatial-index` at version `0.1.0`.
* The `[lib]` name stays `kiddo`, so upstream's tests and doctests compile unmodified.
  Dependents rename it at the dependency edge instead:
  `spatial_index = { package = "spatial-index", path = "../spatial-index" }`.
* Upstream's `[profile.bench]`, `[profile.fast-tests]` and `[profile.release]` are dropped —
  a workspace member may not define profiles.
* Upstream's `[[example]]` and `[[bench]]` targets are dropped, along with the
  dev-dependencies only they need (`criterion`, `codspeed-criterion-compat`, `las`, `reqwest`,
  `flate2`, `memmap`, `zip`, `postcard`, `serde_json`, `radians`, `rand_distr`, `itertools`,
  `elapsed`, `ubyte`, `proc-macro2`, `tracing-subscriber`). Several download geonames data or
  build multi-gigabyte archives. The files remain on disk, just undeclared.
* Upstream's seven `[[test]]` targets are kept, as a regression net for the patch, with the
  dev-dependencies they actually need (`assert_float_eq`, `rand`, `rand_chacha`, `rstest`,
  `trybuild`, `zzz`).
* `[features]`, `[dependencies]`, `[build-dependencies]`, the target-specific `generator`
  dependency and `[lints]` are copied verbatim. Default features are unchanged (`fixed`).

### 2. `.cargo/config.toml` — deleted

Upstream ships one containing:

```toml
[build]
rustflags = ["-Ctarget-cpu=native"]
rustdocflags = ["-Ctarget-cpu=native"]
```

That is upstream's own benchmarking environment, and it is actively harmful inside a workspace:
cargo picks up the nearest `.cargo/config.toml` relative to the *invocation* directory, so any
`cargo` command run from inside `spatial-index/` would silently rebuild the entire workspace
with `-Ctarget-cpu=native` — different flags from a root invocation, so a full cache
invalidation each time you switch directories, and binaries that may fault on other CPUs.
Nothing in the source depends on it.

### 3. `#[derive(Clone, Debug, PartialEq)]` on `VecOfArrays`

`src/leaf_strategy/vec_of_arrays.rs`. Upstream declares none of the three. Because a derived
impl is bounded on every type parameter, and `KdTree` derives `Clone, Debug, PartialEq`, the
absence makes those derives unreachable for *any* mutable tree — `VecOfArrays` is the only
`Mutable` leaf strategy. `LeafNode`, its own field type, already derives all three. This reads
as an omission, not a decision, and is the third thing worth reporting upstream.

Without it `scene-graph` cannot derive `Clone`/`Debug` on `Layer` or `Node`, which
`SceneGraph: Clone` depends on.

### 4. Adaptive split-dimension selection

The substantive change, documented in [`ADAPTIVE_SPLIT.md`](ADAPTIVE_SPLIT.md): a new
`EytzingerAdaptive` stem strategy that picks a full leaf's split axis by widest spread and records
it in the low bits of the split value, rather than deriving it from tree depth. Existing
strategies are unaffected — the two trait hooks it needs default to current behaviour, and the
policy is gated behind an opt-in associated const.

### 5. Adaptive bulk construction

Bulk construction was originally refused for adaptive strategies with
`ConstructionError::AdaptiveSplitBulkBuildUnsupported`, because the builder partitioned around a
pivot *before* writing it and so could not record a dimension in that value without moving the
plane off its own partition. `KdTree::choose_split` now picks a grid-aligned separator before the
value is stored — so plane and partition agree — and selects the axis by widest spread, falling
through to another axis when one admits no separator. Cycling strategies take an early return that
is upstream's behaviour verbatim. The error variant is retained for compatibility but is no longer
produced. See [`ADAPTIVE_SPLIT.md`](ADAPTIVE_SPLIT.md), "Bulk construction, adapted".

This matters because bulk construction is immune to the insertion-order defect below: it partitions
from the whole point set rather than one point at a time.

## Baseline

Upstream's suite passes unmodified in this fork, and is the reference to re-run after every
change to the split policy:

```
cargo test -p spatial-index                     # 397 lib + 13 integration + 7 doctests
cargo test -p spatial-index --features fuzz     # adds the randomised kd_tree_fuzz_v6 target
```

## Known upstream defects this fork exists to address

1. **Coplanar input cannot be inserted.** Split dimensions cycle with depth, so a full leaf
   whose points all share one value on the cycled axis has no pivot and the insert is refused
   with `ConstructionError::UnsplittableBucket`. Voxel-quantized indoor geometry puts thousands
   of points on one plane, far past a 128-point bucket. Characterized by
   `scene-graph/src/pos.rs::coplanar_geometry`.
   **Fixed** by deviation 4 below; verified in `bench/BASELINE.md`.
2. **Ordered incremental inserts consume exponential memory.** Stems live in a flat array
   addressed by breadth-first position, so `stems.resize(stem_idx + 1, ..)` and
   `leaf_idx_map.resize(right_child_idx + 1, ..)` cost `~2^depth` slots. Spatially clustered
   insertion order deepens one path instead of broadening the tree: 528,826 real points in file
   order exceed 6 GB and get OOM-killed, while the *same* points shuffled build in 157 MB and
   291 ms. Adaptive split selection does **not** bound this: measured post-patch, the fork still exceeds a
   1 GiB budget on clustered insertion order, marginally sooner than upstream.

   Still unfixed *for incremental insertion*, but no longer without a remedy: **bulk construction
   now works for adaptive strategies** (deviation 5 below), and it is immune to insertion order
   because it partitions by median from the whole point set. On the benchmark corpus the same
   354,885 points cost 163,842 stem slots at depth 17 bulk-built, against 15,203,841 slots at
   depth 23 incrementally in mesh order — and the incremental build never completes. A caller
   holding an index that has degenerated can rebuild it; a `created/used` slot ratio separates
   healthy from pathological by four orders of magnitude (2.4-3.9 against 112,000) and makes a
   cheap trigger. Note that the timing cost of the defect has the same single cause as the memory
   cost — `A::max_value()` is infinity for `f32`, so the resize fill is non-zero and every new
   slot is physically written rather than served as a zero page, which is why wall-clock and RSS
   move in lockstep.

   Also still true: bulk construction trips a `left_capacity is zero` debug assertion whenever the
   leaf count is not a power of two (16/32/64/128 build; 54 and 94 panic). Independent of the
   adaptive work — it reproduces with cycling strategies — and it is a `debug_assert`, so release
   builds pass it. Pinned by the ignored test
   `construction::tests::bulk_build_panics_on_non_power_of_two_leaf_count`.
