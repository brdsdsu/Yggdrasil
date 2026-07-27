# Adaptive split-dimension selection — design and remaining work

Status: encoding layer done and tested (`src/stem_strategy/dim_packing.rs`). The cursor,
the trait hooks and the selection policy are not written yet.

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

## Remaining work

### Commit 1 — plumbing, behaviour provably identical

1. Two provided methods on `StemStrategy`, both defaulting to preserve current behaviour:
   - `encode_split<A: Axis, const K: usize>(value: A, dim: usize) -> A`, default identity.
     Required because `split_leaf` is shared code — packing unconditionally would perturb every
     strategy's split values by a few ULP and break the "identical behaviour" gate.
   - `sync_root_dim<A: Axis, const K: usize>(&mut self, stems: &[A])`, default no-op. Needed
     because `new()` has no `A` either, so the root's dimension cannot be decoded at
     construction; call it right after `SS::new` at the ~10 descent entry points
     (6 in `orchestrator/mod.rs`, 2 `construction.rs`, 1 `iter.rs`, 1 `query/periodic.rs`).
2. `EytzingerAdaptive`: copy of `EytzingerFlexPf`, with `traverse` decoding the child's dimension
   in place of `(dim + 1) % K`, and `construction_dim` returning `level % K` — pure arithmetic,
   so it stays safe on the dangling pointer `new_no_ptr()` hands the bulk builder.
3. Both construction paths call `encode_split` with the dimension they already choose — still
   the **cycled** one. Incremental: `construction.rs:95`. Bulk: `construction.rs:714, 902`.
   The bulk path must encode even though it stays cycling, or bulk-built trees decode garbage.
4. `scene-graph/src/pos.rs`: `PosIndex` uses `EytzingerAdaptive`.

Gate: 402 `spatial-index` tests, 25 + 5 `scene-graph` tests, and `bench/BASELINE.md` all
unmoved. That proves pack/unpack round-trips in situ before any policy change.

### Commit 2 — the policy

5. In `split_leaf`, choose the dimension by widest bound spread over the bucket's live points,
   ties to the lowest index, then partition against the **encoded** value (not the raw median —
   the stored value must be the one compared, or points land on the wrong side).
   Leaves are dimension-major (`content_points: [[A; B]; K]`), so a min/max scan per axis is a
   linear walk over contiguous floats.
6. Guard the case where quantisation collapses the split, and leave the TODO:
   ```rust
   if boundary == 0 || boundary == orig.size {
       // TODO: grid quantisation collapsed the split. Only reachable when two distinct values
       // sit within dim_mask(K)+1 ULP on the split axis — millions of ULP apart on a 0.05 m
       // lattice. Fix by retrying neighbouring grid points, then the next-widest axis.
       return Err(ConstructionError::UnsplittableBucket { split_dim });
   }
   ```
   Without it the point that triggered the split is routed into the still-full leaf and
   `add_to_leaf` panics on `content_points[dim][B]`.
7. Rewrite `pos.rs::coplanar_defect` into acceptance tests: coplanar and lattice cases must
   build and agree with brute force; identical-points must still fail cleanly (phase 4).
8. Re-run the A/B against `bench/BASELINE.md`. Expect `surfaces-0.05-100k` and
   `clustered-100k` to build, and `real:unity`'s depth 27 / 266M slots to fall towards the
   bulk builder's numbers. Query throughput must stay inside the ±2% noise floor.

## Known guarantee

With widest-spread selection, a split **cannot** fail when the chosen axis has spread greater
than the grid: `split_leaf` only errors when the pivot search walks to `B`, which requires every
value on that axis to be equal. So the only residual failure is points identical on *every*
axis, which needs the overflow representation (phase 4) rather than a better axis choice.

## Not in scope

- **The `2^depth` stem addressing itself.** Deprioritised deliberately. Adaptive selection should
  reduce depth substantially, but any deep tree stays expensive; whether that is sufficient is an
  open question the A/B will answer.
- **Making the Donnelly / SIMD block strategies adaptive.** They would need per-lane dimension
  gathers. Nothing in this workspace instantiates them.
- **Phase 4**, the overflow representation for buckets of identical points.
