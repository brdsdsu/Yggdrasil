use std::num::NonZero;

use spatial_index::{EytzingerAdaptive, KdTree, SquaredEuclidean, VecOfArrays, dist::DistanceMetricScalar};

use crate::NodeId;

use super::{Coordinate, Node};

/// Bucket size of the position index: the maximum number of points a single leaf of the
/// kd-tree holds before it is split.
///
/// This is a deliberate memory-versus-latency choice made by this crate, not a tuning knob
/// the tree is free to change on its own: leaves are fixed-size arrays, so every leaf costs
/// this many slots regardless of how many are occupied. Position indexing is opt-in
/// precisely so that a caller can decide to pay for it.
pub(crate) const POS_INDEX_BUCKET_SIZE: usize = 128;

/// The kd-tree configuration backing every position index in this crate.
///
/// `EytzingerAdaptive` is the breadth-first stem layout, splitting on the widest axis, and `VecOfArrays` is the mutable leaf
/// strategy, which is what makes incremental `add`/`remove` available — the position
/// indexes are maintained point by point as the graph changes, never rebuilt in bulk.
pub(crate) type PosIndex = KdTree<
    f32,
    NodeId,
    EytzingerAdaptive,
    VecOfArrays<f32, NodeId, 3, POS_INDEX_BUCKET_SIZE>,
    3,
    POS_INDEX_BUCKET_SIZE,
>;

/// Specifies the number of nodes to return in position-based queries.
#[derive(Debug, Clone, Copy)]
pub enum PosQueryCount {
    /// Return All matching nodes.
    All,
    /// Return a specific Count of matching nodes.
    Count(usize),
}

impl PosQueryCount {
    /// Convert to usize, capping at the provided maximum value.
    pub(crate) fn capped_with(&self, cap: usize) -> usize {
        match self {
            PosQueryCount::All => cap,
            PosQueryCount::Count(k) => std::cmp::min(*k, cap),
        }
    }
}

pub(crate) trait PositionQuery {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)>;

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)>;
}

impl PositionQuery for PosIndex {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)> {
        match count {
            PosQueryCount::All => {
                let query: [f32; 3] = (*pos).into();
                let mut distances: Vec<_> = self
                    .iter()
                    .map(|(id, coords)| {
                        (
                            id,
                            <SquaredEuclidean<f32> as DistanceMetricScalar<f32>>::dist(
                                &coords, &query,
                            ),
                        )
                    })
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect();

                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances
            }
            PosQueryCount::Count(_) => {
                let count = match NonZero::new(count.capped_with(self.size())) {
                    Some(c) => c,
                    None => return Vec::new(),
                };
                self.query(&(*pos).into())
                    .nearest_n::<SquaredEuclidean<f32>>(count)
                    .execute()
                    .into_iter()
                    .map(|n| (n.item, n.distance.sqrt()))
                    .collect()
            }
        }
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)> {
        // Distance is SquaredEuclidean<f32>, so we need to square the radius for the query.
        let radius_squared = radius * radius;
        let results = match count {
            PosQueryCount::All => self
                .query(&(*pos).into())
                .within::<SquaredEuclidean<f32>>(radius_squared)
                .execute(),
            PosQueryCount::Count(_) => {
                let count = match NonZero::new(count.capped_with(self.size())) {
                    Some(count) => count,
                    None => return Vec::new(),
                };
                self.query(&(*pos).into())
                    .nearest_n::<SquaredEuclidean<f32>>(count)
                    .within(radius_squared)
                    .execute()
            }
        };
        results
            .into_iter()
            .map(|n| (n.item, n.distance.sqrt()))
            .collect()
    }
}

impl PositionQuery for Vec<Node> {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)> {
        match count {
            PosQueryCount::All => {
                let mut distances: Vec<_> = self
                    .iter()
                    .filter(|n| n.coordinates.is_some())
                    .map(|n| (n.id, n.coordinates.unwrap().distance_squared(*pos)))
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances
            }
            PosQueryCount::Count(c) => {
                let mut distances = self
                    .iter()
                    .filter(|n| n.coordinates.is_some())
                    .map(|n| (n, n.coordinates.unwrap().distance_squared(*pos)))
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect::<Vec<(&Node, f32)>>();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances
                    .into_iter()
                    .map(|(node, dist)| (node.id, dist))
                    .take(c)
                    .collect()
            }
        }
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)> {
        let count = count.capped_with(self.len());
        let mut distances = self
            .iter()
            .filter(|n| n.coordinates.is_some())
            .map(|n| (n, n.coordinates.unwrap().distance_squared(*pos)))
            .filter(|(_, d)| *d <= radius * radius)
            .map(|(node, dist)| (node.id, dist))
            .collect::<Vec<(NodeId, f32)>>();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances
            .into_iter()
            .take(count)
            .map(|(id, d)| (id, d.sqrt()))
            .collect::<Vec<_>>()
    }
}

impl PositionQuery for Vec<(Coordinate, NodeId)> {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)> {
        match count {
            PosQueryCount::All => {
                let mut distances: Vec<(NodeId, f32)> = self
                    .iter()
                    .map(|(c, id)| (*id, c.distance_squared(*pos).sqrt()))
                    .collect();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances
            }
            PosQueryCount::Count(c) => {
                let mut distances = self
                    .iter()
                    .map(|(c, id)| (*id, c.distance_squared(*pos)))
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect::<Vec<_>>();
                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances.into_iter().take(c).collect()
            }
        }
    }

    /// Retrieve at most `count` nodes within a specified radius to the given position.
    /// If this function is called frequently, consider creating the Layer with indexing enabled.
    fn nodes_within(
        &self,
        pos: &super::Coordinate,
        radius: f32,
        count: PosQueryCount,
    ) -> Vec<(NodeId, f32)> {
        let count = count.capped_with(self.len());
        let mut distances = self
            .iter()
            .map(|(c, id)| (*id, c.distance(*pos)))
            .filter(|(_, d)| *d <= radius)
            .collect::<Vec<_>>();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.into_iter().take(count).collect::<Vec<_>>()
    }
}

/// Tests for coplanar and lattice-quantised geometry — the input that upstream kiddo cannot
/// insert at all.
///
/// The kd-tree picks the axis it splits a full leaf on. Upstream derives it from tree depth, so a
/// leaf that fills with points sharing one value on the cycled axis has no plane to cut at and the
/// insert is refused. That is not an exotic input for this crate: reconstructed indoor geometry
/// snapped to a voxel lattice puts thousands of points on a single floor, wall or ceiling plane,
/// far past a 128-point bucket. `spatial-index` chooses the axis by widest spread instead, and
/// records it in the split value so queries agree.
///
/// These began as characterization tests asserting the refusal, and were rewritten when the
/// adaptive split landed. What they now pin is that the geometry inserts *and* that queries over
/// it match a brute-force scan — the failure mode a mis-recorded axis would produce is silently
/// wrong neighbours, not an error.
#[cfg(test)]
mod coplanar_geometry {
    use super::*;
    use crate::layer::Layer;
    use crate::node::AnonNode;

    /// Insert every point into a fresh index, reporting the first refusal.
    fn insert_all(points: &[[f32; 3]]) -> Result<PosIndex, (usize, usize)> {
        let mut tree = PosIndex::default();
        for (i, p) in points.iter().enumerate() {
            if let Err(spatial_index::kd_tree::ConstructionError::UnsplittableBucket {
                split_dim,
            }) = tree.add(p, NodeId::new(i as u32, 0))
            {
                return Err((i, split_dim));
            }
        }
        Ok(tree)
    }

    /// The `count` nearest points to `query` by exhaustive scan, as `(index, distance)`.
    fn brute_force(points: &[[f32; 3]], query: [f32; 3], count: usize) -> Vec<(u32, f32)> {
        let mut d: Vec<(u32, f32)> = points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let s: f32 = (0..3).map(|k| (p[k] - query[k]).powi(2)).sum();
                (i as u32, s)
            })
            .collect();
        d.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.0.cmp(&b.0)));
        d.truncate(count);
        d
    }

    /// Every query must agree with brute force on distances. Ids are compared only where the
    /// distance is unambiguous, since ties may be broken either way.
    #[track_caller]
    fn assert_matches_brute_force(tree: &PosIndex, points: &[[f32; 3]], query: [f32; 3]) {
        for count in [1usize, 5, 40, 200] {
            let count = count.min(points.len());
            let want = brute_force(points, query, count);
            let got = tree
                .query(&query)
                .nearest_n::<SquaredEuclidean<f32>>(NonZero::new(count).unwrap())
                .execute();
            assert_eq!(got.len(), want.len(), "count {count}: wrong result length");
            for (g, w) in got.iter().zip(want.iter()) {
                assert!(
                    (g.distance - w.1).abs() <= 1e-6 * w.1.max(1.0),
                    "count {count}: got distance {} want {}",
                    g.distance,
                    w.1
                );
            }
        }
    }

    /// 500 points spread over x and y but sharing one z: a single ceiling plane. Upstream refuses
    /// insert #348 of these, on the shared axis.
    #[test]
    fn coplanar_plane_inserts_and_answers_queries() {
        let plane: Vec<[f32; 3]> = (0..500)
            .map(|i| [(i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85])
            .collect();

        let tree = match insert_all(&plane) {
            Ok(t) => t,
            Err((i, d)) => panic!("refused point {i} on axis {d}"),
        };
        assert_eq!(tree.size(), plane.len());

        for query in [[0.0, 0.0, 3.85], [0.6, 0.4, 3.85], [10.0, -10.0, 0.0]] {
            assert_matches_brute_force(&tree, &plane, query);
        }
    }

    /// The realistic shape: a room on a 0.05 m lattice, floor plus two walls, each surface one
    /// plane holding thousands of points. Upstream stops 208 points into 12,800.
    #[test]
    fn quantized_lattice_room_inserts_and_answers_queries() {
        const GRID: usize = 80;
        let mut room: Vec<[f32; 3]> = Vec::new();
        for i in 0..GRID {
            for j in 0..GRID {
                room.push([i as f32 * 0.05, 0.0, j as f32 * 0.05]);
            }
        }
        for i in 0..GRID {
            for k in 0..GRID / 2 {
                room.push([i as f32 * 0.05, k as f32 * 0.05, 0.0]);
                room.push([0.0, k as f32 * 0.05, i as f32 * 0.05]);
            }
        }

        let tree = match insert_all(&room) {
            Ok(t) => t,
            Err((i, d)) => panic!("refused point {i} of {} on axis {d}", room.len()),
        };
        assert_eq!(tree.size(), room.len());

        for query in [[1.0, 1.0, 1.0], [0.0, 0.0, 0.0], [2.0, 0.05, 3.9]] {
            assert_matches_brute_force(&tree, &room, query);
        }
    }

    /// Removal over coplanar data: removed points must stop being returned, and the tree must stay
    /// queryable afterwards. Exercises the paths that locate a leaf by descent, which is where a
    /// mis-recorded split axis would send the walk down the wrong subtree.
    #[test]
    fn removing_coplanar_points_keeps_the_tree_queryable() {
        let plane: Vec<[f32; 3]> = (0..500)
            .map(|i| [(i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85])
            .collect();
        let mut tree = insert_all(&plane).expect("coplanar insert must succeed");

        let removed: Vec<usize> = (0..plane.len()).step_by(3).collect();
        for &i in &removed {
            tree.remove(&plane[i], NodeId::new(i as u32, 0));
        }
        assert_eq!(tree.size(), plane.len() - removed.len());

        let survivors: Vec<[f32; 3]> = plane
            .iter()
            .enumerate()
            .filter(|(i, _)| !removed.contains(i))
            .map(|(_, p)| *p)
            .collect();

        // No removed id may come back, at any radius.
        let gone: Vec<NodeId> = removed.iter().map(|&i| NodeId::new(i as u32, 0)).collect();
        let found = tree
            .query(&[0.6, 0.4, 3.85])
            .within::<SquaredEuclidean<f32>>(100.0)
            .execute();
        assert_eq!(found.len(), survivors.len(), "radius query lost survivors");
        for r in &found {
            assert!(!gone.contains(&r.item), "removed {} came back", r.item);
        }

        // And the tree still answers correctly over what is left.
        let want = brute_force(&survivors, [0.6, 0.4, 3.85], 10);
        let got = tree
            .query(&[0.6, 0.4, 3.85])
            .nearest_n::<SquaredEuclidean<f32>>(NonZero::new(10).unwrap())
            .execute();
        for (g, w) in got.iter().zip(want.iter()) {
            assert!((g.distance - w.1).abs() <= 1e-6 * w.1.max(1.0));
        }
    }

    /// An indexed layer holds the same geometry through the public API.
    #[test]
    fn indexed_layer_accepts_coplanar_geometry() {
        let mut layer = Layer::with_pos_index(0);
        for i in 0..500u32 {
            let pos = Coordinate::new((i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85);
            layer
                .push_node(AnonNode::new(i, Vec::new(), Some(pos), false, None))
                .expect("an indexed layer must accept coplanar geometry");
        }
        assert_eq!(layer.all_nodes().len(), 500);

        let found = layer.nearest_nodes(&Coordinate::new(0.0, 0.0, 3.85), PosQueryCount::Count(4));
        assert_eq!(found.len(), 4);
        // Nearest to the corner is the corner itself.
        assert!(found[0].1 <= 1e-6);
    }

    /// The one case a better axis choice cannot fix: points identical on *every* axis have no
    /// separating plane at all, so the bucket is refused once full. Storing them needs an overflow
    /// representation rather than a different split — deliberately still outstanding, and pinned
    /// here so that work has a failing test to turn green.
    #[test]
    fn identical_points_are_still_refused_at_bucket_capacity() {
        match insert_all(&[[1.0, 2.0, 3.0]; 300]) {
            Ok(_) => panic!("expected refusal: no plane separates identical points"),
            Err((accepted, _)) => assert_eq!(
                accepted, POS_INDEX_BUCKET_SIZE,
                "the bucket fills, then the next insert has nowhere to go"
            ),
        }
    }

    /// The unindexed backends handle the same geometry, as they always did.
    #[test]
    fn unindexed_backends_accept_the_same_geometry() {
        let mut layer = Layer::new(0);
        for i in 0..500u32 {
            let pos = Coordinate::new((i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85);
            layer
                .push_node(AnonNode::new(i, Vec::new(), Some(pos), false, None))
                .expect("an unindexed layer has no bucket to split");
        }
        assert_eq!(layer.all_nodes().len(), 500);
        let found = layer.nearest_nodes(&Coordinate::new(0.0, 0.0, 3.85), PosQueryCount::Count(4));
        assert_eq!(found.len(), 4);
    }

    /// The bucket size is a deliberate memory-versus-latency choice by this crate, not something
    /// the tree may adjust for itself. Degenerate data must not change it — increasing it is the
    /// fix upstream suggests for the coplanar refusal, and the one this crate does not take.
    #[test]
    fn bucket_size_is_fixed_at_128_through_degenerate_data() {
        assert_eq!(POS_INDEX_BUCKET_SIZE, 128);

        let mut tree = PosIndex::default();
        for i in 0..400u32 {
            let _ = tree.add(&[0.0, 0.0, 3.85], NodeId::new(i, 0));
        }
        // Whatever the tree did with that input, its configuration is a compile-time constant and
        // its live count can never exceed what it accepted.
        assert_eq!(POS_INDEX_BUCKET_SIZE, 128);
        assert!(tree.size() <= 400);
    }
}

/// Scale test for the position index, at the size real scenes reach.
///
/// Nothing else in this crate builds an index this large: `parse/concept_graph.rs` puts its point
/// cloud in an *unindexed* layer and `examples/demo.rs` uses 100 points, which is why the coplanar
/// defect went unnoticed for so long. This is the test that would have caught it.
#[cfg(test)]
mod scale {
    use super::*;
    use crate::layer::Layer;
    use crate::node::AnonNode;

    /// Points on a 0.05 m lattice filling a room-sized volume, emitted in a scattered order.
    ///
    /// The order matters and is not incidental. Lattice points in scan order arrive spatially
    /// clustered, which drives tree depth rather than breadth — and stems are addressed by
    /// breadth-first position, so that costs `2^depth` slots and exhausts memory well before
    /// 350,000 points. That is a separate, unfixed defect (see `bench/BASELINE.md`), so this test
    /// deliberately interleaves the points instead of pretending it does not exist.
    fn scattered_lattice(count: usize) -> Vec<Coordinate> {
        // A stride coprime with the span walks every cell exactly once in a scattered order,
        // which is deterministic without needing a PRNG.
        const SPAN: usize = 71;
        let cells = SPAN * SPAN * SPAN;
        let stride = 47_351usize;
        assert!(count <= cells);
        (0..count)
            .map(|i| {
                let c = (i.wrapping_mul(stride)) % cells;
                Coordinate::new(
                    (c % SPAN) as f32 * 0.05,
                    ((c / SPAN) % SPAN) as f32 * 0.05,
                    ((c / SPAN / SPAN) % SPAN) as f32 * 0.05,
                )
            })
            .collect()
    }

    /// Build an indexed layer of ~350,000 lattice points and query it.
    ///
    /// Slow in a debug build, so it is ignored by default:
    /// `cargo test -p scene-graph --release -- --ignored`
    #[test]
    #[ignore = "350k inserts; run explicitly, ideally in release"]
    fn indexed_layer_holds_350k_lattice_points() {
        let points = scattered_lattice(350_000);
        let mut layer = Layer::with_pos_index(0);

        for (i, p) in points.iter().enumerate() {
            layer
                .push_node(AnonNode::new(i as u32, Vec::new(), Some(*p), false, None))
                .unwrap_or_else(|e| panic!("insert {i} of {} refused: {e}", points.len()));
        }
        assert_eq!(layer.all_nodes().len(), points.len());

        // A query at a lattice site must find that site at distance zero, and return neighbours in
        // nondecreasing order of distance.
        let probe = points[points.len() / 2];
        let found = layer.nearest_nodes(&probe, PosQueryCount::Count(16));
        assert_eq!(found.len(), 16);
        assert!(found[0].1 <= 1e-6, "nearest to a lattice site is itself");
        for w in found.windows(2) {
            assert!(w[0].1 <= w[1].1 + 1e-6, "results not ordered by distance");
        }

        // And a radius query must agree with the same radius measured by brute force.
        let radius = 0.12f32;
        let within = layer.nodes_within(&probe, radius, PosQueryCount::All);
        let expected = points
            .iter()
            .filter(|q| q.distance(probe) <= radius + 1e-6)
            .count();
        assert_eq!(within.len(), expected, "radius query lost or gained points");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Coordinate;

    /// Node ids are all placed on layer 0; only the index within the layer varies.
    fn nid(i: usize) -> NodeId {
        NodeId::new(i as u32, 0)
    }

    /// A point on the +X axis, so its distance from the origin is exactly `x`.
    fn pt(x: f32) -> Coordinate {
        Coordinate::new(x, 0.0, 0.0)
    }

    /// Converts 1D points into Vec<Node> with generated ids
    fn as_nodes(xs: &[f32]) -> Vec<Node> {
        xs.iter()
            .enumerate()
            .map(|(i, x)| Node::new(nid(i), Vec::new(), Some(pt(*x)), false, None))
            .collect()
    }

    /// Converts 1D points into Vec<(Coordinate, NodeId)> with generated ids
    fn as_coords(xs: &[f32]) -> Vec<(Coordinate, NodeId)> {
        xs.iter()
            .enumerate()
            .map(|(i, x)| (pt(*x), nid(i)))
            .collect()
    }

    /// Convert 1D points into a [`PosIndex`] with generated ids
    fn as_kdtree(xs: &[f32]) -> PosIndex {
        let mut tree = PosIndex::default();
        for (i, x) in xs.iter().enumerate() {
            tree.add(&[*x, 0.0, 0.0], nid(i)).unwrap();
        }
        tree
    }

    #[track_caller]
    fn assert_equal(backend: &str, got: &Vec<(NodeId, f32)>, want: &Vec<(NodeId, f32)>) {
        assert_eq!(
            got.len(),
            want.len(),
            "{backend}: returned {} results, kd-tree returned {}\n  got:  {got:?}\n  want: {want:?}",
            got.len(),
            want.len()
        );
        for ((got_id, got_d), (want_id, want_d)) in got.iter().zip(want.iter()) {
            assert_eq!(
                got_id, want_id,
                "{backend}: node id mismatch\n  got:  {got:?}\n  want: {want:?}"
            );
            assert!(
                (got_d - want_d).abs() <= 1e-5,
                "{backend}: distance for {got_id} is {got_d}, kd-tree says {want_d}"
            );
        }
    }

    #[test]
    fn vec_node_nearest_returns_true_distances() {
        let nodes = as_nodes(&[3.0, 4.0, 5.0]);

        let got = nodes.nearest_nodes(&Coordinate::ZERO, PosQueryCount::Count(3));

        let distances = got.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        assert_eq!(distances, vec![3.0, 4.0, 5.0]);

        let got = nodes.nearest_nodes(&Coordinate::ZERO, PosQueryCount::All);

        let distances = got.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        assert_eq!(distances, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn vec_coords_nearest_returns_true_distances() {
        let coords = as_coords(&[3.0, 4.0, 5.0]);

        let got = coords.nearest_nodes(&Coordinate::ZERO, PosQueryCount::Count(3));

        let distances = got.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        assert_eq!(distances, vec![3.0, 4.0, 5.0]);

        let got = coords.nearest_nodes(&Coordinate::ZERO, PosQueryCount::All);

        let distances = got.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        assert_eq!(distances, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn vec_coords_nodes_within_respects_radius() {
        let coords = as_coords(&[1.0, 3.0]);

        let got = coords.nodes_within(&Coordinate::ZERO, 2.0, PosQueryCount::All);

        assert_eq!(
            got.len(),
            1,
            "radius 2.0 must exclude the point at distance 3.0, got {got:?}"
        );
        assert_eq!(got[0].0, nid(0));
        assert!(
            (got[0].1 - 1.0).abs() <= 1e-5,
            "expected distance 1.0, got {}",
            got[0].1
        );
    }

    /// The invariant the benchmark suite rests on: all three backends must be
    /// interchangeable. The kd-tree is the reference, since indexing must not change
    /// what a query answers — only how fast it answers it.
    #[test]
    fn all_backends_agree_on_nearest_nodes() {
        let xs = [1.0, 4.0, 9.0, 16.0];
        let query = Coordinate::new(0.5, 2.0, -3.0);
        let nodes = as_nodes(&xs);
        let coords = as_coords(&xs);
        let tree = as_kdtree(&xs);

        for count in [
            PosQueryCount::Count(1),
            PosQueryCount::Count(3),
            PosQueryCount::Count(xs.len()),
            PosQueryCount::All,
        ] {
            let reference = tree.nearest_nodes(&query, count);
            assert_equal(
                &format!("Vec<Node> nearest_nodes({count:?})"),
                &nodes.nearest_nodes(&query, count),
                &reference,
            );
            assert_equal(
                &format!("Vec<(Coordinate, NodeId)> nearest_nodes({count:?})"),
                &coords.nearest_nodes(&query, count),
                &reference,
            );
        }
    }

    /// Same invariant for radius search, swept across radii on both sides of 1.0 so the
    /// squared-versus-unsquared comparison cannot pass by coincidence.
    #[test]
    fn all_backends_agree_on_nodes_within() {
        let xs = [1.0, 4.0, 9.0, 16.0];
        let query = Coordinate::ZERO;
        let nodes = as_nodes(&xs);
        let coords = as_coords(&xs);
        let tree = as_kdtree(&xs);

        for radius in [0.5f32, 1.0, 2.0, 5.0, 10.0, 100.0] {
            let reference = tree.nodes_within(&query, radius, PosQueryCount::All);
            assert_equal(
                &format!("Vec<Node> nodes_within(radius = {radius})"),
                &nodes.nodes_within(&query, radius, PosQueryCount::All),
                &reference,
            );
            assert_equal(
                &format!("Vec<(Coordinate, NodeId)> nodes_within(radius = {radius})"),
                &coords.nodes_within(&query, radius, PosQueryCount::All),
                &reference,
            );
        }
    }
}
