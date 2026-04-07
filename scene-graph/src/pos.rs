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
            PosQueryCount::All => self
                .iter()
                .map(|(id, coords)| (id, SquaredEuclidean::dist(&coords, &(*pos).into())))
                .map(|(id, d)| (id, d.sqrt()))
                .collect(),
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
            PosQueryCount::All => self
                .iter()
                .filter(|n| n.coordinates.is_some())
                .map(|n| (n.id, n.coordinates.unwrap().distance_squared(*pos)))
                .map(|(id, d)| (id, d.sqrt()))
                .collect(),
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
        self.iter()
            .filter(|n| n.coordinates.is_some())
            .map(|n| (n, n.coordinates.unwrap().distance_squared(*pos)))
            .filter(|(_, d)| *d <= radius * radius)
            .map(|(node, dist)| (node.id, dist))
            .map(|(id, d)| (id, d.sqrt()))
            .take(count)
            .collect::<Vec<_>>()
    }
}

impl PositionQuery for Vec<(Coordinate, NodeId)> {
    /// Retrieve at most `count` nearest nodes to the given position.
    /// The list is sorted by distance from nearest to farthest.
    fn nearest_nodes(&self, pos: &super::Coordinate, count: PosQueryCount) -> Vec<(NodeId, f32)> {
        match count {
            PosQueryCount::All => self
                .iter()
                .map(|(c, id)| (*id, c.distance_squared(*pos)))
                .collect(),
            PosQueryCount::Count(c) => {
                let mut distances = self
                    .iter()
                    .map(|(c, id)| (*id, c.distance_squared(*pos)))
                    .map(|(id, d)| (id, d.sqrt()))
                    .collect::<Vec<(NodeId, f32)>>();
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
        self.iter()
            .map(|(c, id)| (*id, c.distance_squared(*pos)))
            .map(|(id, d)| (id, d.sqrt()))
            .filter(|(_, d)| *d <= radius * radius)
            .take(count)
            .collect::<Vec<_>>()
    }
}
