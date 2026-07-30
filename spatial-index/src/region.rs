//! Classifying an axis-aligned region against a query volume.
//!
//! A tree query prunes by asking, at each node, whether the region that node covers can hold
//! anything the caller wants. The distance queries answer that with a scalar — see
//! [`QueryContext::max_dist`] — which is enough for a ball and nothing else. This trait is the
//! general form: hand the region to the caller and let it decide.
//!
//! [`QueryContext::max_dist`]: crate::kd_tree::query_context::QueryContext::max_dist
//!
//! # Why the predicate takes a region and not a point
//!
//! The natural first design is a pair of pointwise predicates — an infimum and a supremum —
//! where a point is a member when it satisfies both. That gives sound *acceptance*: a box is the
//! convex hull of its corners, so if a convex volume contains all of them it contains the box.
//!
//! It cannot give sound *rejection*. All corners outside says nothing — a narrow cone can pass
//! clean through a large cell with every corner outside it. And rejection is the whole reason a
//! tree query is faster than a scan: accepting a subtree still leaves every point in it to be
//! enumerated, because those points are the answer.
//!
//! The tell is that a pointwise scheme has no "cannot decide" state — a point is never
//! ambiguous, a region is — and that state is the one a traversal actually branches on.

/// What a [`RegionMembershipPredicate`] concluded about a region.
///
/// Two states rather than three. A third meaning "every point here is a member", which would let
/// a traversal emit a subtree without testing it, was measured on the visibility workload at
/// roughly 2% — the members still have to be enumerated, being the answer — and left out so the
/// contract stays small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionMembershipStatus {
    /// No point of the region is a member, so the subtree can be skipped entirely.
    ///
    /// This is the load-bearing answer: a `Reject` that is wrong silently drops results, and
    /// nothing downstream of the traversal can notice.
    Reject,
    /// The region cannot be ruled out and has to be opened.
    ///
    /// Deliberately not called `Accept` — it asserts nothing about whether anything in the
    /// region is a member, only that rejecting it would be unsound. Reading it the other way is
    /// how an unsound predicate gets written.
    Undecided,
}

/// A volume that can decide whether an axis-aligned region is worth opening.
///
/// # Contract
///
/// [`classify`](Self::classify) must be **conservative**: [`RegionMembershipStatus::Reject`]
/// asserts that *no* point of the box is a member, so an implementation that is unsure must
/// answer [`RegionMembershipStatus::Undecided`]. Answering `Undecided` always is correct and
/// merely slow; a wrong `Reject` is a silent wrong answer.
///
/// [`contains`](Self::contains) decides a single point, and must agree with `classify` in the
/// sense that a box holding a point for which `contains` is true is never `Reject`ed.
///
/// Implementations are expected to be pure — the traversal calls them many times per query, in
/// an unspecified order, and may skip calls entirely when another bound has already pruned.
///
/// [`crate::test_utils::region`] provides a randomised harness for exactly these two properties;
/// they are worth checking rather than assuming, because nothing else in the system will.
pub trait RegionMembershipPredicate<A, const K: usize> {
    /// Classify the axis-aligned box spanned by `lo` and `hi`, inclusive.
    fn classify(&self, lo: &[A; K], hi: &[A; K]) -> RegionMembershipStatus;

    /// Whether a single point is a member.
    fn contains(&self, point: &[A; K]) -> bool;
}
