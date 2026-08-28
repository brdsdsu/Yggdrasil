# Progress Report

*Phase 3 (downstream integration) status report.*

---

## TLDR

Yggdrasil now runs as a drop-in scene graph backend for LP2, and the full
benchmark campaign is complete: 5 repeated runs per setting (original,
Yggdrasil table, Yggdrasil on-demand), each covering both scenes (office,
45 trajectories; home, 31). Every run produced **outputs identical to the
original**. **Yggdrasil answers LP2's
dominant queries two to three orders of magnitude faster**: ~870× (office)
and ~640× (home) on the most frequent lookup (room and position of a
waypoint), ~90-110× on the radius query, ~25-30× on nearest-waypoint,
setup 50-80× cheaper. The total time spent interacting with the scene graph
drops from 7.7 s to 31 ms per office trajectory (99.6% eliminated) and
from 1.8 s to 21 ms per home trajectory (98.8%). Because total
pipeline time is dominated by probability math no backend touches, this
translates end-to-end into a modest ~4% speedup. Yggdrasil had no shortest-path query, so one was implemented in the Rust core and exposed to Python. Three real bugs in the Python bindings were
found and fixed along the way. The remaining decision is which Yggdrasil
variant to put forward as the headline.

## 1. Objective and target

Per the proposal, the goal is to show that Yggdrasil (a) integrates into an
existing robotics pipeline with low friction and covers the scene graph
functionality such systems rely on, and (b) matches or surpasses the original
backend's performance. The chosen target is LP2 (Gorlo, Schmid, Carlone,
RA-L 2024): it predicts a probability distribution over a person's position
up to 60 seconds ahead, using an LLM to guess upcoming human-object
interactions and a 3D scene graph (rooms / walkable waypoints / objects) to
ground those guesses in space. LP2 only reads its scene graph, which makes it
a clean downstream-integration target: we replace what is read and how
queries are answered, and nothing else.

## 2. LLM cache

LP2's LLM calls are recorded once and replayed in every run afterwards:
responses are looked up by a fingerprint of the entire prompt text, which
makes runs deterministic and free of API costs, and doubles as a
correctness alarm. For the final run campaign we recorded both scenes with gpt-5.6-luna: 335 responses for office and 235 for home.

## 3. Integration design

**Topology.** Three Yggdrasil layers, objects, places (waypoints), rooms, with each waypoint nested under its room. Nesting is the key structural
choice: the vanilla LP2 answers "which room is this waypoint in?" by scanning
the graph's entire edge list (its single hottest query, ~7,800 times per
office trajectory); nesting turns it into reading one parent pointer.
Only the places layer is spatially indexed, because it is the only layer
receiving nearest-neighbor queries, indexing follows the query patterns.

**Query mapping.** Nearest-waypoint and radius queries go to Yggdrasil's
spatial index (replacing brute-force scans and a KD-tree wrapper that
rebuilt a full node list on every query). Room lookups use the parent
pointers. Object-by-class lookup remains a simple scan (about 110 objects,
nothing to win; noted that Yggdrasil has no attribute indexing, which would
matter at much larger scales). Shortest paths over the waypoint network,
the one capability Yggdrasil lacked, were implemented in the Rust core with
tie-breaking proven identical to the original's, and exposed to Python, in
two modes:

- **Table mode** replicates the original's startup precompute exactly (one
  full shortest-path run per object, same table). The direct comparison.
- **On-demand mode** computes only the paths actually needed, when needed.
  The original's table computes far more than is ever read. This mode is the
  "use Yggdrasil properly" result.

**Deliberate exclusions**, so the benchmark measures structure rather than
tuning: no semantic indexing added to Yggdrasil, and no Python-side caching
of repeated lookups. Therefore, every query hits the backend every time.

**Integration cost in code.** Counting code lines only (comments and blank
lines excluded): the Yggdrasil-backed query code is 504 lines against the
421 lines of original query code it replaces; the surplus is the one-time
converter (118 lines) that builds the Yggdrasil graph from the scene file.
No existing LP2 code was modified; the integration is four new files
selected by config. 

## 4. Verification 

1. **Converter validation**: node counts, positions, room assignments,
   orderings, nesting, nearest-neighbor answers, and a full shortest-path
   cross-check against the original. zero distance or route mismatches on either scene.
2. **Prompt construction** The production pipeline inherits the
   original's prompt generator, so parity is guaranteed by construction and
   confirmed by zero cache misses across every full run. Separately, a
   generator producing the prompt purely from Yggdrasil queries was built
   and verified
3. **Output equivalence** Both Yggdrasil variants ran
   the full trajectory suites of both scenes; every output was compared
   against the original run and everything was identical
4. **Campaign gates** Repeated runs of the same setting are exactly
   deterministic (run 1 vs run 2 identical, both scenes), the equivalence
   check was repeated per scene inside the campaign, and the timed runs'
   outputs match the untimed runs' outputs exactly, confirming the query
   timers change nothing about the computation.
## 5. Performance

**Per office trajectory, the original spends 7.7 seconds interacting with
the scene graph; Yggdrasil table mode spends 31 milliseconds, a 251×
reduction that eliminates 99.6% of all graph time. Per home trajectory:
1.8 s vs 21 ms (83×, 98.8%).** (The on-demand variant: 183 ms and 53 ms.)
All numbers in this section come from the campaign itself: every
scene-graph call in all 15 runs carried a lightweight timer, so each
distribution pools 5 repetitions.

### 5.1 What each query costs

One table per scene, plus both scenes pooled. n is how many times the
query actually ran across the 5 runs; p50 is the typical call,
p95/p99/max are the tails. Values in microseconds unless marked ms.
Once-per-run rows (table build, setup) show the per-run average. These
are the query costs under the benchmark's real conditions.

**Office:**

| Query | Backend | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|---|
| Room + position of a waypoint (most frequent query) | original | 1,747,260 | 987 | 1,019 | 1,054 | 2,065 |
| | Yggdrasil | 1,747,260 | 1.13 | 2.9 | 4.4 | 41 |
| Nearest waypoint to a position | original | 450 | 169 | 233 | 265 | 323 |
| | Yggdrasil | 450 | 5.7 | 10.3 | 11.7 | 13.4 |
| Start region (waypoints within 0.5 m) | original | 225 | 8,420 | 8,749 | 8,817 | 8,830 |
| | Yggdrasil | 225 | 79 | 95 | 104 | 108 |
| Path lookup in the precomputed table | original | 56,500 | 0.50 | 1.8 | 2.0 | 11 |
| | Yggdrasil (table mode) | 56,500 | 0.63 | 2.0 | 2.1 | 10 |
| Path computed on demand (one target) | Yggdrasil (on-demand mode) | 56,500 | 143 | 972 | 1,047 | 1,329 |
| Grounding distances, per prediction step | original | 22,560 | 20 | 54 | 69 | 152 |
| | Yggdrasil (table mode) | 22,560 | 23 | 64 | 79 | 159 |
| | Yggdrasil (on-demand mode) | 22,560 | 923 | 1,096 | 1,235 | 4,098 |
| Planning position reads, bulk per path | Yggdrasil | 63,095 | 11 | 33 | 41 | 362 |
| Build the path table at startup (once per run) | original | 5 | 938 ms | | | |
| | Yggdrasil (table mode) | 5 | 547 ms | | | |
| Setup: build queryable structures (once per run) | original | 5 | 2,681 ms (still built twice) | | | |
| | Yggdrasil | 5 | 35 ms | | | |

**Home:**

| Query | Backend | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|---|
| Room + position of a waypoint (most frequent query) | original | 357,470 | 725 | 758 | 790 | 1,801 |
| | Yggdrasil | 357,470 | 1.14 | 3.2 | 4.2 | 39 |
| Nearest waypoint to a position | original | 310 | 134 | 176 | 589 | 661 |
| | Yggdrasil | 310 | 5.3 | 9.0 | 11.5 | 12.6 |
| Start region (waypoints within 0.5 m) | original | 155 | 5,981 | 6,484 | 6,557 | 6,591 |
| | Yggdrasil | 155 | 69 | 91 | 97 | 104 |
| Path lookup in the precomputed table | original | 15,920 | 0.50 | 1.9 | 2.0 | 13 |
| | Yggdrasil (table mode) | 15,920 | 0.65 | 2.0 | 2.2 | 16 |
| Path computed on demand (one target) | Yggdrasil (on-demand mode) | 15,920 | 100 | 634 | 682 | 746 |
| Grounding distances, per prediction step | original | 10,020 | 14 | 35 | 49 | 126 |
| | Yggdrasil (table mode) | 10,020 | 14 | 40 | 59 | 128 |
| | Yggdrasil (on-demand mode) | 10,020 | 430 | 726 | 793 | 1,965 |
| Planning position reads, bulk per path | Yggdrasil | 18,775 | 10 | 25 | 29 | 65 |
| Build the path table at startup (once per run) | original | 5 | 861 ms | | | |
| | Yggdrasil (table mode) | 5 | 438 ms | | | |
| Setup: build queryable structures (once per run) | original | 5 | 1,355 ms (still built twice) | | | |
| | Yggdrasil | 5 | 27 ms | | | |

**Both scenes pooled:**

| Query | Backend | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|---|
| Room + position of a waypoint (most frequent query) | original | 2,104,730 | 982 | 1,016 | 1,050 | 2,065 |
| | Yggdrasil | 2,104,730 | 1.13 | 2.9 | 4.3 | 41 |
| Nearest waypoint to a position | original | 760 | 155 | 224 | 280 | 661 |
| | Yggdrasil | 760 | 5.6 | 9.8 | 11.6 | 13.4 |
| Start region (waypoints within 0.5 m) | original | 380 | 8,333 | 8,695 | 8,807 | 8,830 |
| | Yggdrasil | 380 | 77 | 94 | 101 | 108 |
| Path lookup in the precomputed table | original | 72,420 | 0.50 | 1.8 | 2.0 | 13 |
| | Yggdrasil (table mode) | 72,420 | 0.64 | 2.0 | 2.1 | 16 |
| Path computed on demand (one target) | Yggdrasil (on-demand mode) | 72,420 | 128 | 913 | 1,040 | 1,329 |
| Grounding distances, per prediction step | original | 32,580 | 18 | 52 | 65 | 152 |
| | Yggdrasil (table mode) | 32,580 | 20 | 61 | 74 | 159 |
| | Yggdrasil (on-demand mode) | 32,580 | 837 | 1,083 | 1,167 | 4,098 |
| Planning position reads, bulk per path | Yggdrasil | 81,870 | 10 | 32 | 40 | 362 |
| Build the path table at startup (once per repetition, both scenes) | original | 5 | 1,799 ms | | | |
| | Yggdrasil (table mode) | 5 | 985 ms | | | |
| Setup: build queryable structures (once per repetition, both scenes) | original | 5 | 4,036 ms (still built twice per scene) | | | |
| | Yggdrasil | 5 | 62 ms | | | |

Notes on reading the tables:

- **Timer cost (measured):** each timed call carries ~0.05 µs of clock
  overhead, invisible next to the original's microsecond-to-millisecond
  queries but up to ~5% of Yggdrasil's ~1 µs entries. Where it matters at
  all, it makes Yggdrasil look slightly worse, never better.
- **Thin rows:** nearest-waypoint runs twice per trajectory and
  start-region once, so their n is small. Timing them once per every real
  waypoint position (isolated, on an idle node) gives the same picture:
  nearest waypoint 30 µs vs 0.5 µs (office, 1,220 positions) and 25 µs vs
  0.5 µs (home, 861 positions); start region 5,411 µs vs 9.5 µs and
  3,740 µs vs 21 µs.
  
### 5.2 Where the time goes

Each cell is the measured time the runs actually spent on that kind of
query, divided down to one trajectory (once-per-run costs like setup and
the path table are spread over the run's trajectories; all cells average
the 5 repetitions):

**Office (45 trajectories per run):**

| Graph interaction | Calls/traj | Original | Ygg table | Ygg on-demand |
|---|---|---|---|---|
| Room + position lookups | 7,766 | **7,655 ms** | 10.8 ms | 11.3 ms |
| Planning position reads | 280 paths | (dict reads, ~0.2 µs each)* | 3.8 ms | 3.7 ms |
| Nearest waypoint | 2 | 0.4 ms | ~0 | ~0 |
| Start region | 1 | 8.5 ms | 0.1 ms | 0.1 ms |
| Path lookups + grounding (on-demand computes each path instead) | 367 | 2.7 ms | 3.2 ms | **167 ms** |
| Path table build ÷ 45 | once | 20.8 ms | 12.2 ms | - |
| Setup ÷ 45 (original builds twice) | once | 59.7 ms | 0.8 ms | 0.8 ms |
| **Total graph time per trajectory** | | **7,747 ms** | **31 ms** | **183 ms** |

**Home (31 trajectories per run):**

| Graph interaction | Calls/traj | Original | Ygg table | Ygg on-demand |
|---|---|---|---|---|
| Room + position lookups | 2,306 | **1,673 ms** | 3.4 ms | 3.6 ms |
| Planning position reads | 121 paths | (dict reads, ~0.2 µs each)* | 1.3 ms | 1.3 ms |
| Nearest waypoint | 2 | 0.3 ms | ~0 | ~0 |
| Start region | 1 | 6.0 ms | 0.1 ms | 0.1 ms |
| Path lookups + grounding (on-demand computes each path instead) | 177 | 1.2 ms | 1.3 ms | **47 ms** |
| Path table build ÷ 31 | once | 27.8 ms | 14.1 ms | - |
| Setup ÷ 31 (original builds twice) | once | 43.9 ms | 0.9 ms | 0.9 ms |
| **Total graph time per trajectory** | | **1,752 ms** | **21 ms** | **53 ms** |

**Both scenes (76 trajectories per repetition, cells are per-trajectory
averages over all of them):**

| Graph interaction | Calls/traj | Original | Ygg table | Ygg on-demand |
|---|---|---|---|---|
| Room + position lookups | 5,539 | **5,215 ms** | 7.8 ms | 8.1 ms |
| Planning position reads | 215 paths | (dict reads, ~0.2 µs each)* | 2.8 ms | 2.7 ms |
| Nearest waypoint | 2 | 0.3 ms | ~0 | ~0 |
| Start region | 1 | 7.5 ms | 0.1 ms | 0.1 ms |
| Path lookups + grounding (on-demand computes each path instead) | 290 | 2.1 ms | 2.4 ms | **118 ms** |
| Path table build ÷ 76 | once per scene | 23.7 ms | 13.0 ms | - |
| Setup ÷ 76 (original builds twice per scene) | once per scene | 53.3 ms | 0.8 ms | 0.8 ms |
| **Total graph time per trajectory** | | **5,302 ms** | **27 ms** | **130 ms** |

\* the original reads planning positions one dictionary access at a time,
too small to time individually inside a run; counting these as zero for
the original and in full for Yggdrasil only favors the original.

One row dominates everything in both scenes: the original answers "which
room is this waypoint in?" by scanning the graph's entire edge list,
thousands of times per trajectory. That's 99% of its graph time. The last
rows show the modes' trade-off: table mode pays for the table once per
run and looks paths up thereafter (the original's strategy, done ~1.8×
faster); on-demand pays nothing up front and 167 ms (office) / 47 ms
(home) per trajectory, relevant for a live robot's first prediction
rather than throughput.

### 5.3 End-to-end confirmation

All numbers below come from the campaign runs' per-trajectory file
timestamps and job records. Each cell shows the median over the 5
repetitions, with the range across repetitions in parentheses. Every
trajectory produces predictions under the new cache, so nothing is
excluded from pairing.

**Office (45 trajectories):**

| | Original | Ygg table | Ygg on-demand |
|---|---|---|---|
| Median time per trajectory | 214 s (213-216) | 207 s (207-209) | 207 s (207-208) |
| Whole run | 148.7 min (147.8-149.2) | 142.8 min (142.2-143.5) | 142.8 min (142.8-143.3) |
| Paired comparison per run | - | faster on **45/45** (44/45 in one run) | faster on **45/45** in all 5 runs |
| Mean saving per trajectory | - | 7.7 s | 7.2 s |

**Home (31 trajectories):**

| | Original | Ygg table | Ygg on-demand |
|---|---|---|---|
| Median time per trajectory | 31 s (30-31) | 29 s (29-30) | 29 s (29-30) |
| Whole run | 20.9 min (20.7-21.0) | 19.9 min (19.8-20.0) | 19.8 min (19.8-20.1) |
| Paired comparison per run | - | faster on 26-31 of 31 | faster on 21-31 of 31 |
| Mean saving per trajectory | - | 1.7 s | 2.0 s |

**Both scenes (one repetition = office + home):**

| | Original | Ygg table | Ygg on-demand |
|---|---|---|---|
| Whole repetition | 169.7 min (168.4-170.1) | 162.8 min (162.0-163.4) | 162.8 min (162.6-163.1) |

The scaling
argument is in the tables themselves: every "original" LP2 cost grows
with scene size (full-list scans), while the Yggdrasil costs stay
near-flat (pointer reads, index descents). So the performance gains of
yggdrasil become more dramatic as the scene size grows, and the two
scenes show exactly that: the office scene has 1,220 waypoints to home's
861, and the original's room lookup costs 987 µs there against 725 µs in
home (growing with the scene), while Yggdrasil's costs 1.1 µs in both.

## 6. Findings of independent interest

- **Three bugs in Yggdrasil's Python bindings**, found by this integration
  and fixed in Rust (branch `lp2-dijkstra`): layer-scoped spatial queries
  silently searched the whole graph; node IDs could not be serialized (LP2
  saves its prediction trees to disk); and iterating any spatial query
  result crashed instead of terminating cleanly. Candidates for upstreaming
  regardless of this project.
- **Prompt text depends on a hash-container artifact.** The original's
  objects-per-room listing follows the iteration order of a hash container
  inside the C++ scene graph library: deterministic but not derivable from
  scene content. Our converter records that order as an explicit attribute,
  making the ordering data rather than an implementation accident.
- **LLM error-handling retries are a real cost driver.** Under the
  original gpt-4o-mini cache a third of all traffic was retries (229 of
  556 calls): the small model frequently names rooms as interaction
  objects and must be re-asked, and 6 office trajectories died to it
  entirely. With gpt-5.6-luna the retry rate drops to 2% (13 of 570
  calls across both scenes) and no trajectory fails. Free under replay,
  but worth knowing for anyone running LP2 live.
- **The feared float-precision risk did not materialize**: positions narrow
  from double to single precision in Yggdrasil, yet 150k+ shortest-path
  comparisons agree to the last digit on both scenes.



## 7. Where things stand, and decisions needed from you

**Done:** integration, converter, Rust shortest paths, all correctness
gates (section 4), LLM caches for both scenes under one model, the full
benchmark campaign (5 runs per setting, both scenes, section 5),
documentation.

**Awaiting your direction:**

1. **Which Yggdrasil variant to headline.** Both are proven
   output-identical and both beat the original in every setting, so this
   is a design-taste call, not a correctness one:
   - *Table mode* mirrors the original's strategy (precompute once, look
     up thereafter): the cleanest like-for-like story, and the cheapest
     per trajectory (31 ms office / 21 ms home of graph time) (my own
     preference).
   - *On-demand mode* computes paths only when needed: no startup cost,
     which matters for a live robot's first prediction, at 183 ms /
     53 ms of graph time per trajectory. Our lean: table mode as the
     headline comparison, on-demand reported as the latency-oriented
     alternative.
2. **Upstreaming the Yggdrasil work.** The shortest-path implementation
   and the three binding fixes live on the `lp2-dijkstra` branch and are
   useful beyond this project; merge policy is yours.

