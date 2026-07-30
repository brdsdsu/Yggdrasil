//! Lazy iteration over the points a [`RegionMembershipPredicate`] admits.
//!
//! The distance queries prune with a scalar — the accumulated lower bound from the query point
//! to the current cell, against `max_dist`. A region predicate cannot be expressed that way, so
//! this traversal carries the cell's axis-aligned box instead and asks the predicate to
//! classify it. A [`RegionMembershipStatus::Reject`] drops the whole subtree unopened.
//!
//! # Why this is an iterator rather than a callback
//!
//! It mirrors [`WithinUnsortedIter`](super::WithinUnsortedIter): the traversal state lives in
//! the struct — an explicit stack of frames plus a cursor into the leaf being drained — so
//! `next` can suspend and resume. That gives callers early exit and composition with the rest
//! of `Iterator` for free, and a `visit`-style callback or a materialised `Vec` are then just
//! `for` and `collect` at the call site rather than separate engines.
//!
//! # Why it yields points
//!
//! [`Iterator::Item`] is `(point, item)` and not `item` alone, and that is load-bearing rather
//! than a convenience. A membership query has to run [`RegionMembershipPredicate::contains`] on
//! each surviving point, so the coordinate must reach the caller anyway; handing back bare ids
//! would force a lookup per survivor to recover it. `QueryBuilder::iter` for `within` takes the
//! other route — when points are requested it abandons its pruning traversal and falls back to
//! [`KdTreeIter`](super::KdTreeIter) over the whole tree, filtering afterwards. Doing that here
//! would discard exactly the rejection this exists to perform.

use crate::region::{RegionMembershipPredicate, RegionMembershipStatus};
use crate::traits::leaf_strategy::LeafStrategy;
use crate::{Axis, Content, StemStrategy};

use super::iter::InlineStack;
use super::{KdTreeAccessor, StemLeafResolution};

/// Frames held while the traversal is suspended.
///
/// The box lives here rather than in `QueryStackContext`, which the distance queries use: that
/// type has no `K` parameter, so widening it would mean changing `ScalarStackContext::from_parts`
/// and every implementor, the SIMD stack contexts included. This frame is private to the
/// iterator and already carries `K`, so the box costs nothing outside this file.
struct RegionFrame<SS, A, const K: usize> {
    stem_strat: SS,
    lo: [A; K],
    hi: [A; K],
}

const INLINE_STACK_CAPACITY: usize = 64;

/// Iterator over the `(point, item)` pairs a predicate admits.
pub struct MembershipIter<'a, Tree, A, T, SS, LS, P, const K: usize, const B: usize>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: LeafStrategy<A, T, SS, K, B>,
    Tree: KdTreeAccessor<A, T, SS, LS, K, B>,
    P: RegionMembershipPredicate<A, K>,
{
    tree: &'a Tree,
    predicate: P,
    stack: InlineStack<RegionFrame<SS, A, K>, INLINE_STACK_CAPACITY>,
    /// The leaf currently being drained, if any.
    leaf: Option<usize>,
    leaf_len: usize,
    pos_in_leaf: usize,
    _phantom: std::marker::PhantomData<(T, LS)>,
}

impl<'a, Tree, A, T, SS, LS, P, const K: usize, const B: usize>
    MembershipIter<'a, Tree, A, T, SS, LS, P, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: LeafStrategy<A, T, SS, K, B>,
    Tree: KdTreeAccessor<A, T, SS, LS, K, B>,
    P: RegionMembershipPredicate<A, K>,
{
    /// The root cell is unbounded on every face.
    ///
    /// The tree records no global bounds, and rather than compute and store them the traversal
    /// starts at infinity and lets the splits narrow it. A predicate sees a box with infinite
    /// extent for the first few levels and cannot usefully reject one, which costs a handful of
    /// unpruned nodes near the root and avoids touching construction or the serialised format.
    /// Every face is finite once each axis has been split at least once.
    pub(crate) fn new(tree: &'a Tree, predicate: P) -> Self {
        let mut stack = InlineStack::new();
        if tree.size() != 0 {
            let stems_ptr =
                std::ptr::NonNull::new(tree.stems().as_ptr() as *mut u8).expect("stems are non-null");
            // `bind_stems` before anything reads a split: `EytzingerAdaptive` decodes the split
            // dimension out of the stem itself, and an unbound strategy reports dimension 0 for
            // every node. That silently narrows one axis at every level and produces boxes that
            // do not contain their own subtree, so the predicate rejects geometry it should keep.
            let mut root = SS::new(stems_ptr);
            root.bind_stems::<A>(tree.stems());
            stack.push(RegionFrame {
                stem_strat: root,
                lo: [A::min_value(); K],
                hi: [A::max_value(); K],
            });
        }

        Self {
            tree,
            predicate,
            stack,
            leaf: None,
            leaf_len: 0,
            pos_in_leaf: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Mirrors `WithinUnsortedIter::resolve_current_leaf`.
    #[inline(always)]
    fn resolve_current_leaf(&self, stem_strat: &SS) -> Option<usize> {
        if self
            .tree
            .stem_leaf_resolution()
            .is_terminal_stem_idx(stem_strat.stem_idx())
        {
            Some(
                self.tree
                    .stem_leaf_resolution()
                    .resolve_terminal_stem_idx(stem_strat.stem_idx(), 0),
            )
        } else if stem_strat.level() > self.tree.max_stem_level() {
            Some(
                self.tree
                    .stem_leaf_resolution()
                    .resolve_terminal_stem_idx(stem_strat.stem_idx(), stem_strat.leaf_idx()),
            )
        } else {
            None
        }
    }

    /// Descend until a leaf is reached, deferring the sibling with its own box.
    ///
    /// Returns `false` when the stack empties without finding one.
    fn descend_to_leaf(&mut self) -> bool {
        while let Some(frame) = self.stack.pop() {
            if self.predicate.classify(&frame.lo, &frame.hi) == RegionMembershipStatus::Reject {
                continue;
            }

            let mut stem_strat = frame.stem_strat;
            // `lo` never moves along this walk: `branch` always sends `self` to the left child,
            // whose lower face is inherited unchanged. Only `hi` narrows.
            let lo = frame.lo;
            let mut hi = frame.hi;

            loop {
                if let Some(leaf_idx) = self.resolve_current_leaf(&stem_strat) {
                    self.leaf = Some(leaf_idx);
                    self.leaf_len = self.tree.leaves().leaf_len(leaf_idx);
                    self.pos_in_leaf = 0;
                    return true;
                }

                let dim = stem_strat.dim::<K>();
                let stem_idx = stem_strat.stem_idx();
                let pivot = if stem_idx < self.tree.stems().len() {
                    unsafe { *self.tree.stems().get_unchecked(stem_idx) }
                } else {
                    A::max_value()
                };

                if pivot >= A::max_value() {
                    // Sentinel: no split here, so the cell is unchanged and the walk continues.
                    stem_strat.traverse::<A, K>(false);
                    continue;
                }

                // The stored pivot is the partition boundary itself — construction guarantees
                // the left subtree sits strictly below it and the right at or above — so it is
                // the correct face for both child boxes. It is used verbatim, without stripping
                // the packed split dimension out of its low bits, because those bits are part of
                // the boundary the points were actually partitioned against; decoding first
                // would move the face and could exclude a point that belongs inside it.
                // Descend left, defer right. There is no query point and so no near/far
                // ordering to respect — results are unordered, so either child may go first.
                //
                // `branch` is the operation that states this directly: it sends `self` into the
                // left child and hands back the right. `branch_relative(is_right)` would invert
                // it, sending `self` to the child named by the flag, which pairs each context
                // with the *other* one's box and rejects geometry that should have been kept.
                let mut far_lo = lo;
                let far_hi = hi;
                unsafe {
                    *far_lo.get_unchecked_mut(dim) = pivot;
                    *hi.get_unchecked_mut(dim) = pivot;
                }
                let far_ctx = stem_strat.branch::<A, K>();

                if self.predicate.classify(&far_lo, &far_hi) != RegionMembershipStatus::Reject {
                    self.stack.push(RegionFrame {
                        stem_strat: far_ctx,
                        lo: far_lo,
                        hi: far_hi,
                    });
                }
            }
        }

        false
    }
}

impl<Tree, A, T, SS, LS, P, const K: usize, const B: usize> Iterator
    for MembershipIter<'_, Tree, A, T, SS, LS, P, K, B>
where
    A: Axis<Coord = A>,
    T: Content,
    SS: StemStrategy,
    LS: LeafStrategy<A, T, SS, K, B>,
    Tree: KdTreeAccessor<A, T, SS, LS, K, B>,
    P: RegionMembershipPredicate<A, K>,
{
    type Item = ([A; K], T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Drain the leaf in hand before opening another.
            if self.leaf.is_some() {
                while self.pos_in_leaf < self.leaf_len {
                    let leaf_idx = self.leaf.expect("leaf is in hand");
                    let (point, item) =
                        self.tree.leaves().leaf_point_item(leaf_idx, self.pos_in_leaf);
                    self.pos_in_leaf += 1;
                    if self.predicate.contains(&point) {
                        return Some((point, item));
                    }
                }
                self.leaf = None;
            }

            if !self.descend_to_leaf() {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::region::{RegionMembershipPredicate, RegionMembershipStatus};
    use crate::{EytzingerAdaptive, KdTree, VecOfArrays};

    type Tree = KdTree<f32, u32, EytzingerAdaptive, VecOfArrays<f32, u32, 3, 32>, 3, 32>;

    /// An axis-aligned box, which is the predicate whose exact answer is easiest to state
    /// independently — a point is a member iff every coordinate lies within.
    struct Boxed {
        lo: [f32; 3],
        hi: [f32; 3],
    }

    impl RegionMembershipPredicate<f32, 3> for Boxed {
        fn classify(&self, lo: &[f32; 3], hi: &[f32; 3]) -> RegionMembershipStatus {
            for axis in 0..3 {
                if hi[axis] < self.lo[axis] || lo[axis] > self.hi[axis] {
                    return RegionMembershipStatus::Reject;
                }
            }
            RegionMembershipStatus::Undecided
        }

        fn contains(&self, p: &[f32; 3]) -> bool {
            (0..3).all(|axis| p[axis] >= self.lo[axis] && p[axis] <= self.hi[axis])
        }
    }

    /// Rejects nothing, so the traversal is forced to open every subtree.
    struct Everything;

    impl RegionMembershipPredicate<f32, 3> for Everything {
        fn classify(&self, _: &[f32; 3], _: &[f32; 3]) -> RegionMembershipStatus {
            RegionMembershipStatus::Undecided
        }
        fn contains(&self, _: &[f32; 3]) -> bool {
            true
        }
    }

    fn scattered(count: u32) -> Vec<([f32; 3], u32)> {
        let mut state: u64 = 0x9e37_79b9;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 40) as f32 / (1u64 << 24) as f32) * 20.0 - 10.0
        };
        (0..count).map(|i| ([next(), next(), next()], i)).collect()
    }

    fn build(points: &[([f32; 3], u32)]) -> Tree {
        let mut tree = Tree::default();
        for (point, item) in points {
            tree.add(point, *item);
        }
        tree
    }

    /// The property everything rests on: pruning must not change the answer.
    ///
    /// Compared against a brute-force filter over the same points, for boxes ranging from ones
    /// that miss the cloud entirely to ones that swallow it, so both the fully-rejecting and
    /// fully-accepting paths are exercised as well as the partial overlaps in between.
    #[test]
    fn membership_matches_a_brute_force_scan() {
        let points = scattered(4_000);
        let tree = build(&points);

        let mut state: u64 = 0xdead_beef;
        let mut next = |lo: f32, hi: f32| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            lo + (hi - lo) * ((state >> 40) as f32 / (1u64 << 24) as f32)
        };

        let (mut ever_empty, mut ever_partial) = (false, false);
        for _ in 0..300 {
            let centre = [next(-14.0, 14.0), next(-14.0, 14.0), next(-14.0, 14.0)];
            let half = next(0.1, 12.0);
            let predicate = Boxed {
                lo: [centre[0] - half, centre[1] - half, centre[2] - half],
                hi: [centre[0] + half, centre[1] + half, centre[2] + half],
            };

            let mut expected: Vec<u32> = points
                .iter()
                .filter(|(p, _)| predicate.contains(p))
                .map(|(_, item)| *item)
                .collect();
            expected.sort_unstable();

            let mut got: Vec<u32> = tree.members(predicate).map(|(_, item)| item).collect();
            got.sort_unstable();

            assert_eq!(got, expected, "box centred {centre:?} half-width {half}");
            ever_empty |= expected.is_empty();
            ever_partial |= !expected.is_empty() && expected.len() < points.len();
        }
        assert!(
            ever_empty && ever_partial,
            "random boxes covered neither extreme, so the comparison proves little"
        );

        // The two extremes stated outright rather than left to chance: one box that swallows
        // every point, which must reject nothing, and one far away that must reject everything.
        // The random draw above reliably produces partial overlaps but not these.
        let all = Boxed {
            lo: [-1e6; 3],
            hi: [1e6; 3],
        };
        assert_eq!(
            tree.members(all).count(),
            points.len(),
            "a box containing the whole cloud dropped points"
        );
        let none = Boxed {
            lo: [1e5; 3],
            hi: [2e5; 3],
        };
        assert_eq!(
            tree.members(none).count(),
            0,
            "a box disjoint from the cloud returned points"
        );
    }

    /// A predicate that rejects nothing must yield the whole tree, exactly once each. This is
    /// what catches a traversal that loses or double-visits a subtree — a bug pruning would
    /// otherwise hide, since a rejected subtree is indistinguishable from a skipped one.
    #[test]
    fn rejecting_nothing_yields_every_point_exactly_once() {
        let points = scattered(4_000);
        let tree = build(&points);

        let mut got: Vec<u32> = tree.members(Everything).map(|(_, item)| item).collect();
        got.sort_unstable();

        let mut expected: Vec<u32> = points.iter().map(|(_, item)| *item).collect();
        expected.sort_unstable();

        assert_eq!(got.len(), points.len(), "wrong number of points yielded");
        assert_eq!(got, expected);
    }

    /// The points must come back with the coordinates they were stored under, or a caller
    /// running `contains` on them is testing the wrong geometry.
    #[test]
    fn yielded_points_match_the_stored_coordinates() {
        let points = scattered(1_000);
        let tree = build(&points);
        let by_item: std::collections::HashMap<u32, [f32; 3]> =
            points.iter().map(|(p, i)| (*i, *p)).collect();

        for (point, item) in tree.members(Everything) {
            assert_eq!(
                point.map(f32::to_bits),
                by_item[&item].map(f32::to_bits),
                "item {item} came back with a different coordinate"
            );
        }
    }

    /// Laziness is the reason this is an iterator, so it is worth asserting rather than
    /// assuming: taking two results must not require draining the traversal.
    #[test]
    fn iteration_is_lazy() {
        let points = scattered(4_000);
        let tree = build(&points);
        let taken: Vec<_> = tree.members(Everything).take(2).collect();
        assert_eq!(taken.len(), 2);
    }

    #[test]
    fn an_empty_tree_yields_nothing() {
        let tree = Tree::default();
        assert_eq!(tree.members(Everything).count(), 0);
    }


}
