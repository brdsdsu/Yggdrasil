use std::num::NonZero;

use spatial_index::{Eytzinger, KdTree, SquaredEuclidean, VecOfArrays, dist::DistanceMetricScalar};

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
/// `Eytzinger` is the breadth-first stem layout, and `VecOfArrays` is the mutable leaf
/// strategy, which is what makes incremental `add`/`remove` available — the position
/// indexes are maintained point by point as the graph changes, never rebuilt in bulk.
pub(crate) type PosIndex = KdTree<
    f32,
    NodeId,
    Eytzinger,
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

/// Characterization tests for the coplanar-split defect.
///
/// The kd-tree picks the axis it splits a full leaf on by cycling with tree depth. When a
/// leaf fills up with points that all share one value on the cycled axis, there is no pivot
/// to split at and the insert is refused. Coplanarity is not an exotic input for this crate:
/// reconstructed indoor geometry snapped to a voxel lattice puts thousands of points on a
/// single floor, wall, or ceiling plane, far past a 128-point bucket.
///
/// These tests assert the behaviour as it stands today, so they are a record of the defect
/// rather than a statement of intent. **They are expected to fail once the split axis is
/// chosen adaptively**, which is the point: that is when they get rewritten into the
/// acceptance tests that assert the inserts succeed and the queries agree with brute force.
#[cfg(test)]
mod coplanar_defect {
    use super::*;
    use crate::layer::Layer;
    use crate::node::AnonNode;

    /// Feed `points` into a fresh index one at a time, reporting how many landed before the
    /// tree refused one, and the axis it gave up on.
    fn insert_until_refused(points: &[[f32; 3]]) -> (usize, Option<usize>) {
        let mut tree = PosIndex::default();
        for (i, p) in points.iter().enumerate() {
            if let Err(spatial_index::kd_tree::ConstructionError::UnsplittableBucket { split_dim }) =
                tree.add(p, NodeId::new(i as u32, 0))
            {
                return (i, Some(split_dim));
            }
        }
        (points.len(), None)
    }

    /// Points spread over x and y but sharing one z: the shape of a single ceiling plane.
    /// Splits on x and y succeed, so the tree absorbs well over a bucket's worth before the
    /// cycle reaches z and finds nothing to split on.
    #[test]
    fn coplanar_plane_is_refused_on_the_shared_axis() {
        let plane: Vec<[f32; 3]> = (0..500)
            .map(|i| [(i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85])
            .collect();

        let (accepted, split_dim) = insert_until_refused(&plane);

        assert_eq!(split_dim, Some(2), "must give up on z, the shared axis");
        assert_eq!(accepted, 348, "insert #348 of 500 is refused");
    }

    /// Fully degenerate input: no axis has any spread, so the refusal lands exactly at
    /// bucket capacity — the first insert that would require a split.
    #[test]
    fn identical_points_are_refused_at_bucket_capacity() {
        let (accepted, split_dim) = insert_until_refused(&[[1.0, 2.0, 3.0]; 300]);

        assert_eq!(split_dim, Some(0));
        assert_eq!(
            accepted, POS_INDEX_BUCKET_SIZE,
            "the bucket fills, then the very next insert has nowhere to go"
        );
    }

    /// The realistic shape: a room on a 0.05 m lattice, floor plus two walls. Each surface
    /// is one plane holding thousands of points, and the tree stops 208 points into 12,800.
    #[test]
    fn quantized_lattice_room_is_refused() {
        let mut room: Vec<[f32; 3]> = Vec::new();
        for i in 0..80 {
            for j in 0..80 {
                room.push([i as f32 * 0.05, 0.0, j as f32 * 0.05]);
            }
        }
        for i in 0..80 {
            for k in 0..40 {
                room.push([i as f32 * 0.05, k as f32 * 0.05, 0.0]);
                room.push([0.0, k as f32 * 0.05, i as f32 * 0.05]);
            }
        }

        let (accepted, split_dim) = insert_until_refused(&room);

        assert_eq!(split_dim, Some(1), "the floor plane's constant axis");
        assert_eq!(accepted, 208, "208 of {} points accepted", room.len());
    }

    /// The same defect as the caller sees it: an indexed layer cannot hold a coplanar patch,
    /// and reports it as a failed insert rather than silently dropping the node.
    #[test]
    fn indexed_layer_refuses_coplanar_geometry() {
        let mut layer = Layer::with_pos_index(0);

        let outcome = (0..500).try_for_each(|i| {
            let pos = Coordinate::new((i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85);
            layer
                .push_node(AnonNode::new(i as u32, Vec::new(), Some(pos), false, None))
                .map(|_| ())
        });

        assert!(matches!(
            outcome,
            Err(crate::error::SceneGraphError::PosIndexInsertion(_))
        ));
        // The refused node is not left half-inserted: the layer holds exactly what the
        // index accepted.
        assert_eq!(layer.all_nodes().len(), 348);
    }

    /// The unindexed backends handle the same geometry without complaint, which is what
    /// makes this specifically a defect in the feature meant to make large graphs fast.
    #[test]
    fn unindexed_backends_accept_the_same_geometry() {
        let mut layer = Layer::new(0);

        for i in 0..500 {
            let pos = Coordinate::new((i % 25) as f32 * 0.05, (i / 25) as f32 * 0.05, 3.85);
            layer
                .push_node(AnonNode::new(i as u32, Vec::new(), Some(pos), false, None))
                .expect("an unindexed layer has no bucket to split");
        }

        assert_eq!(layer.all_nodes().len(), 500);
        let found = layer.nearest_nodes(&Coordinate::new(0.0, 0.0, 3.85), PosQueryCount::Count(4));
        assert_eq!(found.len(), 4);
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
