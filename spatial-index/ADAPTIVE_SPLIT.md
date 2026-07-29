# Adaptive split-dimension selection — design and remaining work

Status: **landed, incremental and bulk.** Encoding helpers, the grid separator, the
`EytzingerAdaptive` cursor and its trait hooks; widest-spread selection in
`VecOfArrays::split_leaf_adaptive` for incremental `add`; and `KdTree::choose_split` for the bulk
builders. `scene-graph`'s `PosIndex` names the cursor, so this workspace is on the adaptive path
throughout.

> **This file's older sections lag the code, deliberately.** The "Remaining — one coherent change"
> list is kept for its reasoning even though items 1, 2, 3, 5 and 6 are done, item 4 turned out to
> be the common case rather than the unreachable one it predicted, and item 8's A/B has been run.
> Only item 7 — folding `pos.rs::coplanar_defect` into acceptance tests — is genuinely outstanding.
> "Bulk construction is refused, not adapted" is no longer true; see "Bulk construction, adapted".
> `README.md` and `bench/BASELINE.md` are the current descriptions.

## The defect

Upstream derives a stem's split axis from tree depth (`dim = (dim + 1) % K` in
`stem_strategy/eytzinger/mod.rs::traverse`). Two consequences, both measured in
`bench/BASELINE.md`:

1. A full leaf whose points all share one value on the cycled axis has no pivot, so the insert
   is refused with `ConstructionError::UnsplittableBucket`. Voxel-quantised indoor geometry puts
   thousands of points on one plane. Characterised in `scene-graph/src/pos.rs::coplanar_defect`.
2. When the cycled axis has little spread inside the bucket, the split barely separates the
   points and the same region re-overflows, spending depth instead of dividing space. Stems live
   in a flat array addressed by breadth-first position, so depth costs `~2^depth` slots:
   real data reached depth 27 with 266,301,286 stem slots for 2,377 leaves, 5.0 GiB.

Choosing the axis by widest spread in the overflowing bucket addresses both. The catch is that
the choice must be *recorded*, or queries compare the wrong coordinate against the plane and
silently return wrong neighbours — a worse failure than the current refusal.

## Settled decisions

**Record the dimension in the low bits of the split value**, not in a side array. A stem in v6
is a bare `A` with no room for metadata, and `dim()` has no `A` parameter, so a side array would
need a second pointer plumbed into the cursor *and* costs an extra cache line per descent step.
See `dim_packing.rs` for the encoding, why width suffices without knowing the axis type, and why
the dimension is stored complemented.

**Add a new cursor type rather than modifying `EytzingerFlexPf`.** Upstream's 397 tests keep
exercising the unmodified strategy; we opt in only at `scene-graph`'s `PosIndex` alias. Note the
limit of this isolation: a new cursor isolates us from other *cursors*, not from shared tree code
in `kd_tree/`, which is generic over `SS` and monomorphised per strategy. **Anything that must
differ per strategy has to be dispatched through the trait**, with a default that preserves
current behaviour.

**Do not change the stem-array filler.** `A::max_value()` is detected by value elsewhere
(`Donnelly::{scalar,no_pf,simd_descent}` use `is_max_value` as a routing decision, and the bulk
builder asserts on it), and that write lives in shared code (`construction.rs:106`). The
complement encoding makes an all-ones low-bit pattern decode to dimension 0 for every `K`, so
unwritten slots are harmless without touching the filler at all.

**The cursor caches the decoded dimension.** `fn dim<const K: usize>(&self)` has no `A`, so it
cannot interpret the bytes at `stems_ptr`. `traverse<A, K>` does have `A`, so it decodes the
child's dimension on the way down and caches it — keeping `dim()`'s signature, and with it all
61 call sites, untouched. The cursor therefore has the same four fields as upstream; only the
meaning of `dim` changes, from a counter to a cached decode.

**`stems_ptr` may be stale in construction.** Upstream only ever passes it to `prefetch_t0`,
where a bogus address is harmless, and `split_leaf` reallocates the array
(`self.stems.resize(...)`). Since our `dim()` genuinely dereferences it: **queries decode
through the cursor** (tree is `&self`, so no reallocation can happen mid-descent) and
**construction reads and writes dimensions through `self.stems` directly**.

## Progress

Status line at the top of this file is the summary; this section records what each landed piece
actually did, including where it departed from the plan above.

### Landed

**`dim_packing`** — `dim_bits`/`dim_mask` as `const fn` of `K`, plus pack/unpack over one byte at
a computed offset. Driven by the value's *width* only, since every axis type this crate supports
keeps its fraction in the low-order bits, so no per-type `Axis` methods were needed. The
dimension is stored plainly, and `unpack_from` clamps the decode into range. An earlier draft
stored it complemented, on the belief that every `max_value()` filler has all its low bits set;
that is true for integer and fixed-point axes but **not** for floats, where `max_value()` is
infinity and the mantissa is all zeros. No single encoding is safe for both families, so the
clamp does the work — one comparison, and the filler is left untouched, which keeps `Donnelly`'s
`is_max_value` routing working.

**`grid_separator`** — given the pair a split brackets, returns an aligned value strictly between
them, or `None` below one grid cell. Candidates are verified by value comparison rather than
derived by bit arithmetic, which is what makes the sign asymmetry a non-issue. An exhaustive
sweep over gaps of 4..64 ULP pins the guarantee that a separator always exists beyond one cell.

**`EytzingerAdaptive` + trait hooks** — `encode_split` (default identity) and `bind_stems`
(default no-op), both `doc(hidden)`. Two departures from the plan above:

* **No `sync_root_dim`, and no cached dimension.** The plan had `traverse` decode the child's
  dimension into a cached field. That is unsound: `traverse` runs one step further than the
  descent loop reads — the last step computes a child index the loop never visits — so it would
  decode from a slot past the end of the stems array. `dim()` decodes on demand instead, which is
  safe because `dim()` is only reached at an index the caller has just read. The hook became
  `bind_stems`, supplying the axis width and stem count that `dim()` cannot obtain otherwise, at
  eight descent entry points. `DeferredState` lost its `dim` field as a result.
* **Construction does not encode yet.** The plan had commit 1 encode the *cycled* dimension to
  prove the wiring. That cannot work: the encoded value differs from the pivot by up to one grid
  cell, and `split_leaf` has already partitioned against the pivot, so the stored plane would sit
  off the partition and queries would lose points. Encoding and partitioning are inseparable.

### Bulk construction, adapted

Supersedes "Bulk construction is refused, not adapted" below, which explains why this was
originally deferred and is worth reading first.

`KdTree::choose_split` replaces the bare `stems[stem_index] = axis_at(&source[sort_index[pivot]],
dim)` at both write sites. Per internal node, for adaptive strategies only:

1. Rank the axes by spread over the chunk, widest first, using `Axis::saturating_dist`. Ties keep
   the cycled axis's precedence so well-conditioned data behaves as before.
2. For each axis in turn: `update_pivot`, then bracket the boundary and ask `grid_separator` for an
   aligned value strictly inside the bracket. First success wins; the partition left in
   `sort_index` is the chosen axis's, since each attempt re-partitions from scratch.
3. Because the separator lies strictly between the bracketing coordinates, the index partition and
   the value partition coincide — no re-partitioning, and the recursion is untouched.

Cycling strategies take an early return that is exactly upstream's behaviour, so the 397 upstream
tests stay bit-identical.

Two things the original design did not anticipate, both found by testing against the corpus rather
than by reasoning:

* **`grid_separator` returning `None` is common, not unreachable.** The claim was that two distinct
  values within one 4-ULP cell "does not arise" on a 0.05 m lattice. It arises constantly, because
  interpolated mesh vertices are not exactly on the lattice they were derived from. This is why the
  axis fall-through in step 2 is load bearing rather than defensive: without it the corpus refuses.
* **`update_pivot` had to be tightened.** Its leftward nudge walks contiguously, which assumes
  `select_nth_unstable_by` leaves equal values adjacent to the pivot. It does not in general, and
  the case that breaks it is a highly asymmetric pivot — a chunk of `B + 1` splitting 128/1, where a
  full bucket of duplicates sits left of the boundary. A cycling strategy survives that (it stores
  the boundary coordinate verbatim, and the query's branch-and-bound recovers); an adaptive one has
  no value satisfying `max(left) < s <= min(right)` and refuses a chunk it should have split. One
  forward pass now moves every left-hand element equal to the boundary into the right region.
  Gated on `ADAPTIVE_SPLIT_DIM`, so cycling is untouched.

Measured on the office corpus (354,885 lattice points, bucket 128), stem slots and depth:

| build | slots | depth |
|---|---|---|
| incremental, mesh order | 15,203,841 (abandoned at 11,768 points) | 23 |
| bulk, cycling axes | 1,310,722 | 20 |
| bulk, adaptive axes | **163,842** | **17** |

Balanced would be depth 12, so there is still headroom — the remaining gap is the pivot being
chosen from a leaf budget rather than from the data.

### Rebalancing, which is what bulk construction was wanted for

`add_rebalancing` pairs a cheap shape test with a bulk rebuild. Measured findings that a reader
should not have to rediscover:

* **Re-inserting incrementally is not a rebalance.** The obvious way to avoid needing adaptive bulk
  construction is to drain the tree and re-add the points in a shuffled order. It does not work:
  shuffling a *spatially concentrated* subset does not decorrelate it, and the rebuild frequently
  produces a **deeper** tree than the one it replaced (measured: slots 32 -> 897 at depth 4 -> 9,
  and 821 -> 21,508 at depth 9 -> 14). Every threshold from 4 to 64 then failed with the rebuild
  itself exceeding the slot budget. Bulk construction is load bearing, not a convenience.
* **The threshold comes from incremental measurements.** See README deviation 6 — bulk-built trees
  sit at 2.4-3.9 but incremental ones reach 9.4 by 60k points and keep climbing, so the naive
  threshold of 8 rebuilds healthy trees.
* **Thrashing was the predicted risk and did not materialise.** 11-189 rebuilds and under 0.9 s of
  rebuild time across tens of thousands of inserts, so no amortisation guard was needed. The
  measured counts were themselves inflated by the broken incremental rebuild, which often failed to
  reduce the ratio and re-triggered at once.
* **A failed rebuild must not fail the insert.** Bulk construction can refuse a point set; treating
  that as an insertion error made the corpus unbuildable.

End-to-end on the benchmark corpus, building the whole indexed scene graph: 6.04 s and 7.4 GB peak
RSS before, 1.98 s and 839 MB after, against 837 MB for the unindexed target — so the indexes
stopped dominating memory. Build time went from 6.3x to 2.0x when going from 202k to 358k points.

Still open: with rebalancing bounding memory, mesh-order insertion now progresses far enough to hit
a *different* wall — `split_leaf_adaptive` refusing a bucket whose points are near-coincident on
every axis, which is the phase-4 overflow representation, below. So the benchmark still decorrelates
its insertion order, for that reason rather than for memory.

### Bulk construction is refused, not adapted

`new_from_slice` / `new_from_source` / `new_from_entries` return
`ConstructionError::AdaptiveSplitBulkBuildUnsupported` when the strategy sets
`ADAPTIVE_SPLIT_DIM`, guarded in `new_from_source_with` just after the single-leaf early return —
a tree with no stems has no dimension to record, so that case stays allowed.

The reason it is refused rather than fixed: the bulk builder chooses a pivot from `sort_index`,
partitions around it, and *then* writes it. Folding a dimension into the written value would move
each plane up to one grid cell off the partition it was built from, so points would sit on the
wrong side of their own split and queries would lose them. Making it correct needs the same
grid-aligned bracketing the incremental split does — which is real work, not the two-line change
an earlier draft of this file claimed.

Refusing costs this workspace nothing: `scene-graph` maintains its indexes point by point through
`add`, and never bulk-builds. The alternative — leaving it unencoded — would produce a tree that
answers queries on arbitrary axes with no error at all, which is the exact failure this design
exists to prevent.

### Remaining — one coherent change

All in `VecOfArrays::split_leaf` unless noted:

1. Choose the axis by widest bound spread over the leaf's live points, ties to the lowest index.
   Leaves are dimension-major (`content_points: [[A; B]; K]`), so each scan is a contiguous walk.
2. Sort on that axis, take the bracketing pair `(points[pivot_idx - 1], points[pivot_idx])`, pass
   it to `grid_separator`, fold the dimension in with `SS::encode_split`, and partition against
   **that** value.
3. This *replaces* upstream's pivot-nudge logic (the path ending in its "we're fucked" comment)
   on the adaptive route. That logic exists to shuffle the pivot when equal values straddle it,
   which is only necessary because the axis was chosen blind. Cycling strategies keep it, since
   `encode_split` defaults to identity.
4. `grid_separator` returning `None` → `UnsplittableBucket` plus this TODO. **The prediction in it
   was wrong**: this is not "only reachable" on a lattice, it is common, because interpolated mesh
   vertices are not exactly on the lattice they were derived from. The axis fall-through it suggests
   is therefore load bearing, not defensive, and is implemented.
   ```rust
   // TODO: grid quantisation left no aligned plane between the bracketing values. Only
   // reachable when two distinct values sit within dim_mask(K)+1 ULP on the split axis —
   // millions of ULP apart on a 0.05 m lattice. Fix by widening the search, then falling
   // through to the next-widest axis.
   ```
   Without it the point that triggered the split is routed into the still-full leaf and
   `add_to_leaf` panics on `content_points[dim][B]`.
5. The bulk builder (`construction.rs:825, 933`) must encode too, or a bulk-built tree decodes
   arbitrary low bits as dimensions and returns silently wrong neighbours. It may keep cycling —
   `EytzingerAdaptive::construction_dim` is `level % K`, pure arithmetic, so it stays safe on the
   dangling pointer `new_no_ptr()` hands the builder.
6. `scene-graph/src/pos.rs`: `PosIndex` uses `EytzingerAdaptive`.
7. Rewrite `pos.rs::coplanar_defect` into acceptance tests — coplanar and lattice cases must
   build and agree with brute force; identical points must still fail cleanly, which is phase 4.
8. Re-run the A/B against `bench/BASELINE.md`.

Expected movement in the A/B: `surfaces-0.05-100k` and `clustered-100k` should build rather than
report a failure; `real:unity`'s depth 27 and 266,301,286 stem slots should fall towards the bulk
builder's figures; `nearest_*` must stay inside the ±2% noise floor, since indexing exists in
`scene-graph` to cut query latency.

## Known guarantee

With widest-spread selection, a split **cannot** fail when the chosen axis has spread greater
than the grid: `split_leaf` only errors when the pivot search walks to `B`, which requires every
value on that axis to be equal. So the only residual failure is points identical on *every*
axis, which needs the overflow representation (phase 4) rather than a better axis choice.

## Not in scope

- **The `2^depth` stem addressing itself.** Still unchanged, but no longer an exposure for this
  workspace. The open question this section used to pose — whether adaptive selection alone bounds
  depth — has been answered, and the answer was no: clustered insertion still drove the array past
  a 1 GiB budget after the split patch. What bounds it is `add_rebalancing` (README deviation 6),
  which detects the runaway from the *prospective* `stem_idx` and bulk-rebuilds before the
  allocation happens. `scene-graph` routes every index insertion through it, so a tree there cannot
  explode; a caller using plain `add` still can, which is a reason to prefer `add_rebalancing`
  rather than an open defect. Replacing the flat breadth-first array with indirect addressing would
  remove the exposure at the root and remains out of scope.
- **Making the Donnelly / SIMD block strategies adaptive.** They would need per-lane dimension
  gathers. Nothing in this workspace instantiates them.
- **Phase 4**, the overflow representation for buckets of points identical on every axis. Now the
  *active* blocker rather than a theoretical one, and it only became visible once rebalancing let
  mesh-order insertion run far enough to reach it: `split_leaf_adaptive` refuses a bucket whose
  points sit within one grid cell on all three axes, which real interpolated mesh vertices do. This
  is why `yggdrasil-bench` still decorrelates its insertion order — for this reason, not for memory.
