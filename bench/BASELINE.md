# Pre-patch baseline

Recorded while `spatial-index` was still behaviourally identical to upstream kiddo
6.0.0-alpha.3 — the vendoring commit plus one `#[derive]`. Every delta below is therefore
measurement noise, which is the point: it calibrates the harness and fixes the numbers the
adaptive-split work will be judged against.

Machine: 15 GiB RAM, rustc 1.96.1, `--release`. Re-record on other hardware before comparing.

## Running it

```sh
cargo run --release -p bench                          # the five synthetic cases
cargo run --release -p bench -- --only uniform-500k    # one case
cargo run --release -p bench -- --json                 # machine-readable
cargo run --release -p bench -- --data cloud.f32 --only real --budget 6
```

`--data` takes raw little-endian `[f32; 3]` triples. `--budget <GiB>` caps a single build; the
default of 1 GiB keeps a plain run quick, and the pathological cases need it raised to be
measured at all.

## Methodology

Both implementations are linked into one binary and measured in the same process on the same
inputs, so a comparison never rests on numbers remembered from an earlier session. Details that
matter for trusting the deltas:

* **Order is swapped and merged.** Whichever implementation runs first pays for heap growth and
  instruction-cache misses that the second then benefits from — worth a systematic ~12% swing on
  build and remove with *identical* code. Each implementation runs once in each position and the
  better of each measurement is kept.
* **Minimum, not mean.** For latency work the fastest observed run is the least contaminated.
* **Queries get an untimed warmup pass**, so they measure steady state.
* **Memory is counted by a wrapping global allocator**, not sampled from RSS: it is per-case
  rather than a process high-water mark, and it does not depend on when the OS reclaims pages.
  It is also fully deterministic — see the `+0.0%` column.
* **Inputs come from a xorshift PRNG defined in the harness**, not `rand`, so they are stable
  across machines and immune to dependency churn.
* `B = 128` and a `u64` item, mirroring `scene-graph`'s position index.

### Noise floor

Repeating an identical measurement three times moved the deltas over a **±2% band typically,
±8% worst case** on the synthetic sets. On the 5 GiB real-data case the band widens to roughly
**±15%** under memory pressure. Treat anything inside those bands as nothing. Memory deltas are
exact: any movement there is real.

## Numbers

| case | build ns/insert | build peak | nearest_1 | nearest_10 | nearest_100 | within | remove | bulk ns/pt | bulk peak |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| uniform-100k | 39 | 5.1 MiB | 400 | 1343 | 8120 | 734 | 128 | 37 | 2.7 MiB |
| uniform-500k | 56 | 20.4 MiB | 450 | 1525 | 9626 | 871 | 267 | 79 | 13.5 MiB |
| lattice-0.05-100k | 42 | 5.1 MiB | 424 | 1367 | 8248 | 691 | 204 | 40 | 4.7 MiB |
| real:unity (304,227) | **6467** | **5.0 GiB** | 441 | 2203 | 11890 | 10641 | 178 | 50 | **14.2 MiB** |

Times in ns, upstream and fork agreeing within the noise floor throughout. `within` uses a
radius sized to enclose ~10 neighbours from the input's own density.

## What does not build at all

| case | outcome |
| --- | --- |
| `clustered-100k` | **exceeded 2.5 GiB after 31,744 of 100,000 points** — both implementations, same point |
| `surfaces-0.05-100k` | **refused after 3,854 of 100,000 points** — both implementations, same point |

Two independent defects, and the two things the adaptive-split work has to move:

1. **Coplanar refusal.** `surfaces-0.05-100k` puts points on the six faces of a room and snaps
   them to a 0.05 m lattice, so each plane holds thousands of points. Note that
   `lattice-0.05-100k` — a *uniform* cloud on the same lattice — builds fine: spread through a
   volume, a 128-point bucket almost never lands entirely on one plane. Real geometry is
   surfaces, not volumes, which is why the defect is normal rather than exotic there.
2. **Exponential memory on clustered insertion order.** Stems live in a flat array addressed by
   breadth-first position, so depth costs `~2^depth` slots. Clustered arrivals deepen one path
   instead of broadening the tree. The clustered case crossed the 1 GiB budget and reached
   2.5 GiB inside a *single* 1024-insert window, which is the doubling signature itself.

The real-data row makes the second one concrete on the same input. For unity's 304,227 points:

| path | ns/point | peak memory |
| --- | --- | --- |
| incremental `add` | 6,467 | 5.0 GiB |
| bulk `new_from_entries` | 50 | 14.2 MiB |

**129× the time and 360× the memory**, same points, same tree type. The bulk builder picks
global medians and stays balanced; the incremental path does not. `scene-graph` can only use the
incremental path, because layer and node indexes are maintained point by point as the graph
changes.

This also corrects the phase-2 record: unity was noted there as building successfully in 2.5 s.
It does complete, but at 5.0 GiB — it was never a healthy case, just a survivable one. The
phase-2 RSS figures for map1 are confirmed independently here: it crosses 1 GiB at ~204k points,
matching the 220k → 1,074 MB measured then.

## After the patch

Re-run and compare. The rows that should move:

* `clustered-100k` and `surfaces-0.05-100k` should *build* rather than report a failure — that
  is the acceptance criterion, and both are currently pinned by the characterization tests in
  `scene-graph/src/pos.rs::coplanar_defect`.
* `real:unity` incremental build should approach its own bulk figures if adaptive splitting
  bounds depth. Whether it does is an open question, not an assumption: widest-spread selection
  keeps splits balanced, but `2^depth` addressing means any deep tree stays expensive.
* Everything in the first table should stay inside the noise floor. A real regression in
  `nearest_*` matters most: indexing exists in `scene-graph` to cut query latency.
