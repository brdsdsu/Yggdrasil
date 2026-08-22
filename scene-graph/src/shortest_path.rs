use std::cmp::Ordering;
use std::collections::BinaryHeap;

use super::error::{Result, SceneGraphError};
use super::{Layer, NodeId};

/// Result of a single-source shortest-path run over one layer's intra-layer edges.
///
/// Settled nodes appear in `dist` and `paths` in the order they were settled,
/// which for equal-cost ties matches the insertion-order-first semantics of
/// networkx's `single_source_dijkstra`. Nodes unreachable from the source are
/// simply absent.
#[derive(Debug, Clone)]
pub struct ShortestPaths {
    /// The source node the run started from.
    pub source: NodeId,
    /// `(settled node, distance from source)` in settle order.
    pub dist: Vec<(NodeId, f64)>,
    /// `paths[i]` is the full node sequence `source ..= dist[i].0`.
    pub paths: Vec<Vec<NodeId>>,
}

impl ShortestPaths {
    /// Distance to `target`, if it was settled.
    pub fn distance_to(&self, target: NodeId) -> Option<f64> {
        self.dist
            .iter()
            .find(|(id, _)| *id == target)
            .map(|(_, d)| *d)
    }

    /// Path to `target`, if it was settled.
    pub fn path_to(&self, target: NodeId) -> Option<&[NodeId]> {
        self.dist
            .iter()
            .position(|(id, _)| *id == target)
            .map(|i| self.paths[i].as_slice())
    }
}

/// Heap entry ordered so that `BinaryHeap` pops the smallest distance first and,
/// on equal distances, the entry pushed earliest — replicating the
/// `(dist, counter, node)` tuples networkx pushes into its heap.
struct HeapEntry {
    dist: f64,
    counter: u64,
    idx: usize,
}

impl PartialEq for HeapEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HeapEntry {}
impl PartialOrd for HeapEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HeapEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: BinaryHeap is a max-heap, we need the smallest entry on top.
        other
            .dist
            .total_cmp(&self.dist)
            .then_with(|| other.counter.cmp(&self.counter))
    }
}

impl Layer {
    /// Single-source shortest paths over this layer's intra-layer edges.
    ///
    /// The weight of an edge is the Euclidean distance between the first
    /// coordinate of its endpoints, computed in `f64`. Edges are directed;
    /// callers wanting undirected semantics must have inserted both directions.
    ///
    /// Equal-cost ties resolve to the path found first following `Node::edges`
    /// order, so results are path-identical to networkx's
    /// `single_source_dijkstra` when edges were inserted in the same order
    /// networkx saw them.
    ///
    /// Errors with [`SceneGraphError::NodeNotFound`] if `source` is not in this
    /// layer, or [`SceneGraphError::CoordinatesRequired`] if a node touched by
    /// the search has no coordinates.
    pub fn dijkstra(&self, source: NodeId) -> Result<ShortestPaths> {
        self.dijkstra_impl(source, None)
    }

    /// [`Layer::dijkstra`] that stops as soon as every node in `targets` has
    /// been settled. Unreachable targets are absent from the result.
    pub fn dijkstra_to(&self, source: NodeId, targets: &[NodeId]) -> Result<ShortestPaths> {
        self.dijkstra_impl(source, Some(targets))
    }

    fn dijkstra_impl(&self, source: NodeId, targets: Option<&[NodeId]>) -> Result<ShortestPaths> {
        let idx_of = |id: NodeId| -> Result<usize> {
            self.nodes
                .binary_search_by_key(&id, |node| node.id)
                .map_err(|_| SceneGraphError::NodeNotFound)
        };

        let n = self.nodes.len();
        let source_idx = idx_of(source)?;

        // How many targets still need settling; None means "settle everything".
        let mut remaining_targets = match targets {
            Some(t) => {
                let mut is_target = vec![false; n];
                for id in t {
                    // Duplicate targets count once.
                    is_target[idx_of(*id)?] = true;
                }
                Some((is_target.iter().filter(|b| **b).count(), is_target))
            }
            None => None,
        };

        let pos = |idx: usize| -> Result<[f64; 3]> {
            let c = self.nodes[idx]
                .coordinates
                .first()
                .ok_or(SceneGraphError::CoordinatesRequired)?;
            Ok([c.x as f64, c.y as f64, c.z as f64])
        };

        // seen[v]: best distance pushed for v so far; settled[v]: v already popped.
        let mut seen: Vec<Option<f64>> = vec![None; n];
        let mut settled = vec![false; n];
        let mut pred: Vec<usize> = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        let mut counter: u64 = 0;

        let mut out_dist: Vec<(NodeId, f64)> = Vec::new();
        let mut out_paths: Vec<Vec<NodeId>> = Vec::new();

        // Verify the source has coordinates up front so an isolated source with no
        // coordinates errors instead of silently returning a zero-length result.
        pos(source_idx)?;
        seen[source_idx] = Some(0.0);
        heap.push(HeapEntry {
            dist: 0.0,
            counter,
            idx: source_idx,
        });

        while let Some(HeapEntry { dist, idx: u, .. }) = heap.pop() {
            if settled[u] {
                continue;
            }
            settled[u] = true;

            out_dist.push((self.nodes[u].id, dist));
            out_paths.push({
                let mut path = Vec::new();
                let mut cur = u;
                while cur != usize::MAX {
                    path.push(self.nodes[cur].id);
                    cur = if cur == source_idx { usize::MAX } else { pred[cur] };
                }
                path.reverse();
                path
            });

            if let Some((remaining, is_target)) = &mut remaining_targets {
                if is_target[u] {
                    *remaining -= 1;
                    if *remaining == 0 {
                        break;
                    }
                }
            }

            let u_pos = pos(u)?;
            for edge in &self.nodes[u].edges {
                let v = idx_of(edge.dst)?;
                if settled[v] {
                    continue;
                }
                let v_pos = pos(v)?;
                let weight = ((u_pos[0] - v_pos[0]).powi(2)
                    + (u_pos[1] - v_pos[1]).powi(2)
                    + (u_pos[2] - v_pos[2]).powi(2))
                .sqrt();
                let vu_dist = dist + weight;
                // Strict improvement only: on an equal-cost tie the earlier push
                // wins, matching networkx.
                if seen[v].is_none_or(|s| vu_dist < s) {
                    seen[v] = Some(vu_dist);
                    pred[v] = u;
                    counter += 1;
                    heap.push(HeapEntry {
                        dist: vu_dist,
                        counter,
                        idx: v,
                    });
                }
            }
        }

        Ok(ShortestPaths {
            source,
            dist: out_dist,
            paths: out_paths,
        })
    }
}

#[cfg(test)]
mod test {
    use super::super::{Feature, NodeId, SceneGraph};
    use super::*;

    /// Build a layer of nodes at the given positions with the given directed
    /// edges (by position index), inserting both directions per pair to model
    /// an undirected graph the way the LP2 converter does.
    fn build(
        positions: &[[f32; 3]],
        undirected_edges: &[(usize, usize)],
    ) -> (SceneGraph, Vec<NodeId>) {
        let mut sg = SceneGraph::default();
        let mut anon = Vec::new();
        for p in positions {
            anon.push(sg.new_coordinates(p[0], p[1], p[2], Vec::new()));
        }
        let layer = sg.new_layer();
        let ids: Vec<NodeId> = anon
            .into_iter()
            .map(|a| layer.push_node(a).unwrap())
            .collect();
        for (a, b) in undirected_edges {
            layer.add_edge(ids[*a], ids[*b], "").unwrap();
            layer.add_edge(ids[*b], ids[*a], "").unwrap();
        }
        (sg, ids)
    }

    #[test]
    fn hand_computed_with_equal_cost_diamond() {
        // 0 --1--> 1 --1--> 3, and 0 --1--> 2 --1--> 3: two equal-cost routes to 3.
        // Edge (0,1) is inserted before (0,2), so networkx (and we) settle the
        // path through 1 first and keep it.
        let positions = [
            [0.0, 0.0, 0.0], // 0
            [1.0, 0.0, 0.0], // 1
            [0.0, 1.0, 0.0], // 2 (same distance from 0 as node 1)
            [1.0, 1.0, 0.0], // 3
            [3.0, 1.0, 0.0], // 4 (further out, reached via 3)
        ];
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4)];
        let (sg, ids) = build(&positions, &edges);
        let sp = sg.layer(0).unwrap().dijkstra(ids[0]).unwrap();

        let d = |t: usize| sp.distance_to(ids[t]).unwrap();
        assert_eq!(d(0), 0.0);
        assert!((d(1) - 1.0).abs() < 1e-12);
        assert!((d(2) - 1.0).abs() < 1e-12);
        assert!((d(3) - 2.0).abs() < 1e-12);
        assert!((d(4) - 4.0).abs() < 1e-12);

        // Tie-break: path to 3 goes through 1 (first-inserted edge), not 2.
        assert_eq!(sp.path_to(ids[3]).unwrap(), &[ids[0], ids[1], ids[3]]);
        assert_eq!(
            sp.path_to(ids[4]).unwrap(),
            &[ids[0], ids[1], ids[3], ids[4]]
        );
        // Settle order: source first, then the two distance-1 nodes in push order.
        assert_eq!(sp.dist[0].0, ids[0]);
        assert_eq!(sp.dist[1].0, ids[1]);
        assert_eq!(sp.dist[2].0, ids[2]);
    }

    #[test]
    fn matches_brute_force_on_random_graphs() {
        // Deterministic LCG so the test is reproducible without a rand dependency.
        let mut state: u64 = 0x9E3779B97F4A7C15;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 33) as f64 / (1u64 << 31) as f64
        };

        for round in 0..30 {
            let n = 8 + (round % 5) * 13; // 8..=60 nodes
            let positions: Vec<[f32; 3]> = (0..n)
                .map(|_| {
                    [
                        (next() * 20.0) as f32,
                        (next() * 20.0) as f32,
                        (next() * 4.0) as f32,
                    ]
                })
                .collect();
            let mut edges = Vec::new();
            for a in 0..n {
                for b in (a + 1)..n {
                    if next() < 0.15 {
                        edges.push((a, b));
                    }
                }
            }
            let (sg, ids) = build(&positions, &edges);
            let layer = sg.layer(0).unwrap();

            // Floyd-Warshall reference in f64 over the same weights.
            let inf = f64::INFINITY;
            let mut ref_dist = vec![vec![inf; n]; n];
            for (i, row) in ref_dist.iter_mut().enumerate() {
                row[i] = 0.0;
            }
            let p64 = |i: usize| {
                let p = positions[i];
                [p[0] as f64, p[1] as f64, p[2] as f64]
            };
            for (a, b) in &edges {
                let (pa, pb) = (p64(*a), p64(*b));
                let w = ((pa[0] - pb[0]).powi(2) + (pa[1] - pb[1]).powi(2) + (pa[2] - pb[2]).powi(2))
                    .sqrt();
                if w < ref_dist[*a][*b] {
                    ref_dist[*a][*b] = w;
                    ref_dist[*b][*a] = w;
                }
            }
            for k in 0..n {
                for i in 0..n {
                    for j in 0..n {
                        let via = ref_dist[i][k] + ref_dist[k][j];
                        if via < ref_dist[i][j] {
                            ref_dist[i][j] = via;
                        }
                    }
                }
            }

            let src = round % n;
            let sp = layer.dijkstra(ids[src]).unwrap();

            // Every settled distance matches the reference.
            for (id, d) in &sp.dist {
                let t = ids.iter().position(|x| x == id).unwrap();
                assert!(
                    (d - ref_dist[src][t]).abs() < 1e-9,
                    "round {round}: dist to {t} was {d}, reference {}",
                    ref_dist[src][t]
                );
            }
            // Every reachable node was settled.
            let settled_count = sp.dist.len();
            let reachable = (0..n).filter(|t| ref_dist[src][*t] < inf).count();
            assert_eq!(settled_count, reachable, "round {round}");

            // Every returned path's edge-sum equals its reported distance.
            for (i, (_, d)) in sp.dist.iter().enumerate() {
                let path = &sp.paths[i];
                let mut sum = 0.0;
                for w in path.windows(2) {
                    let a = ids.iter().position(|x| *x == w[0]).unwrap();
                    let b = ids.iter().position(|x| *x == w[1]).unwrap();
                    let (pa, pb) = (p64(a), p64(b));
                    sum += ((pa[0] - pb[0]).powi(2)
                        + (pa[1] - pb[1]).powi(2)
                        + (pa[2] - pb[2]).powi(2))
                    .sqrt();
                }
                assert!((sum - d).abs() < 1e-9, "round {round}: path sum mismatch");
            }
        }
    }

    #[test]
    fn disconnected_nodes_are_absent() {
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [50.0, 50.0, 0.0], // island
        ];
        let (sg, ids) = build(&positions, &[(0, 1)]);
        let layer = sg.layer(0).unwrap();

        let sp = layer.dijkstra(ids[0]).unwrap();
        assert_eq!(sp.dist.len(), 2);
        assert!(sp.distance_to(ids[2]).is_none());

        // Early-exit variant terminates and omits the unreachable target.
        let sp = layer.dijkstra_to(ids[0], &[ids[2]]).unwrap();
        assert!(sp.distance_to(ids[2]).is_none());
    }

    #[test]
    fn dijkstra_to_is_a_prefix_of_full_run() {
        let positions: Vec<[f32; 3]> = (0..12)
            .map(|i| [(i % 4) as f32, (i / 4) as f32, 0.0])
            .collect();
        let mut edges = Vec::new();
        for i in 0..12usize {
            if i % 4 < 3 {
                edges.push((i, i + 1));
            }
            if i / 4 < 2 {
                edges.push((i, i + 4));
            }
        }
        let (sg, ids) = build(&positions, &edges);
        let layer = sg.layer(0).unwrap();

        let full = layer.dijkstra(ids[0]).unwrap();
        let partial = layer.dijkstra_to(ids[0], &[ids[5], ids[7]]).unwrap();

        // The partial run's settle sequence is a prefix of the full run's, and
        // agrees on every distance and path.
        assert!(partial.dist.len() <= full.dist.len());
        for i in 0..partial.dist.len() {
            assert_eq!(partial.dist[i].0, full.dist[i].0);
            assert!((partial.dist[i].1 - full.dist[i].1).abs() < 1e-12);
            assert_eq!(partial.paths[i], full.paths[i]);
        }
        // Both requested targets were settled.
        assert!(partial.distance_to(ids[5]).is_some());
        assert!(partial.distance_to(ids[7]).is_some());
    }

    #[test]
    fn errors() {
        let (sg, ids) = build(&[[0.0; 3], [1.0, 0.0, 0.0]], &[(0, 1)]);
        let layer = sg.layer(0).unwrap();

        // Unknown source.
        let bogus = NodeId::new(999, 0);
        assert!(layer.dijkstra(bogus).is_err());
        // Unknown target.
        assert!(layer.dijkstra_to(ids[0], &[bogus]).is_err());

        // A node without coordinates in the search area errors.
        let mut sg = SceneGraph::default();
        let a = sg.new_coordinates(0.0, 0.0, 0.0, Vec::new());
        let b = sg.new_node(vec![Feature::new("name", "no coords")]);
        let layer = sg.new_layer();
        let aid = layer.push_node(a).unwrap();
        let bid = layer.push_node(b).unwrap();
        layer.add_edge(aid, bid, "").unwrap();
        assert!(matches!(
            sg.layer(0).unwrap().dijkstra(aid),
            Err(SceneGraphError::CoordinatesRequired)
        ));
    }
}
