# Scene Graph Testing Strategy

## 1. Objective and Scope

The goal is to develop a testing strategy for Yggdrasil, starting with the `scene-graph` crate. The focus is on reviewing existing test coverage, identifying gaps, and proposing unit and integration tests to verify graph behavior and correctness.

This includes nodes, layers, relationships, spatial queries, coordinate updates, bounding boxes, and graph merging.

The initial testing work will be limited to `scene-graph`. The legacy engine, spatial-index fork, Python bindings, visualization, and benchmarks are outside the scope of this initial implementation. Future expansion to other Yggdrasil crates is addressed in Phase 6.

No new tests have been implemented yet.

## 2. Existing Tests

The `scene-graph` crate currently contains **45 inline `#[test]` declarations**, including one ignored scale test, and **6 Rust doctests**. These counts are based on source inspection; the tests have not yet been executed or measured for coverage.

Existing tests cover graph construction, node relationships, spatial queries, coordinate updates, visibility, bounding boxes, deletion, and graph conversion. Some inline tests also exercise interactions between multiple components.

All existing tests will remain in their current files.

### 2.1 Test Inventory

| Source File | Tests | Functionality Covered |
|---|---:|---|
| [`src/lib.rs`](src/lib.rs) | 5 | Graph construction, node relationships, directed edges, spatial queries, visibility, and bounding boxes |
| [`src/sg.rs`](src/sg.rs) | 11 | Subgraph extraction, node deletion, indexed and non-indexed queries, and visibility |
| [`src/layer.rs`](src/layer.rs) | 5 | Nearest-neighbor queries, radius queries, visibility, and coordinate-update consistency within a layer |
| [`src/pos.rs`](src/pos.rs) | 13 | Distance calculations, spatial indexing, indexed/non-indexed comparisons, point insertion, and removal |
| [`src/fov.rs`](src/fov.rs) | 10 | Field-of-view calculations, viewing directions, visibility boundaries, and invalid direction handling |
| [`src/parse/concept_graph.rs`](src/parse/concept_graph.rs) | 1 | Conversion of a ConceptGraph into nodes, layers, features, and edges |
| **Total** | **45** | |

### 2.2 Existing Test Functions

**`src/lib.rs`: 5 tests**

`api`, `query`, `fov`, `pos_query`, `bounds`

**`src/sg.rs`: 11 tests**

- `subgraph`

- `del_node_index_propagation`

- `nearest_in_subtree_unindexed_matches_indexed`

- `del_node`

- `visible_in_subtree_collects_every_visible_node_in_the_subtree`

- `visible_in_subtree_skips_nodes_without_coordinates`

- `visible_in_subtree_ignores_nodes_outside_the_subtree`

- `visible_in_subtree_of_an_unknown_node_is_empty`

- `bounds_rejection_does_agrees_with_visible_in_subtree`

- `visible_in_subtree_agrees_between_indexed_and_plain_roots`

- `visible_in_subtree_reports_only_visible_nodes_unlike_visible_subgraph`

**`src/layer.rs`: 5 tests**

- `fov_query`

- `nearest_query`

- `radius_query`

- `coordinate_edits_leave_no_stale_index_entries`

- `reordering_coordinates_preserves_the_indexed_point_set`

**`src/pos.rs`: 13 tests**

- `coplanar_plane_inserts_and_answers_queries`

- `quantized_lattice_room_inserts_and_answers_queries`

- `removing_coplanar_points_keeps_the_tree_queryable`

- `indexed_layer_accepts_coplanar_geometry`

- `identical_points_are_still_refused_at_bucket_capacity`

- `unindexed_backends_accept_the_same_geometry`

- `bucket_size_is_fixed_at_128_through_degenerate_data`

- `indexed_layer_holds_350k_lattice_points` **(ignored by default)**

- `vec_node_nearest_returns_true_distances`

- `vec_coords_nearest_returns_true_distances`

- `vec_coords_nodes_within_respects_radius`

- `all_backends_agree_on_nearest_nodes`

- `all_backends_agree_on_nodes_within`

**`src/fov.rs`: 10 tests**

- `classify_never_rejects_a_box_holding_a_visible_point`

- `classify_stays_conservative_for_a_cone_wider_than_a_hemisphere`

- `contains_agrees_with_observers`

- `classify_rejects_boxes_outside_the_frustum`

- `looking_along_is_frame_agnostic`

- `direction_length_does_not_change_membership`

- `looking_along_rejects_directions_with_no_axis`

- `looking_along_stores_the_direction_it_was_given`

- `cone_frustum_check`

- `cone_frustum_along_z`

**`src/parse/concept_graph.rs`: 1 test**

`parse_from_concept_graph`

The `pos.rs` tests are distributed across the `coplanar_geometry`, `scale`, and `test` modules. The 350,000-point scale test is explicitly ignored and does not run by default.

No inline test declarations were identified in `bounds.rs`, `node.rs`, `node_id.rs`, `error.rs`, or `parse/mod.rs`. There is currently no external `tests/` directory.

### 2.3 Doctests

The crate also contains six Rust documentation tests:

| Source | Operation | What It Verifies |
|---|---|---|
| [`sg.rs:229`](src/sg.rs#L229) | `nest` | Parent-child relationships after nesting |
| [`sg.rs:269`](src/sg.rs#L269) | `detach` | Removal of parent-child relationships |
| [`sg.rs:536`](src/sg.rs#L536) | `nearest_nodes` | Nearest-neighbor query results |
| [`sg.rs:578`](src/sg.rs#L578) | `nodes_within` | Radius-based spatial queries |
| [`sg.rs:623`](src/sg.rs#L623) | `nearest_in_subtree` | Nearest-neighbor queries within a subtree |
| [`fov.rs:51`](src/fov.rs#L51) | `Observer::new_looking_along` | Field-of-view inclusion and exclusion |

These examples verify public API usage and will remain in place.

### 2.4 Example Program

The [`examples/demo.rs`](examples/demo.rs) file demonstrates several public API workflows through four helper functions:

- `run_common_graph_queries`

- `run_field_of_view_queries`

- `run_nearest_neighbors_queris`

- `run_basic_label_retrieval_queries`

The example includes assertions, but it is not registered as a standard Rust test. Its assertions execute when the example program runs, rather than during ordinary library test execution.

### 2.5 Current Assessment

The crate already has meaningful test coverage across its main functionality. However, the number of tests alone does not establish how much of the code or its behavior is verified.

The next step is to establish a passing test baseline, measure coverage, and identify which additional unit and integration tests would provide the most value.

Specific coverage gaps and proposed test cases are addressed in **Section 6: Test Priorities**.

## 3. Testing Strategy

The testing strategy will use two main categories supported by Rust's built-in testing framework: **unit tests** and **integration tests**.

### 3.1 Unit Tests

Unit tests will remain inline within `scene-graph/src/`, alongside the code they test.

These tests focus on individual functions, algorithms, and internal behavior. They can also access private implementation details when necessary.

All existing inline tests will be preserved, including those that already test interactions between multiple components. Additional unit tests will be introduced where specific functionality is not sufficiently covered.

### 3.2 Integration Tests

Integration tests will be organized under `scene-graph/tests/` and exercise the crate through its public API.

These tests will focus on how different components work together, including:

- Node and layer relationships.

- Graph operations such as nesting, detaching, deletion, and merging.

- Spatial queries and coordinate updates.

- Bounding box and spatial index consistency.

- Error handling and graph state after failed operations.

Integration tests will verify observable behavior without relying on private implementation details.

### 3.3 Verification Approach

Both unit and integration tests will use the following techniques where appropriate:

| Technique | Purpose |
|---|---|
| API contract testing | Verify that public operations produce their expected results. |
| Invariant testing | Check that graph relationships and spatial data remain consistent after operations. |
| Regression testing | Prevent previously identified defects from recurring. |
| Differential testing | Compare results against independently calculated expectations or alternative implementations. |

For spatial queries, expected results will be calculated independently using simple coordinates and distance calculations rather than relying solely on the crate's own query functions.

Tests will use small, deterministic datasets with clearly defined expected results. Special cases such as duplicate coordinates, equal distances, and repeated graph operations will be tested according to the expected behavior established for the crate.

### 3.4 Scope and Organization

The strategy will use Rust's existing testing framework without introducing additional dependencies.

Documentation tests will remain in place. Separate end-to-end, smoke, or system test suites are not necessary for the current `scene-graph` scope.

The proposed test directory structure is described in **Section 5**, while specific test cases and priorities are covered in **Section 6**.

## 4. Coverage Strategy

The initial goal is to achieve **80% line coverage** for the `scene-graph` crate. This is a proposed target, not an existing project requirement, and may be adjusted after reviewing the baseline results.

Coverage will be measured using [`cargo-llvm-cov`](https\://github.com/taiki-e/cargo-llvm-cov), which provides source-level coverage reports for Rust projects.

### 4.1 Baseline Coverage

Before adding new tests, the existing test suite will be executed to establish a baseline.

The baseline will record:

- Number of passing, failing, and ignored tests.

- Current line coverage percentage and covered/total line counts.

- Rust, LLVM, and `cargo-llvm-cov` versions; selected features and test targets; and the source files included in the report.

- Source files with limited coverage.

- Any issues preventing test execution.

The following commands will be run from the repository root. `--offline` requires dependencies to be cached locally. On fresh GitHub Actions runners, omit `--offline` and use normal Cargo dependency resolution with `--locked`; a cache hit must not be assumed.

```bash
# Run unit and integration tests
cargo test -p scene-graph --lib --tests --locked --offline
# Run documentation tests
cargo test -p scene-graph --doc --locked --offline
# Generate an HTML coverage report
cargo llvm-cov -p scene-graph --lib --tests --locked --offline \
  --ignore-filename-regex '(^|/)(spatial-index|engine|python-binding|viz|bench)/' \
  --html --output-dir target/scene-graph-coverage/baseline
```

Coverage reports will focus on `scene-graph/src/`. Documentation tests will be executed separately and excluded from the initial coverage measurement.

### 4.2 Coverage Improvement

After implementing the proposed unit and integration tests, the same coverage configuration will be used to generate an updated report.

The results will be compared against the baseline to determine:

- Whether overall line coverage improved.

- Which previously uncovered code paths are now exercised.

- Which important behaviors still require additional testing.

- Whether the proposed 80% target has been reached.

Both reports will be retained to document the changes. Keep the baseline at `target/scene-graph-coverage/baseline` and generate the follow-up report at `target/scene-graph-coverage/follow-up` by changing `--output-dir`. Record the same versions, features, targets, included files, and covered/total lines for both runs, explaining any configuration or denominator changes.

### 4.3 Coverage Requirements

Line coverage measures which source lines execute during testing. It does not guarantee that the results are correct.

Tests must also verify graph relationships, spatial queries, coordinate consistency, bounding boxes, and error handling through explicit assertions.

Because inline test code can contribute to the reported line totals, the coverage report will be reviewed to distinguish test code from production behavior.

The proposed 80% threshold will initially be used as a goal rather than a requirement. Once the target and measurement approach are agreed upon, it may become an enforced coverage requirement.

### 4.4 GitHub Actions

The existing [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) already runs `cargo test --workspace --locked`, including unit tests, integration tests, and doctests, on pushes to `master` and pull requests targeting `master`. Extend that workflow with focused `scene-graph` coverage reporting and uploaded report artifacts, preserving the existing workspace checks rather than duplicating them.

The workflow will retain its existing triggers, with optional development-branch support:

- **Pushes to `master`:** Verify that changes do not break existing tests.

- **Pull requests targeting `master`:** Run tests before changes are merged.

- **Other development branches (optional):** Allow additional branches to use the same automated checks.

Each workflow run will:

1. Preserve the existing workspace unit, integration, and doctest checks.

2. Provision a recorded `cargo-llvm-cov` version and the matching Rust toolchain's `llvm-tools-preview` component for coverage measurement.

3. Run the focused `scene-graph` tests under coverage instrumentation and generate a report, using normal Cargo dependency resolution on fresh runners. This instrumented run measures coverage; no additional uninstrumented duplicate test job is needed.

4. Keep test results in the Actions logs and upload coverage reports as artifacts with an explicit retention period.

5. Mark the workflow as failed if any required tests fail.

Initially, coverage results will be informational. Once the proposed 80% line coverage target is approved and the baseline is established, the workflow may enforce the agreed threshold.

The workflow is intended for **automated testing and coverage verification**, not application deployment or a separate build pipeline. Rust test targets will be compiled as part of normal test execution.

The goal is to maintain confidence that changes to `scene-graph` continue to satisfy the behavior verified by its tests.

## 5. Proposed Test Structure

Existing unit tests will remain within `scene-graph/src/`. New integration tests will be organized under `scene-graph/tests/` to verify the crate's public API and interactions between components.

The proposed directory structure is:

```text

scene-graph/

├── src/

│   └── ... existing inline tests

├── tests/

│   ├── graph_contract.rs

│   ├── spatial_consistency.rs

│   ├── merge_contract.rs

│   └── common/

│       └── mod.rs

└── TESTING_STRATEGY.md

```

### 5.1 Integration Tests

| Test File | Responsibility | Related Test Cases |
|---|---|---|
| `graph_contract.rs` | Verify node identity, layer membership, directed edges, node deletion, and error handling. | G1–G4 |
| `spatial_consistency.rs` | Verify spatial queries, coordinate updates, bounding boxes, reparenting, and consistency after graph operations. | S1–S7 |
| `merge_contract.rs` | Verify graph merging, node identity, layer relationships, and preservation of graph data. | M1–M3 |
| `common/mod.rs` | Provide shared test fixtures and helper functions where needed. | Shared across integration tests |

Each integration test file will exercise the crate through its public API. Shared helpers will be introduced only when multiple tests require the same setup or verification logic.

### 5.2 Unit Tests

Existing inline tests will remain in their current source files. Additional unit tests may be added to the following modules:

| Source File | Proposed Test | Purpose |
|---|---|---|
| `src/node.rs` | N1 | Verify child-node ordering and consistency after removal and insertion. |
| `src/bounds.rs` | B1 | Verify bounding box calculations and boundary conditions. |
| `src/parse/concept_graph.rs` | C1 | Verify that graph conversion preserves the expected node, feature, and edge data. |

These tests will focus on functionality that benefits from direct access to internal implementation details.

### 5.3 Implementation Notes

The proposed structure uses Rust's existing testing framework and does not require additional testing dependencies or changes to the crate's public API.

Existing tests and doctests will be preserved. New test files will be introduced during implementation, after the testing strategy has been reviewed.

Specific test cases and their priorities are described in **Section 6: Test Priorities**.

## 6. Test Priorities

The proposed tests are organized into two priority levels:

- **P0: Initial Priority:** Tests addressing the most important gaps in graph correctness, spatial consistency, merging, and error handling.

- **P1: Follow-up Priority:** Additional tests for API behavior, edge cases, and internal functionality.

These priorities are based on the existing test inventory and source review. Some expected behaviors still require clarification before tests can be implemented.

### 6.1 P0: Initial Priority

| ID | Proposed Test | What It Verifies |
|---|---|---|
| S1 | `queries_match_independent_scan` | Spatial queries return the correct nodes and distances when compared against independently calculated results, including empty results and distance ties. |
| S2 | `coordinate_edits_refresh_queries_bounds_and_visibility` | Changing a node's coordinates correctly updates spatial queries, bounding boxes, and visibility results throughout the graph. |
| S3 | `reparent_preserves_subtree_and_updates_ancestors` | Moving a subtree between parents preserves its descendants and updates the relationships, spatial queries, and bounds of affected ancestors. |
| S4 | `repeat_nest_and_detach_are_consistent` | Repeated nesting and detachment do not introduce duplicate relationships or stale spatial data. |
| M1 | `merged_node_does_not_collide_with_next_allocation` | Merging graphs preserves node identities and does not cause subsequent node ID collisions. |
| M2 | `merge_preserves_supported_layers_and_parented_additions` | Graph merging handles supported layers, parent-child relationships, and node identity conflicts correctly. |
| G3 | `missing_parent_error_preserves_graph_state` | An invalid parent reference returns the expected error without leaving the graph in an inconsistent state. |
| S6 | `failed_coordinate_push_has_no_ghost_hits` | First establish a reproducible failure through the public insertion API, then verify the agreed post-failure state contains no invalid spatial-index entries or query hits. A dependency-level insertion failure alone does not establish this precondition. |

**Implementation order:** Begin with S1 to establish independently calculated spatial-query expectations. Follow with coordinate updates and hierarchy consistency, then merging and error handling.

For S2–S4, M1–M2, G3, and S6, the exact expected behavior must be confirmed before finalizing the assertions.

### 6.2 P1: Follow-up Priority

| ID | Proposed Test | What It Verifies |
|---|---|---|
| G1 | `identity_and_layer_membership_survive_lifecycle` | IDs from supported factory-generated allocations remain unique and correctly associated with their layers throughout creation, deletion, and reallocation; this does not assume uniqueness for arbitrary manually supplied IDs. |
| G2 | `directed_edges_reject_invalid_endpoints` | Directed edges preserve their direction, reject invalid endpoints, and return appropriate errors. |
| G4 | `cascade_delete_cleans_derived_state_and_edges` | Deleting a node and its descendants correctly updates relationships, edges, spatial queries, and bounds. |
| S5 | `root_and_multicoordinate_query_membership` | Spatial queries handle root-owned coordinates and nodes with multiple coordinates according to the agreed behavior. |
| S7 | `visible_copy_obeys_graph_closure_policy` | Extracted visible subgraphs maintain valid node relationships, spatial data, and references. |
| M3 | `merge_updates_features_coordinates_and_selected_edges` | Graph merging correctly updates features, coordinates, and directed edges according to the expected merge behavior. |
| N1 | `child_search_invariant_survives_removal` | Child-node collections preserve the ordering required by internal search after insertions and removals. This is an internal invariant, not a public API ordering guarantee. |
| B1 | `bounds_value_semantics` | Bounding boxes correctly handle empty bounds, coordinate limits, merging, and boundary conditions. |
| C1 | `conversion_preserves_exact_records` | ConceptGraph conversion preserves node data, features, coordinates, and edges rather than only matching expected counts. |

Tests G1–G4, S1–S7, and M1–M3 will be implemented as integration tests under `scene-graph/tests/`.

Tests N1, B1, and C1 will remain inline within their respective source modules, as described in Section 5.

### 6.3 Existing Coverage and Identified Gaps

Several existing tests already verify successful graph operations. The proposed tests are intended to extend that coverage rather than duplicate it.

| Area | Existing Coverage | Additional Verification Needed |
|---|---|---|
| Spatial queries | Distance calculations, nearest-neighbor queries, radius queries, and indexed/non-indexed comparisons | Independently calculated expected results and boundary cases |
| Coordinate updates | Layer-level index updates after coordinate changes | Consistency across ancestor indexes, bounding boxes, and visibility |
| Graph relationships | Nesting, detachment, reparenting, and deletion | Repeated operations and updates to old and new ancestors |
| Graph merging | No direct `SceneGraph::merge` tests identified | Node identity, layer preservation, data updates, and collision handling |
| Error handling | Selected invalid inputs and missing-node queries | Graph consistency after failed mutations |
| Graph conversion | Node, layer, feature, and edge counts | Exact preservation of converted data |

Source inspection also identified potential consistency issues involving coordinate mutations, reparenting, merging, and failed insertions. These require tests to determine whether the behavior is correct; they should not be treated as confirmed defects.

### 6.4 Expected Results

Tests will use small, deterministic graphs with known nodes, coordinates, and relationships.

For spatial queries, expected results will be calculated independently rather than relying solely on comparisons between existing implementations.

For graph mutations, tests will compare the graph's state before and after an operation, including node identities, parent-child relationships, edges, spatial queries, and bounding boxes.

Where behavior is not yet clearly defined, the expected results will be established before implementation. The outstanding decisions are listed in **Section 8: Questions Before Implementation**.

## 7. Implementation Phases

The testing strategy will be implemented in six phases. The first five phases focus on establishing and validating the testing approach for `scene-graph`. The sixth phase extends the approach to other Yggdrasil crates.

### Phase 1: Establish Baseline

Run the existing unit tests, integration-style tests, and doctests to establish the current testing baseline.

- Record passing, failing, and ignored tests.

- Measure initial line coverage using `cargo-llvm-cov`.

- Identify uncovered source files and important functionality.

- Document any issues preventing test execution.

The baseline will serve as the reference for evaluating improvements. Review the existing `.github/workflows/ci.yml` workspace checks on pushes to `master` and pull requests targeting `master`; use those checks without introducing a duplicate test workflow. Plan the coverage-tool provisioning and report artifacts described in Section 4.4.

### Phase 2: Unit and Integration Tests

Implement the initial tests that have clearly defined expected results.

- Begin with S1 to verify spatial queries against independently calculated results.

- Add G1 and G2 to verify node identity, layer membership, and directed edges.

- Add targeted unit tests N1, B1, and C1 where appropriate.

- Introduce shared test helpers only when necessary.

All existing tests will remain in place.

### Phase 3: Graph Mutation and Merge Tests

Implement the remaining tests involving graph mutations, spatial consistency, merging, and error handling.

- Prioritize S2–S4, M1–M2, G3, and S6.

- Follow with G4, S5, S7, and M3.

- Confirm expected behavior for operations that require clarification.

- Document any failures or unexpected results discovered during testing.

Potential defects will be reported separately rather than changing test expectations to accommodate incorrect behavior.

### Phase 4: Coverage Verification

Run the complete test suite and generate an updated coverage report using the same configuration established in Phase 1.

- Compare baseline and updated line coverage.

- Review remaining uncovered functionality.

- Verify that the tests include meaningful assertions.

- Evaluate progress toward the proposed **80% line coverage target**.

Coverage results will be documented alongside the test results.

### Phase 5: GitHub Actions and Pull Request

Extend the existing `.github/workflows/ci.yml` with `scene-graph` coverage reporting and uploaded artifacts, preserving its workspace test checks. Provision `cargo-llvm-cov` and the matching `llvm-tools-preview` component as described in Section 4.4; omit `--offline` on fresh runners.

- Preserve the existing test triggers for pushes to `master` and pull requests targeting `master`.

- Optionally extend automated checks to additional development branches.

- Report test failures through GitHub Actions and upload coverage reports as retained artifacts.

- Initially report coverage without enforcing the proposed threshold.

- Enable coverage enforcement only after the target is approved.

Once the testing changes have been verified, submit a pull request containing the new tests, documented results, and coverage improvements.

The pull request will remain focused on testing, without unrelated production code changes or refactoring.

### Phase 6: Expand Testing Across Yggdrasil

After completing and validating the `scene-graph` testing strategy, evaluate the remaining Yggdrasil crates for future testing improvements.

- Review existing tests and identify coverage gaps in each crate.

- Apply the unit and integration testing approach established for `scene-graph` where appropriate.

- Establish separate coverage baselines and targets based on each crate's functionality.

- Build on the existing workspace test checks as new tests are introduced; add crate-specific coverage reporting or checks only where needed.

- Prioritize future testing based on functionality, dependencies, and project needs.

Potential future targets include `spatial-index`, `engine`, `python-binding`, `viz`, and `bench`. Direct testing of the `spatial-index` fork would require separate coordination because of its upstream relationship.

Each crate will be evaluated independently rather than assuming the same test structure or coverage requirements apply everywhere.

This phase is a proposed future extension and is not part of the initial `scene-graph` testing contribution.

## 8. Questions Before Implementation

Some proposed tests require clarification of the expected behavior before they can be implemented. These questions will help ensure that tests verify the intended functionality rather than assumptions about how the code should behave.

| Area | Questions | Related Tests |
|---|---|---|
| Coordinate updates | Should changes to a node's coordinates immediately update its bounding box and all affected ancestor spatial indexes? | S2, S3 |
| Nesting and reparenting | What should happen when a node is nested under the same parent multiple times or detached repeatedly? Should reparenting always remove the node's previous spatial references? | S3, S4 |
| Graph merging | How should merging handle duplicate node IDs, new layers, parent-child relationships, and conflicting features or edges? | M1–M3 |
| Failed operations | Should an unsuccessful operation leave the graph completely unchanged? How should partially completed operations be handled? | G3, S6 |
| Spatial queries | Should queries return unique nodes or individual coordinate occurrences? Should subtree queries include coordinates belonging to the root node? | S5 |
| Subgraph extraction | Should extracted subgraphs be fully independent, with all relationships and spatial references contained within the resulting graph? | S7 |
| Coverage requirements | Is 80% line coverage an appropriate initial target? Should GitHub Actions enforce this threshold once the baseline and additional tests are established? | Sections 4, 7 |

These questions do not need to be resolved simultaneously. Tests with clearly defined expected results can proceed first, while tests involving ambiguous behavior will be finalized once the relevant requirements are established.

## 9. Acceptance Criteria

The testing strategy will be evaluated against the following criteria.

### 9.1 Test Implementation

- Existing unit tests and doctests are preserved.

- Proposed unit and integration tests are implemented according to the agreed priorities and expected behavior.

- Tests verify graph relationships, node identity, spatial queries, coordinate updates, bounding boxes, merging, and error handling.

- Tests use deterministic inputs and independently calculated expected results where appropriate.

- Any unclear behavior identified in Section 8 is resolved before the corresponding tests are finalized.

### 9.2 Test Execution and Coverage

- All required unit tests, integration tests, and doctests pass.

- Test results are documented, including any ignored tests.

- Baseline and updated coverage reports are generated using `cargo-llvm-cov`.

- Coverage improvements are measured using a consistent configuration.

- The agreed line coverage target is met, with **80% proposed as the initial goal**.

- Important functionality is verified through meaningful assertions, not coverage percentage alone.

### 9.3 GitHub Actions

- GitHub Actions automatically runs the `scene-graph` test suite on pushes to `master` and pull requests targeting `master`.

- Test failures cause the workflow to fail.

- Coverage reports are generated and available for review.

- Coverage enforcement is enabled only after the target and measurement approach are approved.

### 9.4 Pull Request and Review

- Testing changes are submitted through a focused pull request.

- The pull request includes test results, coverage improvements, and any identified issues.

- No unrelated production code changes, public API modifications, or unnecessary dependencies are introduced.

- Any defects discovered during testing are documented for separate review and resolution.

### 9.5 Expansion Across Yggdrasil

After the `scene-graph` testing approach has been validated, the remaining crates will be evaluated for additional testing.

- Review existing tests and establish coverage baselines for `spatial-index`, `engine`, `python-binding`, `viz`, and `bench`, as applicable.

- Identify missing unit and integration tests based on each crate's functionality.

- Establish appropriate coverage targets for each crate rather than automatically applying the `scene-graph` threshold.

- Implement additional tests without unnecessarily duplicating existing coverage.

- Extend GitHub Actions to run the relevant tests for each crate.

- Document test results, coverage improvements, and any identified defects.

- Submit focused pull requests for each crate or related group of changes.

Direct modifications to the `spatial-index` fork will be coordinated separately because of its upstream relationship.

### 9.6 Completion Criteria

**Initial milestone:** Phases 1–5 are complete when the `scene-graph` tests pass, coverage meets the approved target, GitHub Actions verification is operational, and the testing contribution has been reviewed and accepted.

**Project-wide milestone:** Phase 6 is complete when the agreed testing work for the remaining Yggdrasil crates has been implemented, verified through GitHub Actions, and reviewed.

The goal is to establish a consistent testing approach across Yggdrasil while allowing each crate to have testing requirements appropriate to its functionality.
