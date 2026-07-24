use std::num::NonZero;

use kiddo::{SquaredEuclidean, float::kdtree::KdTree, traits::DistanceMetric};

use crate::NodeId;

use super::{Coordinate, Node};

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
    pub(crate) fn as_capped_usize(&self) -> usize {
        match self {
            PosQueryCount::All => usize::MAX,
            PosQueryCount::Count(k) => *k,
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

impl PositionQuery for KdTree<f32, NodeId, 3, 128, u32> {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)> {
        match count {
            PosQueryCount::All => {
                let mut distances: Vec<_> = self
                    .iter()
                    .map(|(id, coords)| (id, SquaredEuclidean::dist(&coords, &(*pos).into())))
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect();

                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                distances
            }
            PosQueryCount::Count(_) => {
                let count = count.as_capped_usize();
                let results = self.nearest_n::<SquaredEuclidean>(&(*pos).into(), count);
                results
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
        let count = count.as_capped_usize();
        let results = self.nearest_n_within::<SquaredEuclidean>(
            &(*pos).into(),
            radius * radius,
            unsafe { NonZero::new_unchecked(std::cmp::max(1, count)) },
            false,
        );
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
                    .map(|(id, d)| (id, d.sqrt()))
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
        let count = count.as_capped_usize();
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
                    .map(|(c, id)| (*id, c.distance_squared(*pos)))
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
        let count = count.as_capped_usize();
        let mut distances = self
            .iter()
            .map(|(c, id)| (*id, c.distance_squared(*pos)))
            .map(|(id, d)| (id, d.sqrt()))
            .filter(|(_, d)| *d <= radius * radius)
            .collect::<Vec<_>>();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.into_iter().take(count).collect::<Vec<_>>()
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

    /// The `Vec<Node>` backend, used by a `Layer` created without a position index.
    fn as_nodes(xs: &[f32]) -> Vec<Node> {
        xs.iter()
            .enumerate()
            .map(|(i, x)| Node::new(nid(i), Vec::new(), Some(pt(*x)), false, None))
            .collect()
    }

    /// The `Vec<(Coordinate, NodeId)>` backend, used by `nearest_in_subtree` when the
    /// root node has no position index (via `SceneGraph::pos_subtree`).
    fn as_coords(xs: &[f32]) -> Vec<(Coordinate, NodeId)> {
        xs.iter()
            .enumerate()
            .map(|(i, x)| (pt(*x), nid(i)))
            .collect()
    }

    /// The kd-tree backend, used by indexed layers and indexed nodes. Treated as the
    /// reference implementation: it is the only backend that is currently correct.
    fn as_kdtree(xs: &[f32]) -> KdTree<f32, NodeId, 3, 128, u32> {
        let mut tree = KdTree::new();
        for (i, x) in xs.iter().enumerate() {
            tree.add(&[*x, 0.0, 0.0], nid(i));
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
        assert_eq!(distances, vec![4.0, 9.0, 16.0]);

        let got = nodes.nearest_nodes(&Coordinate::ZERO, PosQueryCount::All);

        let distances = got.iter().map(|(_, d)| *d).collect::<Vec<_>>();
        assert_eq!(distances, vec![4.0, 9.0, 16.0]);
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
